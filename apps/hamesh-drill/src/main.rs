//! Standing external-CLI regression drill: an outside operator drives the
//! live window through the inspector protocol only — the same narrow door
//! any agent uses. It maps, submits through a stale revision on purpose
//! (the classic outside-operator mistake), repairs by re-inspecting, and
//! verifies the submission landed. No shared memory, no model, no network:
//! fully deterministic.
//!
//! LOCAL-ONLY: opens a real window, needs a desktop. Never run in CI.
//! Run from the workspace root: `cargo run -p hamesh-drill`
//! Exit 0 + DRILL-PASS, or nonzero + DRILL-FAIL <reason>.

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const SMOKE_SECS: u64 = 60;
const DISCOVERY: &str = "target/hamesh-inspect.json";
const TRACE: &str = "target/hamesh-window-trace.json";
const SNAPSHOT: &str = "target/hamesh-drill-snapshot.png";
const WINDOW_LOG: &str = "target/hamesh-drill-window.log";
const DRILL_TEXT: &str = "external drill";

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

/// Raw CLI run without the window-liveness guard (for background threads;
/// `Child` can't be shared across them — a dead window surfaces here as a
/// connect failure instead).
fn cli_raw(args: &[&str]) -> (i32, String) {
    let out = Command::new(exe("hamesh"))
        .args(args)
        .output()
        .unwrap_or_else(|e| fail(format!("failed to run hamesh CLI: {e}")));
    let code = out.status.code().unwrap_or(1);
    (code, String::from_utf8_lossy(&out.stdout).to_string())
}

/// Run the external CLI once; return (exit code, stdout).
fn cli(child: &mut Child, args: &[&str]) -> (i32, String) {
    if let Ok(Some(status)) = child.try_wait() {
        fail(format!("window exited early with {status} before CLI step"));
    }
    cli_raw(args)
}

fn app_rev(out: &str) -> u64 {
    let v: serde_json::Value =
        serde_json::from_str(out).unwrap_or_else(|_| fail(format!("CLI reply not JSON: {out}")));
    v.get("app_rev")
        .and_then(|r| r.as_u64())
        .unwrap_or_else(|| fail(format!("CLI reply has no app_rev: {out}")))
}

fn code_is(out: &str, want: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(out)
        .ok()
        .and_then(|v| v.get("code")?.as_str().map(|s| s.to_string()))
        .as_deref()
        == Some(want)
}

fn main() {
    if !std::path::Path::new("Cargo.toml").exists()
        || !std::path::Path::new("apps/hamesh-drill").exists()
    {
        fail("run from the workspace root (cargo run -p hamesh-drill)".to_string());
    }

    step(0, "build window + CLI");
    let st = Command::new("cargo")
        .args(["build", "-q", "-p", "hamesh-window", "-p", "hamesh-cli"])
        .status()
        .unwrap_or_else(|e| fail(format!("cargo build failed to start: {e}")));
    if !st.success() {
        fail("cargo build -p hamesh-window -p hamesh-cli failed".to_string());
    }

    // A stale discovery file would connect the drill to a dead run.
    let _ = std::fs::remove_file(DISCOVERY);

    step(1, "open window --m3 --inspect (smoke clock)");
    let log = std::fs::File::create(WINDOW_LOG)
        .unwrap_or_else(|e| fail(format!("cannot create {WINDOW_LOG}: {e}")));
    let mut child: Child = Command::new(exe("hamesh-window-demo"))
        .args(["--m3", "--inspect", &format!("--smoke-test={SMOKE_SECS}")])
        .stdout(Stdio::from(
            log.try_clone()
                .unwrap_or_else(|e| fail(format!("log clone failed: {e}"))),
        ))
        .stderr(Stdio::from(log))
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
        if let Ok(Some(status)) = child.try_wait() {
            fail(format!("window exited during discovery with {status}"));
        }
        if t0.elapsed() > Duration::from_secs(30) {
            fail("no discovery file after 30s".to_string());
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    step(3, "map: learn rev + find composer");
    let (code, out) = cli(&mut child, &["map"]);
    if code != 0 {
        fail(format!("map failed with code {code}: {out}"));
    }
    let r1 = app_rev(&out);
    if !out.contains("composer") {
        fail("map has no composer node".to_string());
    }

    step(4, "type into composer by id (no focus tricks)");
    let (code, out) = cli(&mut child, &["type", "composer", DRILL_TEXT]);
    if code != 0 {
        fail(format!("type failed with code {code}: {out}"));
    }
    let r2 = app_rev(&out);
    if r2 <= r1 {
        fail(format!("type did not advance rev: {r1} -> {r2}"));
    }

    step(5, "submit with a STALE rev on purpose (the fault)");
    let (code, out) = cli(
        &mut child,
        &["--expect-rev", &r1.to_string(), "key", "Enter"],
    );
    if code != 2 || !code_is(&out, "STALE") {
        fail(format!(
            "stale Enter should be rejected STALE/2, got {code}: {out}"
        ));
    }

    step(6, "repair: re-map, confirm the rejected op changed nothing");
    let (code, out) = cli(&mut child, &["map"]);
    if code != 0 {
        fail(format!("re-map failed with code {code}: {out}"));
    }
    let r3 = app_rev(&out);
    if r3 != r2 {
        fail(format!("rejected op must not change rev: {r2} vs {r3}"));
    }

    step(7, "retry submit with the fresh rev");
    let (code, out) = cli(
        &mut child,
        &["--expect-rev", &r3.to_string(), "key", "Enter"],
    );
    if code != 0 {
        fail(format!("fresh Enter failed with code {code}: {out}"));
    }
    let r4 = app_rev(&out);
    if r4 <= r3 {
        fail(format!("submit did not advance rev: {r3} -> {r4}"));
    }

    step(8, "snapshot evidence");
    let (code, out) = cli(&mut child, &["snapshot", SNAPSHOT]);
    if code != 0 {
        fail(format!("snapshot failed with code {code}: {out}"));
    }
    let meta =
        std::fs::metadata(SNAPSHOT).unwrap_or_else(|e| fail(format!("snapshot file missing: {e}")));
    if meta.len() == 0 {
        fail("snapshot file is empty".to_string());
    }

    step(9, "wait op: immediate, timeout, and wake-on-change");
    // Already past r3: immediate Ok with waited:false.
    let (code, out) = cli(&mut child, &["wait", &r3.to_string(), "3000"]);
    if code != 0 {
        fail(format!("immediate wait failed with code {code}: {out}"));
    }
    let wv: serde_json::Value =
        serde_json::from_str(&out).unwrap_or_else(|_| fail(format!("wait reply not JSON: {out}")));
    if wv
        .get("result")
        .and_then(|r| r.get("waited"))
        .and_then(|w| w.as_bool())
        != Some(false)
    {
        fail(format!("expected immediate waited:false: {out}"));
    }
    if app_rev(&out) != r4 {
        fail(format!("wait should report current rev {r4}: {out}"));
    }
    // Parked with nothing changing: TIMEOUT (CLI exit 1, code TIMEOUT).
    let (code, out) = cli(&mut child, &["wait", &r4.to_string(), "2500"]);
    if code != 1 || !code_is(&out, "TIMEOUT") {
        fail(format!(
            "parked wait should time out (exit 1/TIMEOUT), got {code}: {out}"
        ));
    }
    // Parked then woken: background waiter, main thread moves the rev.
    let r4s = r4.to_string();
    let waiter = std::thread::spawn(move || cli_raw(&["wait", r4s.as_str(), "15000"]));
    std::thread::sleep(Duration::from_secs(1));
    let (code, out) = cli(&mut child, &["type", "composer", "wait probe"]);
    if code != 0 {
        fail(format!("wake-up type failed with code {code}: {out}"));
    }
    let r5 = app_rev(&out);
    if r5 <= r4 {
        fail(format!("wake-up type did not advance rev: {r4} -> {r5}"));
    }
    let (code, out) = waiter
        .join()
        .unwrap_or_else(|_| fail("waiter thread panicked".to_string()));
    if code != 0 {
        fail(format!("woken wait failed with code {code}: {out}"));
    }
    let wv: serde_json::Value =
        serde_json::from_str(&out).unwrap_or_else(|_| fail(format!("wait reply not JSON: {out}")));
    if wv
        .get("result")
        .and_then(|r| r.get("waited"))
        .and_then(|w| w.as_bool())
        != Some(true)
    {
        fail(format!("expected waited:true after the rev moved: {out}"));
    }
    if app_rev(&out) != r5 {
        fail(format!("woken wait should report rev {r5}: {out}"));
    }

    step(10, "race: two submits, one rev — exactly one may win");
    // Two racing Enter presses on the same rev prove the UI thread is
    // still the serializer after the per-connection-threads fix: one
    // applies (rev+1), the other is rejected STALE. Either may win, but
    // never both and never neither — that is the whole assertion.
    let (code, out) = cli(&mut child, &["type", "composer", "race probe"]);
    if code != 0 {
        fail(format!("race setup type failed with code {code}: {out}"));
    }
    let r6 = app_rev(&out);
    if r6 <= r5 {
        fail(format!("race setup type did not advance rev: {r5} -> {r6}"));
    }
    let r6a = r6.to_string();
    let r6b = r6.to_string();
    let t1 = std::thread::spawn(move || cli_raw(&["--expect-rev", r6a.as_str(), "key", "Enter"]));
    let t2 = std::thread::spawn(move || cli_raw(&["--expect-rev", r6b.as_str(), "key", "Enter"]));
    let (c1, o1) = t1
        .join()
        .unwrap_or_else(|_| fail("racer thread panicked".to_string()));
    let (c2, o2) = t2
        .join()
        .unwrap_or_else(|_| fail("racer thread panicked".to_string()));
    let won = [(c1, &o1), (c2, &o2)]
        .iter()
        .filter(|(c, o)| *c == 0 && o.contains("submitted"))
        .count();
    let stale = [(c1, &o1), (c2, &o2)]
        .iter()
        .filter(|(c, o)| *c == 2 && code_is(o, "STALE"))
        .count();
    if won != 1 || stale != 1 {
        fail(format!(
            "race must yield exactly one winner and one STALE (got {won}/{stale}): [{c1}] {o1} [{c2}] {o2}"
        ));
    }
    let (code, out) = cli(&mut child, &["map"]);
    if code != 0 {
        fail(format!("post-race map failed with code {code}: {out}"));
    }
    let r7 = app_rev(&out);
    if r7 != r6 + 1 {
        fail(format!("exactly one racer may apply: rev {r6} -> {r7}"));
    }

    step(
        11,
        "wait for the smoke clock, then check the window's own trace",
    );
    let status = child
        .wait()
        .unwrap_or_else(|e| fail(format!("waiting for window failed: {e}")));
    if !status.success() {
        fail(format!("window exited with {status}"));
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

    println!("DRILL-PASS stale-inject -> repair -> submit -> wait -> race verified (revs {r1}->{r2}->{r3}->{r4}->{r5}->{r6}->{r7})");
}
