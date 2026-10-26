//! hamesh-pi: clean-room Pi RPC adapter (M4 pathfinder).
//!
//! Speaks the Pi CLI's JSONL RPC protocol over child-process stdio — the
//! same contract Agent X's `AgentX.Core` stack uses, reimplemented here in
//! dependency-free Rust (std + serde_json only):
//!
//! - one JSON object per line (`pi --mode rpc ...`), LF-terminated;
//! - commands `{id, type, ...fields}`, responses `{type:"response", id, ...}`;
//! - unsolicited records are events (`message_update` text deltas,
//!   `agent_settled`, ...);
//! - `prompt` returns a disposition; `"handled"` means no run follows,
//!   otherwise the run ends at `agent_settled`.
//!
//! The transport is a trait: production spawns `pi`, tests plug a scripted
//! fake over TCP. Nothing here depends on Avalonia, Pi internals, or the
//! Agent X repository.

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::{Duration, Instant};

pub mod framing;
use framing::Record;

/// One typed observation from the Pi session.
#[derive(Clone, Debug)]
pub enum PiEvent {
    /// A streamed assistant text fragment.
    TextDelta(String),
    /// The run finished; `aborted` mirrors the wire flag.
    Settled { aborted: bool },
    /// Transport died; no more records will arrive.
    TransportClosed,
    /// Tool activity worth showing while a run is live.
    ToolNote(String),
    /// Anything else, preserved verbatim for forward compatibility.
    Raw(serde_json::Value),
}

/// Live run progress for `prompt_and_wait` consumers: streamed text
/// and tool-activity notes share one callback so the UI thread sees a
/// single ordered stream, exactly as the wire delivered it.
#[derive(Clone, Debug)]
pub enum PiProgress {
    Delta(String),
    ToolNote(String),
}

/// Failure modes. Timeouts and transport death are explicit — a hung Pi
/// never hangs the GUI silently.
#[derive(Clone, Debug)]
pub enum PiError {
    Transport(String),
    Command { command: String, error: String },
    Timeout(String),
    Protocol(String),
    Closed,
}

impl std::fmt::Display for PiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(e) => write!(f, "transport: {e}"),
            Self::Command { command, error } => {
                write!(f, "command {command} failed: {error}")
            }
            Self::Timeout(what) => write!(f, "timeout: {what}"),
            Self::Protocol(e) => write!(f, "protocol: {e}"),
            Self::Closed => write!(f, "connection closed"),
        }
    }
}

impl std::error::Error for PiError {}

/// Snapshot of engine state from `get_state`.
#[derive(Clone, Debug, Default)]
pub struct EngineState {
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub provider: Option<String>,
    pub thinking_level: String,
    pub is_streaming: bool,
    pub is_compacting: bool,
    pub session_id: Option<String>,
    pub message_count: i64,
    pub pending_message_count: i64,
    pub session_file: Option<String>,
}

impl EngineState {
    fn from_data(data: &serde_json::Value) -> Self {
        let s = |k: &str| data.get(k).and_then(|v| v.as_str()).map(|v| v.to_string());
        let i = |k: &str| data.get(k).and_then(|v| v.as_i64()).unwrap_or(0);
        let b = |k: &str| data.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
        let model = |k: &str| {
            data.get("model")
                .and_then(|m| m.get(k))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };
        Self {
            model_id: model("id"),
            model_name: model("name"),
            provider: model("provider"),
            thinking_level: s("thinkingLevel").unwrap_or_default(),
            is_streaming: b("isStreaming"),
            is_compacting: b("isCompacting"),
            session_id: s("sessionId"),
            message_count: i("messageCount"),
            pending_message_count: i("pendingMessageCount"),
            session_file: s("sessionFile"),
        }
    }
}

/// One selectable model from `get_available_models`.
#[derive(Clone, Debug)]
pub struct ModelOption {
    pub provider: String,
    pub id: String,
    pub name: String,
}

/// One restored conversation message: a feed role (`user`, `agent`,
/// `tool`, `system`) plus displayable text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryMessage {
    pub role: String,
    pub text: String,
}

/// Extract plain text from a message `content` that may be a string or
/// an array of typed blocks (`text`, `toolCall`, `image`, ...). Mirrors
/// Agent X's history extraction; unknown blocks are noted, never fatal.
fn content_text(content: &serde_json::Value) -> String {
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    let mut text = String::new();
    let mut notes = Vec::new();
    for block in content.as_array().cloned().unwrap_or_default() {
        let kind = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match kind {
            "text" => {
                if let Some(s) = block.get("text").and_then(|t| t.as_str()) {
                    text.push_str(s);
                }
            }
            "toolCall" => notes.push(format!(
                "[tool call: {}]",
                block.get("name").and_then(|n| n.as_str()).unwrap_or("?")
            )),
            "image" => notes.push("[image]".to_string()),
            _ => {}
        }
    }
    for note in notes {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&note);
    }
    text
}

fn history_message(m: &serde_json::Value) -> Option<HistoryMessage> {
    let role = m.get("role")?.as_str()?;
    match role {
        "system" => None,
        "user" => Some(HistoryMessage {
            role: "user".to_string(),
            text: content_text(m.get("content").unwrap_or(&serde_json::Value::Null)),
        }),
        "assistant" => {
            let text = content_text(m.get("content").unwrap_or(&serde_json::Value::Null));
            if text.is_empty() {
                None
            } else {
                Some(HistoryMessage {
                    role: "agent".to_string(),
                    text,
                })
            }
        }
        "toolResult" | "bashExecution" => Some(HistoryMessage {
            role: "tool".to_string(),
            text: content_text(m.get("content").unwrap_or(&serde_json::Value::Null)),
        }),
        "custom" if m.get("display").and_then(|d| d.as_bool()).unwrap_or(false) => {
            Some(HistoryMessage {
                role: "system".to_string(),
                text: content_text(m.get("content").unwrap_or(&serde_json::Value::Null)),
            })
        }
        "branchSummary" | "compactionSummary" => Some(HistoryMessage {
            role: "system".to_string(),
            text: format!(
                "[context summary] {}",
                m.get("summary").and_then(|s| s.as_str()).unwrap_or("")
            ),
        }),
        _ => Some(HistoryMessage {
            role: role.to_string(),
            text: content_text(m.get("content").unwrap_or(&serde_json::Value::Null)),
        }),
    }
}

/// Outcome of a direct `bash` RPC command (no LLM run).
#[derive(Clone, Debug)]
pub struct BashResult {
    pub output: String,
    pub exit_code: Option<i64>,
    pub cancelled: bool,
    pub truncated: bool,
}

/// Byte transport, split into an owned reader half and an owned writer
/// half. The halves MUST be independent objects: the reader thread blocks
/// in `recv` while commands send, so sharing one lock across both
/// deadlocks the first command (learned the hard way).
pub trait PiReader: Send + 'static {
    /// Next record line (`None` = clean EOF).
    fn recv(&mut self) -> io::Result<Option<String>>;
}

pub trait PiWriter: Send + 'static {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()>;
    /// Break the connection so a blocked reader observes EOF/error.
    /// Without this, dropping the client leaves the reader thread (and
    /// any peer waiting for OUR EOF) hanging on an open duplicate fd.
    fn shutdown(&mut self);
}

pub trait PiTransport: Send + 'static {
    fn split(self) -> (Box<dyn PiReader>, Box<dyn PiWriter>);
}

/// Child-process stdio transport (`pi --mode rpc ...`).
pub struct ChildTransport {
    child: std::process::Child,
}

pub struct ChildReader {
    reader: BufReader<std::process::ChildStdout>,
}

pub struct ChildWriter {
    writer: std::process::ChildStdin,
    child: std::process::Child,
}

impl ChildTransport {
    pub fn spawn(exe: &str, args: &[&str], cwd: Option<&str>) -> io::Result<Self> {
        let mut cmd = std::process::Command::new(exe);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        Ok(Self {
            child: cmd.spawn()?,
        })
    }
}

impl PiTransport for ChildTransport {
    fn split(mut self) -> (Box<dyn PiReader>, Box<dyn PiWriter>) {
        let stdout = self.child.stdout.take().expect("piped stdout");
        let stdin = self.child.stdin.take().expect("piped stdin");
        (
            Box::new(ChildReader {
                reader: BufReader::new(stdout),
            }),
            Box::new(ChildWriter {
                writer: stdin,
                child: self.child,
            }),
        )
    }
}

impl PiReader for ChildReader {
    fn recv(&mut self) -> io::Result<Option<String>> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => Ok(None),
            Ok(_) => Ok(Some(line)),
            Err(e) => Err(e),
        }
    }
}

impl PiWriter for ChildWriter {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }

    fn shutdown(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for ChildWriter {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// TCP transport (scripted tests; also usable for future TCP bridges).
pub struct TcpTransport {
    stream: std::net::TcpStream,
}

pub struct TcpReader {
    reader: BufReader<std::net::TcpStream>,
}

pub struct TcpWriter {
    writer: std::net::TcpStream,
}

impl TcpTransport {
    pub fn connect(addr: &std::net::SocketAddr) -> io::Result<Self> {
        let stream = std::net::TcpStream::connect(addr)?;
        stream.set_nodelay(true).ok();
        Ok(Self { stream })
    }

    pub fn from_stream(stream: std::net::TcpStream) -> Self {
        Self { stream }
    }
}

impl PiTransport for TcpTransport {
    fn split(self) -> (Box<dyn PiReader>, Box<dyn PiWriter>) {
        let writer = self.stream.try_clone().expect("tcp try_clone for split io");
        (
            Box::new(TcpReader {
                reader: BufReader::new(self.stream),
            }),
            Box::new(TcpWriter { writer }),
        )
    }
}

impl PiReader for TcpReader {
    fn recv(&mut self) -> io::Result<Option<String>> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => Ok(None),
            Ok(_) => Ok(Some(line)),
            Err(e) => Err(e),
        }
    }
}

impl PiWriter for TcpWriter {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }

    fn shutdown(&mut self) {
        let _ = self.writer.shutdown(std::net::Shutdown::Both);
    }
}

type PendingMap = Arc<Mutex<HashMap<String, mpsc::Sender<serde_json::Value>>>>;

/// A connected Pi session: correlated commands plus a streamed event inbox.
/// Single-run at a time (Pi itself serializes runs; concurrent prompts are
/// refused by checking `is_streaming` first is the caller's job).
pub struct PiClient {
    writer: Arc<Mutex<Box<dyn PiWriter>>>,
    pending: PendingMap,
    events: mpsc::Receiver<PiEvent>,
    next_id: u64,
    /// Set false by the reader thread when the transport dies.
    alive: Arc<AtomicBool>,
    malformed: Arc<AtomicU64>,
}

impl PiClient {
    /// Take over an open transport and start the reader thread. Verifies
    /// nothing yet — call `get_state` to prove liveness.
    pub fn connect<T: PiTransport>(transport: T) -> Self {
        let (read, write) = transport.split();
        let writer: Arc<Mutex<Box<dyn PiWriter>>> = Arc::new(Mutex::new(write));
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let (event_tx, events) = mpsc::channel::<PiEvent>();
        let alive = Arc::new(AtomicBool::new(true));
        let malformed = Arc::new(AtomicU64::new(0));
        let pending_r = Arc::clone(&pending);
        let alive_r = Arc::clone(&alive);
        let malformed_r = Arc::clone(&malformed);
        std::thread::spawn(move || {
            reader_loop(read, pending_r, event_tx, alive_r, malformed_r);
        });
        Self {
            writer,
            pending,
            events,
            next_id: 1,
            alive,
            malformed,
        }
    }

    /// Spawn `pi --mode rpc ...` and connect. Mirrors the Agent X launch
    /// contract: `--mode rpc --no-extensions --no-skills` plus either
    /// `--no-session` (ephemeral) or a session dir + id (persistent).
    /// Spawn `exe` with the given args, working around Windows `.cmd`
    /// shims (Rust cannot spawn those directly): on `NotFound`, retry once
    /// through `cmd /C`. The command line stays visible either way.
    fn spawn_child(exe: &str, args: &[&str], cwd: Option<&str>) -> io::Result<ChildTransport> {
        match ChildTransport::spawn(exe, args, cwd) {
            Ok(t) => Ok(t),
            Err(e) if e.kind() == io::ErrorKind::NotFound && cfg!(windows) => {
                let mut full = vec!["/C", exe];
                full.extend_from_slice(args);
                ChildTransport::spawn("cmd", &full, cwd)
            }
            Err(e) => Err(e),
        }
    }

    pub fn spawn_ephemeral(exe: &str, cwd: Option<&str>) -> io::Result<Self> {
        let t = Self::spawn_child(
            exe,
            &[
                "--mode",
                "rpc",
                "--no-extensions",
                "--no-skills",
                "--no-session",
            ],
            cwd,
        )?;
        Ok(Self::connect(t))
    }

    pub fn spawn_session(
        exe: &str,
        cwd: Option<&str>,
        session_dir: &str,
        session_id: &str,
    ) -> io::Result<Self> {
        let t = Self::spawn_child(
            exe,
            &[
                "--mode",
                "rpc",
                "--no-extensions",
                "--no-skills",
                "--session-dir",
                session_dir,
                "--session-id",
                session_id,
            ],
            cwd,
        )?;
        Ok(Self::connect(t))
    }

    pub fn malformed_lines(&self) -> u64 {
        self.malformed.load(Ordering::Relaxed)
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    fn next_id(&mut self) -> String {
        let id = format!("h{:06}", self.next_id);
        self.next_id += 1;
        id
    }

    fn command(
        &mut self,
        cmd: &str,
        fields: &[(&str, serde_json::Value)],
        timeout: Duration,
    ) -> Result<serde_json::Value, PiError> {
        if !self.is_alive() {
            return Err(PiError::Closed);
        }
        let id = self.next_id();
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|e| PiError::Transport(format!("pending lock: {e}")))?
            .insert(id.clone(), tx);
        let bytes = framing::encode(cmd, &id, fields);
        let send_result = self
            .writer
            .lock()
            .map_err(|e| PiError::Transport(format!("writer lock: {e}")))?
            .send(&bytes);
        if let Err(e) = send_result {
            self.pending.lock().ok().map(|mut p| p.remove(&id));
            return Err(PiError::Transport(format!("write: {e}")));
        }
        let value = match rx.recv_timeout(timeout) {
            Ok(v) => v,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.pending.lock().ok().map(|mut p| p.remove(&id));
                return Err(PiError::Timeout(format!("command {cmd} ({timeout:?})")));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(PiError::Closed),
        };
        let data = framing::is_success_response(&value).map_err(|e| PiError::Command {
            command: cmd.to_string(),
            error: e,
        })?;
        Ok(data.clone())
    }

    /// Drain one event, waiting up to `timeout`.
    pub fn next_event(&mut self, timeout: Duration) -> Result<PiEvent, PiError> {
        self.events.recv_timeout(timeout).map_err(|_| {
            if !self.is_alive() {
                PiError::Closed
            } else {
                PiError::Timeout("event wait".to_string())
            }
        })
    }

    pub fn get_state(&mut self, timeout: Duration) -> Result<EngineState, PiError> {
        Ok(EngineState::from_data(&self.command(
            "get_state",
            &[],
            timeout,
        )?))
    }

    pub fn get_available_models(&mut self, timeout: Duration) -> Result<Vec<ModelOption>, PiError> {
        let data = self.command("get_available_models", &[], timeout)?;
        let arr = data.as_array().cloned().unwrap_or_default();
        Ok(arr
            .iter()
            .filter_map(|m| {
                Some(ModelOption {
                    provider: m.get("provider")?.as_str()?.to_string(),
                    id: m.get("id")?.as_str()?.to_string(),
                    name: m
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
            })
            .collect())
    }

    pub fn set_model(
        &mut self,
        provider: &str,
        id: &str,
        timeout: Duration,
    ) -> Result<(), PiError> {
        self.command(
            "set_model",
            &[
                ("provider", serde_json::Value::String(provider.to_string())),
                ("modelId", serde_json::Value::String(id.to_string())),
            ],
            timeout,
        )?;
        Ok(())
    }

    pub fn get_available_thinking_levels(
        &mut self,
        timeout: Duration,
    ) -> Result<Vec<String>, PiError> {
        let data = self.command("get_available_thinking_levels", &[], timeout)?;
        Ok(data
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect())
    }

    pub fn set_thinking_level(&mut self, level: &str, timeout: Duration) -> Result<(), PiError> {
        self.command(
            "set_thinking_level",
            &[("level", serde_json::Value::String(level.to_string()))],
            timeout,
        )?;
        Ok(())
    }

    /// Send a prompt. Returns the disposition: `"handled"` means NO run
    /// follows (do not wait for `agent_settled`); anything else means a run
    /// started — stream deltas and wait for settle yourself, or use
    /// `prompt_and_wait`.
    pub fn send_prompt(&mut self, message: &str, timeout: Duration) -> Result<String, PiError> {
        if message.is_empty() {
            return Err(PiError::Protocol("empty prompt".to_string()));
        }
        let data = self.command(
            "prompt",
            &[("message", serde_json::Value::String(message.to_string()))],
            timeout,
        )?;
        Ok(data
            .get("disposition")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string())
    }

    /// Send a prompt, collect streamed text, resolve at `agent_settled`.
    /// `progress` receives deltas and tool notes live in wire order
    /// (feed the conversation view). `idle_timeout` bounds the whole
    /// wait, not each delta.
    pub fn prompt_and_wait(
        &mut self,
        message: &str,
        progress: &mut dyn FnMut(PiProgress),
        idle_timeout: Duration,
    ) -> Result<(String, bool), PiError> {
        let deadline = Instant::now() + idle_timeout;
        let disposition = self.send_prompt(message, idle_timeout)?;
        if disposition == "handled" {
            return Ok((String::new(), false));
        }
        let mut text = String::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(PiError::Timeout("prompt_and_wait settled".to_string()));
            }
            match self.next_event(left)? {
                PiEvent::TextDelta(d) => {
                    text.push_str(&d);
                    progress(PiProgress::Delta(d));
                }
                PiEvent::ToolNote(n) => {
                    progress(PiProgress::ToolNote(n));
                }
                PiEvent::Settled { aborted } => return Ok((text, aborted)),
                PiEvent::TransportClosed => return Err(PiError::Closed),
                PiEvent::Raw(_) => {}
            }
        }
    }

    pub fn abort(&mut self, timeout: Duration) -> Result<(), PiError> {
        self.command("abort", &[], timeout)?;
        Ok(())
    }

    pub fn get_messages(&mut self, timeout: Duration) -> Result<serde_json::Value, PiError> {
        self.command("get_messages", &[], timeout)
    }

    /// Fetch restored conversation history as displayable (role, text)
    /// pairs. Shape mirrors Agent X's `ParseHistory` tolerance: unknown
    /// roles and content shapes are skipped or summarized, never fatal;
    /// `system` boilerplate is dropped.
    pub fn get_history(&mut self, timeout: Duration) -> Result<Vec<HistoryMessage>, PiError> {
        let data = self.get_messages(timeout)?;
        let empty = Vec::new();
        let messages = data
            .get("messages")
            .and_then(|m| m.as_array())
            .unwrap_or(&empty);
        Ok(messages.iter().filter_map(history_message).collect())
    }

    pub fn run_bash(
        &mut self,
        command_line: &str,
        timeout: Duration,
    ) -> Result<BashResult, PiError> {
        let data = self.command(
            "bash",
            &[(
                "command",
                serde_json::Value::String(command_line.to_string()),
            )],
            timeout,
        )?;
        let s = |k: &str| {
            data.get(k)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        Ok(BashResult {
            output: s("output"),
            exit_code: data.get("exitCode").and_then(|v| v.as_i64()),
            cancelled: data
                .get("cancelled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            truncated: data
                .get("truncated")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        })
    }
}

impl Drop for PiClient {
    fn drop(&mut self) {
        // Break the connection first: the reader thread may be blocked
        // in recv, and the peer may be waiting for OUR EOF. Without
        // this, drop-then-join deadlocks (caught by the fake-server test).
        if let Ok(mut w) = self.writer.lock() {
            w.shutdown();
        }
    }
}

fn reader_loop(
    mut read: Box<dyn PiReader>,
    pending: PendingMap,
    events: mpsc::Sender<PiEvent>,
    alive: Arc<AtomicBool>,
    malformed: Arc<AtomicU64>,
) {
    loop {
        let line = read.recv();
        let line = match line {
            Ok(Some(l)) => l,
            Ok(None) => break, // clean EOF
            Err(_) => break,   // transport error
        };
        if line.trim().is_empty() {
            continue;
        }
        let record = match framing::decode(&line) {
            Ok(r) => r,
            Err(_) => {
                malformed.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };
        match record {
            Record::Response { id, value } => {
                if let Some(i) = id {
                    let tx = pending.lock().ok().and_then(|mut p| p.remove(&i));
                    if let Some(tx) = tx {
                        let _ = tx.send(value);
                    } else {
                        let _ = events.send(PiEvent::Raw(value));
                    }
                } else {
                    let _ = events.send(PiEvent::Raw(value));
                }
            }
            Record::TextDelta(d) => {
                let _ = events.send(PiEvent::TextDelta(d));
            }
            Record::Settled { aborted } => {
                let _ = events.send(PiEvent::Settled { aborted });
            }
            Record::ToolNote(n) => {
                let _ = events.send(PiEvent::ToolNote(n));
            }
            Record::Event { value, .. } => {
                let _ = events.send(PiEvent::Raw(value));
            }
        }
    }
    // Transport is gone: mark dead (waiters observe Closed instead of
    // hanging), drop all pending senders (their receivers fail fast),
    // then report closure to event consumers.
    alive.store(false, Ordering::Relaxed);
    if let Ok(mut p) = pending.lock() {
        p.clear();
    }
    let _ = events.send(PiEvent::TransportClosed);
}
