//! hamesh-mcp: the inspector protocol as MCP tools, over stdio.
//!
//! Any MCP-capable agent becomes a Hand: same seven ops as `hamesh-cli`
//! (`map detail click key type snapshot wait`), same rev stamps, same
//! stale discipline — translated 1:1, nothing invented. The server is a
//! thin forwarder: each tool call opens one TCP round-trip to the live
//! window (blocking, in `spawn_blocking`) and returns the inspector reply
//! JSON verbatim.
//!
//! Auth: the inspector discovery file plus env overrides. Resolution per
//! call, so a window restart (new port/token) needs no server restart:
//! explicit `port`/`token` args win, then `HAMESH_PORT`/`HAMESH_TOKEN`,
//! then `target/hamesh-inspect.json` relative to the server's cwd.
//! Run from the workspace root so discovery resolves.
//!
//! STALE discipline (also repeated in every mutating tool's description):
//! replies carry `app_rev`. Pass it back as `expect_rev` on the next
//! mutating call; if a reply comes back with `"code": "STALE"`, re-run
//! `hamesh_map` and retry with the fresh `app_rev`. Rejected ops change
//! nothing.
//!
//! LOCAL-ONLY, like the rest of the Eye/Hand path: the window it drives
//! is 127.0.0.1 dev-only, and MCP stdio gives the calling client full
//! control of the GUI. Never expose this server beyond the machine.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock},
    tool, tool_handler, tool_router, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::Deserialize;

/// Per-call connection: explicit args > env > discovery file.
#[derive(Debug, Deserialize, JsonSchema)]
struct Conn {
    /// Inspector TCP port. Omit to use HAMESH_PORT or the discovery file.
    port: Option<u16>,
    /// Per-run dev token. Omit to use HAMESH_TOKEN or the discovery file.
    token: Option<String>,
}

impl Conn {
    fn endpoint(&self) -> Result<(u16, String), String> {
        if let (Some(p), Some(t)) = (
            self.port.or_else(|| {
                std::env::var("HAMESH_PORT")
                    .ok()
                    .and_then(|s| s.parse().ok())
            }),
            self.token
                .clone()
                .or_else(|| std::env::var("HAMESH_TOKEN").ok()),
        ) {
            return Ok((p, t));
        }
        let raw = std::fs::read_to_string("target/hamesh-inspect.json").map_err(|_| {
            "no inspector found: pass port+token, set HAMESH_PORT/HAMESH_TOKEN, or run from the workspace root while the app runs with --inspect"
                .to_string()
        })?;
        let v: serde_json::Value =
            serde_json::from_str(&raw).map_err(|e| format!("bad discovery file: {e}"))?;
        match (
            v.get("port").and_then(|p| p.as_u64()),
            v.get("token").and_then(|t| t.as_str()),
        ) {
            (Some(p), Some(t)) => Ok((p as u16, t.to_string())),
            _ => Err("discovery file lacks port/token".to_string()),
        }
    }
}

/// One inspector round-trip. Returns the reply JSON (pretty) or a local
/// error object with the same shape (`ok: false`, `code`, message).
fn forward(op: &str, params: serde_json::Value, conn: &Conn) -> String {
    let reply = (|| -> Result<serde_json::Value, String> {
        let (port, token) = conn.endpoint()?;
        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .map_err(|e| format!("connect 127.0.0.1:{port} failed: {e}"))?;
        let req = serde_json::json!({"id": 1, "token": token, "op": op, "params": params});
        let bytes = serde_json::to_string(&req).map_err(|e| format!("encode: {e}"))?;
        stream
            .write_all(bytes.as_bytes())
            .and_then(|_| stream.write_all(b"\n"))
            .and_then(|_| stream.flush())
            .map_err(|e| format!("write: {e}"))?;
        let mut line = String::new();
        BufReader::new(stream.try_clone().map_err(|e| format!("clone: {e}"))?)
            .read_line(&mut line)
            .map_err(|e| format!("read: {e}"))?;
        serde_json::from_str(line.trim()).map_err(|e| format!("bad reply: {e}: {line}"))
    })();
    match reply {
        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".to_string()),
        Err(message) => {
            serde_json::json!({"ok": false, "code": "LOCAL", "message": message}).to_string()
        }
    }
}

/// The inspector protocol is stateful and ordered (revs!); MCP clients may
/// fire calls concurrently, so the bridge owns the ordering: one inspector
/// round-trip at a time, in arrival order (tokio's mutex is FIFO-fair).
/// A `wait` legitimately holds the lock — waiting means nothing else
/// should happen until the rev moves or the timeout elapses.
static INSPECTOR_LOCK: std::sync::LazyLock<tokio::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));

async fn call(op: &'static str, params: serde_json::Value, conn: Conn) -> CallToolResult {
    let _guard = INSPECTOR_LOCK.lock().await;
    let body = tokio::task::spawn_blocking(move || forward(op, params, &conn))
        .await
        .unwrap_or_else(|e| {
            serde_json::json!({"ok": false, "code": "LOCAL", "message": format!("join: {e}")})
                .to_string()
        });
    // Structural errors for MCP clients: the inspector JSON stays in
    // content either way, but STALE / DENIED / LOCAL now also raise
    // isError so clients with error handling trip without parsing.
    let ok = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("ok").and_then(|o| o.as_bool()))
        .unwrap_or(false);
    let content = vec![ContentBlock::text(body)];
    if ok {
        CallToolResult::success(content)
    } else {
        CallToolResult::error(content)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DetailArgs {
    #[serde(flatten)]
    conn: Conn,
    /// Component id from hamesh_map (e.g. "composer").
    id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ClickArgs {
    #[serde(flatten)]
    conn: Conn,
    /// Component id (click_id) or "x,y" pixels (click_at).
    target: String,
    /// app_rev from the last reply; see STALE rule.
    expect_rev: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct KeyArgs {
    #[serde(flatten)]
    conn: Conn,
    /// Tab ShiftTab Enter Escape Char:x Backspace Delete Left Right Up Down Home End.
    key: String,
    /// app_rev from the last reply; see STALE rule.
    expect_rev: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TypeArgs {
    #[serde(flatten)]
    conn: Conn,
    /// Component id (e.g. "composer").
    id: String,
    /// Full text to type into the component.
    text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SnapshotArgs {
    #[serde(flatten)]
    conn: Conn,
    /// PNG path. Defaults to target/hamesh-ext-snapshot.png.
    path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WaitArgs {
    #[serde(flatten)]
    conn: Conn,
    /// app_rev to wait past (0 = return current rev).
    app_rev: Option<u64>,
    /// Milliseconds to wait for a change (default 5000).
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone)]
struct HameshMcp {
    tool_router: ToolRouter<Self>,
}

impl HameshMcp {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for HameshMcp {}

#[tool_router(router = tool_router)]
impl HameshMcp {
    /// Read the live component tree: ids, roles, rects, rev stamps. Start
    /// every session here; the app_rev you get back anchors later calls.
    #[tool(name = "hamesh_map")]
    async fn map(&self, p: Parameters<Conn>) -> CallToolResult {
        call("map", serde_json::json!({}), p.0).await
    }

    /// Read one component in depth: props, state keys, diagnostics
    /// (disabled/invisible/clipped and why). Takes a component id.
    #[tool(name = "hamesh_detail")]
    async fn detail(&self, p: Parameters<DetailArgs>) -> CallToolResult {
        let a = p.0;
        call("detail", serde_json::json!({"id": a.id}), a.conn).await
    }

    /// Click a component by id, or at x,y pixels as "x,y". Mutating:
    /// pass app_rev back as expect_rev; on code STALE, re-run hamesh_map
    /// and retry with the fresh app_rev. Rejected ops change nothing.
    #[tool(name = "hamesh_click")]
    async fn click(&self, p: Parameters<ClickArgs>) -> CallToolResult {
        let a = p.0;
        let (op, params) = if let Some((x, y)) = a.target.split_once(',') {
            let x: f64 = x.trim().parse().unwrap_or(-1.0);
            let y: f64 = y.trim().parse().unwrap_or(-1.0);
            (
                "click_at",
                serde_json::json!({"x": x, "y": y, "expect_rev": a.expect_rev}),
            )
        } else {
            (
                "click_id",
                serde_json::json!({"id": a.target, "expect_rev": a.expect_rev}),
            )
        };
        call(op, params, a.conn).await
    }

    /// Press one key: Tab ShiftTab Enter Escape Char:x Backspace Delete
    /// Left Right Up Down Home End. Mutating: pass app_rev back as
    /// expect_rev; on code STALE, re-run hamesh_map and retry.
    #[tool(name = "hamesh_key")]
    async fn key(&self, p: Parameters<KeyArgs>) -> CallToolResult {
        let a = p.0;
        call(
            "key",
            serde_json::json!({"key": a.key, "expect_rev": a.expect_rev}),
            a.conn,
        )
        .await
    }

    /// Type full text into a component by id (no focus tricks needed).
    #[tool(name = "hamesh_type")]
    async fn type_text(&self, p: Parameters<TypeArgs>) -> CallToolResult {
        let a = p.0;
        call(
            "type",
            serde_json::json!({"id": a.id, "text": a.text}),
            a.conn,
        )
        .await
    }

    /// Render the current frame to a PNG; replies with the path + revs.
    #[tool(name = "hamesh_snapshot")]
    async fn snapshot(&self, p: Parameters<SnapshotArgs>) -> CallToolResult {
        let a = p.0;
        let path = a
            .path
            .unwrap_or_else(|| "target/hamesh-ext-snapshot.png".to_string());
        call("snapshot", serde_json::json!({"path": path}), a.conn).await
    }

    /// Block until app_rev changes or timeout_ms elapses (default 5000).
    /// Use it to wait for streamed output or settle loops.
    #[tool(name = "hamesh_wait")]
    async fn wait(&self, p: Parameters<WaitArgs>) -> CallToolResult {
        let a = p.0;
        call(
            "wait",
            serde_json::json!({"app_rev": a.app_rev.unwrap_or(0), "timeout_ms": a.timeout_ms.unwrap_or(5000)}),
            a.conn,
        )
        .await
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = HameshMcp::new().serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
