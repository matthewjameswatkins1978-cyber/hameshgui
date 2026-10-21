//! Standing MCP drill: the bridge regression-covered without a human.
//!
//! Spawns the live window, then drives it through `hamesh-mcp` over stdio
//! JSON-RPC — the exact session script an MCP client runs, asserted step
//! by step: handshake, tool list, map, type, ordered submit, stale
//! rejection in-band, snapshot, clean shutdown, and the window's own
//! trace as the independent witness. No model, no network: deterministic.
//!
//! LOCAL-ONLY: opens a real window, needs a desktop. Never run in CI.
//! Run from the workspace root: `cargo run -p hamesh-mcp-drill`
//! Exit 0 + DRILL-PASS, or nonzero + DRILL-FAIL <reason>.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const SMOKE_SECS: u64 = 60;
const DISCOVERY: &str = "target/hamesh-inspect.json";
const TRACE: &str = "target/hamesh-window-trace.json";
const SNAPSHOT: &str = "target/hamesh-mcp-drill-snapshot.png";
const DRILL_TEXT: &str = "mcp drill";

fn exe(name: &str) -> String {
    format!("target/debug/{name}{}", std::env::consts::EXE_SUFFIX)
}

fn fail(reason: String) -> ! {
    println!("DRILL-FAIL {reason}");
    std::process::exit(1);
}

fn step(n: usize, name: &str) {
    println!("DRILL-STEP {n}: {name}");
}

/// One JSON-RPC round-trip over the MCP child's stdio: write one line,
/// read lines until the matching id arrives.
fn rpc(
    stdin: &mut std::process::ChildStdin,
    stdout: &mut BufReader<std::process::ChildStdout>,
    id: u64,
    method: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    let line = if params.is_null() {
        serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method}).to_string()
    } else {
        serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
            .to_string()
    };
    writeln!(stdin, "{line}").unwrap_or_else(|e| fail(format!("mcp stdin write failed: {e}")));
    stdin
        .flush()
        .unwrap_or_else(|e| fail(format!("mcp stdin flush failed: {e}")));
    let t0 = Instant::now();
    let mut buf = String::new();
    loop {
        if t0.elapsed() > Duration::from_secs(25) {
            fail(format!("no MCP response for id {id} within 25s"));
        }
        buf.clear();
        match stdout.read_line(&mut buf) {
            Ok(0) => fail(format!("mcp stdout closed waiting for id {id}")),
            Ok(_) => {}
            Err(e) => fail(format!("mcp stdout read failed: {e}")),
        }
        let v: serde_json::Value =
            serde_json::from_str(buf.trim()).unwrap_or(serde_json::Value::Null);
        if v.get("id").and_then(|i| i.as_u64()) == Some(id) {
            return v;
        }
        // Anything else (shouldn't happen: the server sends no
        // notifications) is ignored so a stray line can't jam the drill.
    }
}

/// Unwrap a tools/call result: the bridge returns the inspector reply as
/// the single text content item; parse it back to JSON.
fn tool_json(resp: &serde_json::Value, what: &str) -> serde_json::Value {
    let text = resp
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.get(0))
        .and_then(|i| i.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or_else(|| fail(format!("{what}: no text content: {resp}")));
    serde_json::from_str(text).unwrap_or_else(|_| fail(format!("{what}: not JSON: {text}")))
}

fn main() {
    if !std::path::Path::new("Cargo.toml").exists()
        || !std::path::Path::new("apps/hamesh-mcp-drill").exists()
    {
        fail("run from the workspace root (cargo run -p hamesh-mcp-drill)".to_string());
    }

    step(0, "build window + MCP bridge");
    let st = Command::new("cargo")
        .args(["build", "-q", "-p", "hamesh-window", "-p", "hamesh-mcp"])
        .status()
        .unwrap_or_else(|e| fail(format!("cargo build failed to start: {e}")));
    if !st.success() {
        fail("cargo build -p hamesh-window -p hamesh-mcp failed".to_string());
    }
    let _ = std::fs::remove_file(DISCOVERY);

    step(1, "open window --m3 --inspect (smoke clock)");
    let wlog = std::fs::File::create("target/hamesh-mcp-drill-window.log")
        .unwrap_or_else(|e| fail(format!("cannot create window log: {e}")));
    let mut window: Child = Command::new(exe("hamesh-window-demo"))
        .args(["--m3", "--inspect", &format!("--smoke-test={SMOKE_SECS}")])
        .stdout(Stdio::from(
            wlog.try_clone()
                .unwrap_or_else(|e| fail(format!("log clone failed: {e}"))),
        ))
        .stderr(Stdio::from(wlog))
        .spawn()
        .unwrap_or_else(|e| fail(format!("failed to spawn window: {e}")));

    step(2, "wait for inspector discovery");
    let t0 = Instant::now();
    loop {
        if let Ok(raw) = std::fs::read_to_string(DISCOVERY) {
            if raw.contains("\"port\"") {
                break;
            }
        }
        if let Ok(Some(status)) = window.try_wait() {
            fail(format!("window exited during discovery with {status}"));
        }
        if t0.elapsed() > Duration::from_secs(30) {
            fail("no discovery file after 30s".to_string());
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    step(3, "spawn the bridge, handshake");
    let mlog = std::fs::File::create("target/hamesh-mcp-drill-mcp.log")
        .unwrap_or_else(|e| fail(format!("cannot create mcp log: {e}")));
    let mut mcp: Child = Command::new(exe("hamesh-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(mlog))
        .spawn()
        .unwrap_or_else(|e| fail(format!("failed to spawn hamesh-mcp: {e}")));
    let mut stdin = mcp
        .stdin
        .take()
        .unwrap_or_else(|| fail("no mcp stdin".to_string()));
    let mut stdout = BufReader::new(
        mcp.stdout
            .take()
            .unwrap_or_else(|| fail("no mcp stdout".to_string())),
    );
    let init = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "initialize",
        serde_json::json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "drill", "version": "0.1"}}),
    );
    if init.get("result").is_none() {
        fail(format!("initialize failed: {init}"));
    }
    writeln!(
        stdin,
        "{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}}"
    )
    .unwrap_or_else(|e| fail(format!("initialized notify failed: {e}")));
    stdin
        .flush()
        .unwrap_or_else(|e| fail(format!("flush failed: {e}")));

    step(4, "tools/list: all 7 tools advertised");
    let list = rpc(
        &mut stdin,
        &mut stdout,
        2,
        "tools/list",
        serde_json::Value::Null,
    );
    let names: Vec<String> = list
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|t| {
                    t.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_else(|| fail(format!("tools/list malformed: {list}")));
    for want in [
        "hamesh_map",
        "hamesh_detail",
        "hamesh_click",
        "hamesh_key",
        "hamesh_type",
        "hamesh_snapshot",
        "hamesh_wait",
    ] {
        if !names.contains(&want.to_string()) {
            fail(format!("tools/list missing {want}: {names:?}"));
        }
    }

    let call = |stdin: &mut std::process::ChildStdin,
                stdout: &mut BufReader<std::process::ChildStdout>,
                id: u64,
                name: &str,
                args: serde_json::Value| {
        rpc(
            stdin,
            stdout,
            id,
            "tools/call",
            serde_json::json!({"name": name, "arguments": args}),
        )
    };

    step(5, "map: learn rev + find composer");
    let map = tool_json(
        &call(
            &mut stdin,
            &mut stdout,
            3,
            "hamesh_map",
            serde_json::json!({}),
        ),
        "map",
    );
    let r1 = map
        .get("app_rev")
        .and_then(|r| r.as_u64())
        .unwrap_or_else(|| fail(format!("map has no app_rev: {map}")));
    if !map.to_string().contains("composer") {
        fail("map has no composer node".to_string());
    }

    step(6, "type into composer by id");
    let typed = tool_json(
        &call(
            &mut stdin,
            &mut stdout,
            4,
            "hamesh_type",
            serde_json::json!({"id": "composer", "text": DRILL_TEXT}),
        ),
        "type",
    );
    let r2 = typed
        .get("app_rev")
        .and_then(|r| r.as_u64())
        .unwrap_or_else(|| fail(format!("type has no app_rev: {typed}")));
    if r2 <= r1 {
        fail(format!("type did not advance rev: {r1} -> {r2}"));
    }

    step(7, "stale key rejected in-band (the fault)");
    let stale = tool_json(
        &call(
            &mut stdin,
            &mut stdout,
            5,
            "hamesh_key",
            serde_json::json!({"key": "Enter", "expect_rev": r1}),
        ),
        "stale key",
    );
    if stale.get("code").and_then(|c| c.as_str()) != Some("STALE") {
        fail(format!(
            "stale Enter should come back STALE in-band: {stale}"
        ));
    }

    step(8, "repair: fresh rev, ordered submit");
    let fresh = tool_json(
        &call(
            &mut stdin,
            &mut stdout,
            6,
            "hamesh_map",
            serde_json::json!({}),
        ),
        "re-map",
    );
    let r3 = fresh
        .get("app_rev")
        .and_then(|r| r.as_u64())
        .unwrap_or_else(|| fail(format!("re-map has no app_rev: {fresh}")));
    if r3 != r2 {
        fail(format!("rejected op must not change rev: {r2} vs {r3}"));
    }
    let done = tool_json(
        &call(
            &mut stdin,
            &mut stdout,
            7,
            "hamesh_key",
            serde_json::json!({"key": "Enter", "expect_rev": r3}),
        ),
        "submit",
    );
    let r4 = done
        .get("app_rev")
        .and_then(|r| r.as_u64())
        .unwrap_or_else(|| fail(format!("submit has no app_rev: {done}")));
    if r4 <= r3 {
        fail(format!("submit did not advance rev: {r3} -> {r4}"));
    }
    let result = done
        .get("result")
        .and_then(|r| r.get("result"))
        .and_then(|r| r.as_str())
        .unwrap_or("");
    if !result.contains("submitted") || !result.contains(DRILL_TEXT) {
        fail(format!("submit reply lacks the drill text: {done}"));
    }

    step(9, "snapshot evidence, clean shutdown");
    let shot = tool_json(
        &call(
            &mut stdin,
            &mut stdout,
            8,
            "hamesh_snapshot",
            serde_json::json!({"path": SNAPSHOT}),
        ),
        "snapshot",
    );
    if shot.get("ok").and_then(|o| o.as_bool()) != Some(true) {
        fail(format!("snapshot not ok: {shot}"));
    }
    let meta =
        std::fs::metadata(SNAPSHOT).unwrap_or_else(|e| fail(format!("snapshot missing: {e}")));
    if meta.len() == 0 {
        fail("snapshot file is empty".to_string());
    }
    drop(stdin);
    // The bridge exits cleanly on stdin EOF; bound the wait so a hung
    // child fails the drill instead of hanging it.
    let t0 = Instant::now();
    let mcp_status = loop {
        if let Ok(Some(status)) = mcp.try_wait() {
            break status;
        }
        if t0.elapsed() > Duration::from_secs(10) {
            fail("mcp did not exit on stdin EOF".to_string());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    if !mcp_status.success() {
        fail(format!("mcp exited with {mcp_status}"));
    }

    step(10, "wait out the smoke clock, check the window's own trace");
    let window_status = window
        .wait()
        .unwrap_or_else(|e| fail(format!("waiting for window failed: {e}")));
    if !window_status.success() {
        fail(format!("window exited with {window_status}"));
    }
    let trace =
        std::fs::read_to_string(TRACE).unwrap_or_else(|e| fail(format!("no trace file: {e}")));
    let mut submitted = false;
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&trace) {
        if let Some(arr) = v.as_array() {
            for e in arr {
                let kind = e.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                let detail = e.get("detail").and_then(|d| d.as_str()).unwrap_or("");
                if kind == "m3.key" && detail.contains("submitted") && detail.contains(DRILL_TEXT) {
                    submitted = true;
                    break;
                }
            }
        }
    }
    if !submitted {
        fail("trace has no submitted record for the drill text".to_string());
    }

    println!("DRILL-PASS mcp handshake -> ordered submit -> stale repair verified (revs {r1}->{r2}->{r3}->{r4})");
}
