//! Standing Blocks regression drill: an outside operator verifies the
//! Agent X dashboard through the inspector protocol only — the same
//! narrow door any agent uses. It maps the composition, checks block
//! semantics (roles, Progress value, Button action), clicks Inspect
//! design through real hit-testing, proves the binding updated, and
//! confirms stale revisions are refused. No shared memory, no model,
//! no network: fully deterministic.
//!
//! LOCAL-ONLY: opens a real window, needs a desktop. Never run in CI.
//! Run from the workspace root: `cargo run -p hamesh-blocks-drill`
//! Exit 0 + DRILL-PASS, or nonzero + DRILL-FAIL <reason>.

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const SMOKE_SECS: u64 = 45;
const DISCOVERY: &str = "target/hamesh-inspect.json";
const TRACE: &str = "target/hamesh-window-trace.json";
const SNAPSHOT: &str = "target/hamesh-blocks-drill-snapshot.png";
const OVERLAY_SNAPSHOT: &str = "target/hamesh-blocks-drill-overlay.png";
const WINDOW_LOG: &str = "target/hamesh-blocks-drill-window.log";

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

fn cli(child: &mut Child, args: &[&str]) -> (i32, String) {
    if let Ok(Some(status)) = child.try_wait() {
        fail(format!("window exited early with {status} before CLI step"));
    }
    let out = Command::new(exe("hamesh"))
        .args(args)
        .output()
        .unwrap_or_else(|e| fail(format!("failed to run hamesh CLI: {e}")));
    let code = out.status.code().unwrap_or(1);
    (code, String::from_utf8_lossy(&out.stdout).to_string())
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

fn result_prop(out: &str, key: &str) -> String {
    let v: serde_json::Value =
        serde_json::from_str(out).unwrap_or_else(|_| fail(format!("CLI reply not JSON: {out}")));
    v.get("result")
        .and_then(|r| r.get(key))
        .and_then(|p| p.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| fail(format!("reply result has no {key}: {out}")))
}

fn detail_prop(child: &mut Child, id: &str, key: &str) -> String {
    let (code, out) = cli(child, &["detail", id]);
    if code != 0 {
        fail(format!("detail {id} failed with code {code}: {out}"));
    }
    let v: serde_json::Value =
        serde_json::from_str(&out).unwrap_or_else(|_| fail(format!("detail not JSON: {out}")));
    v.get("result")
        .and_then(|r| r.get("props"))
        .and_then(|p| p.get(key))
        .and_then(|p| p.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| fail(format!("detail {id} has no prop {key}: {out}")))
}

/// Resolved text geometry: (rect width, origin x, advance, halign).
/// Everything the alignment check needs, from one detail call.
fn detail_geom(child: &mut Child, id: &str) -> (f32, f32, f32, String) {
    let (code, out) = cli(child, &["detail", id]);
    if code != 0 {
        fail(format!("detail {id} failed with code {code}: {out}"));
    }
    let v: serde_json::Value =
        serde_json::from_str(&out).unwrap_or_else(|_| fail(format!("detail not JSON: {out}")));
    let r = v
        .get("result")
        .unwrap_or_else(|| fail(format!("detail {id} has no result")));
    let w = r
        .get("rect")
        .and_then(|q| q.get("w"))
        .and_then(|x| x.as_f64())
        .unwrap_or_else(|| fail(format!("detail {id} has no rect.w"))) as f32;
    let g = r
        .get("text_geom")
        .unwrap_or_else(|| fail(format!("detail {id} has no text_geom")));
    let ox = g
        .get("origin")
        .and_then(|o| o.get(0))
        .and_then(|x| x.as_f64())
        .unwrap_or_else(|| fail(format!("detail {id} has no origin"))) as f32;
    let adv = g
        .get("advance")
        .and_then(|x| x.as_f64())
        .unwrap_or_else(|| fail(format!("detail {id} has no advance"))) as f32;
    let halign = g
        .get("halign")
        .and_then(|x| x.as_str())
        .unwrap_or_else(|| fail(format!("detail {id} has no halign")))
        .to_string();
    (w, ox, adv, halign)
}

/// Machine alignment check (§H4): shaped advance centred in the box.
/// Fails on the old left-biased renderer (offset ~9px) and passes on
/// the resolved contract (offset < 1px) — the drill detects the bug.
fn check_centered(child: &mut Child, id: &str, ctx: &str) {
    let (w, ox, adv, halign) = detail_geom(child, id);
    if halign != "center" {
        fail(format!("{ctx}: {id} halign is {halign}, want center"));
    }
    let off = (ox + adv / 2.0 - w / 2.0).abs();
    println!("DRILL-GEOM {ctx}: {id} offset_px={off:.2} (w={w:.1} ox={ox:.1} adv={adv:.1})");
    if off > 1.0 {
        fail(format!(
            "{ctx}: {id} off-centre by {off:.2}px (tolerance 1.0)"
        ));
    }
}

fn main() {
    if !std::path::Path::new("Cargo.toml").exists()
        || !std::path::Path::new("apps/hamesh-blocks-drill").exists()
    {
        fail("run from the workspace root (cargo run -p hamesh-blocks-drill)".to_string());
    }

    step(0, "build window + CLI");
    let st = Command::new("cargo")
        .args(["build", "-q", "-p", "hamesh-window", "-p", "hamesh-cli"])
        .status()
        .unwrap_or_else(|e| fail(format!("cargo build failed to start: {e}")));
    if !st.success() {
        fail("cargo build -p hamesh-window -p hamesh-cli failed".to_string());
    }

    let _ = std::fs::remove_file(DISCOVERY);

    step(1, "open --blocks --inspect (smoke clock)");
    let log = std::fs::File::create(WINDOW_LOG)
        .unwrap_or_else(|e| fail(format!("cannot create {WINDOW_LOG}: {e}")));
    let mut child: Child = Command::new(exe("hamesh-window-demo"))
        .args([
            "--blocks",
            "--inspect",
            &format!("--smoke-test={SMOKE_SECS}"),
        ])
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

    step(3, "map: the dashboard composition is present");
    let (code, out) = cli(&mut child, &["map"]);
    if code != 0 {
        fail(format!("map failed with code {code}: {out}"));
    }
    let r1 = app_rev(&out);
    for id in [
        "dash",
        "dash-title",
        "dash-status",
        "m-agents",
        "m-tasks",
        "m-errors",
        "dash-bar",
        "dash-action",
    ] {
        if !out.contains(id) {
            fail(format!("map has no {id} node"));
        }
    }

    step(4, "detail: progress bar exposes its binding");
    if detail_prop(&mut child, "dash-bar", "block") != "Progress" {
        fail("dash-bar is not a Progress block".to_string());
    }
    if detail_prop(&mut child, "dash-bar", "value") != "75" {
        fail("dash-bar value is not 75".to_string());
    }

    step(5, "detail: button exposes its action");
    if detail_prop(&mut child, "dash-action", "block") != "Button" {
        fail("dash-action is not a Button block".to_string());
    }
    if detail_prop(&mut child, "dash-action", "action") != "inspect_design" {
        fail("dash-action has no inspect_design action".to_string());
    }

    step(6, "text alignment: badge and button are centred pre-click");
    check_centered(&mut child, "dash-status", "pre-click");
    check_centered(&mut child, "dash-action", "pre-click");

    step(7, "click Inspect design through hit-testing");
    let (code, out) = cli(
        &mut child,
        &["--expect-rev", &r1.to_string(), "click", "dash-action"],
    );
    if code != 0 {
        fail(format!("click dash-action failed with code {code}: {out}"));
    }
    let r2 = app_rev(&out);
    if r2 <= r1 {
        fail(format!("click did not advance rev: {r1} -> {r2}"));
    }
    if result_prop(&out, "hit") != "dash-action" {
        fail(format!("click hit the wrong node: {out}"));
    }

    step(8, "binding updated: the label follows state");
    if detail_prop(&mut child, "dash-action", "text") != "Inspected ✓" {
        fail("dash-action label did not follow the inspected state".to_string());
    }

    step(9, "text alignment: relabelled button is still centred");
    // The label changed (and the box resized); centering must hold
    // with no per-content offsets — the dynamic-update regression.
    check_centered(&mut child, "dash-action", "post-relabel");
    check_centered(&mut child, "dash-status", "post-relabel");

    step(10, "stale click on the old rev is refused");
    let (code, out) = cli(
        &mut child,
        &["--expect-rev", &r1.to_string(), "click", "dash-action"],
    );
    if code != 2 || !code_is(&out, "STALE") {
        fail(format!("stale click should be STALE/2, got {code}: {out}"));
    }

    step(11, "snapshot evidence");
    let (code, out) = cli(&mut child, &["snapshot", SNAPSHOT]);
    if code != 0 {
        fail(format!("snapshot failed with code {code}: {out}"));
    }
    let meta =
        std::fs::metadata(SNAPSHOT).unwrap_or_else(|e| fail(format!("snapshot file missing: {e}")));
    if meta.len() == 0 {
        fail("snapshot file is empty".to_string());
    }

    step(12, "overlay capture: guides match the reported geometry");
    // The diagnostic image must carry the same resolved geometry the
    // Eye reports for the same revision — no second geometry model.
    let (code, out) = cli(&mut child, &["snapshot", OVERLAY_SNAPSHOT, "dash-status"]);
    if code != 0 {
        fail(format!("overlay snapshot failed with code {code}: {out}"));
    }
    let v: serde_json::Value = serde_json::from_str(&out)
        .unwrap_or_else(|_| fail(format!("overlay reply not JSON: {out}")));
    let ov = v
        .get("result")
        .and_then(|r| r.get("overlay"))
        .unwrap_or_else(|| fail(format!("overlay reply has no overlay geometry: {out}")));
    let ov_origin = ov
        .get("text_geom")
        .and_then(|g| g.get("origin"))
        .and_then(|o| o.get(0))
        .and_then(|x| x.as_f64())
        .unwrap_or_else(|| fail(format!("overlay geometry has no origin: {out}")));
    // Same revision, same numbers as the detail path asserted above.
    let (w, ox, adv, _) = detail_geom(&mut child, "dash-status");
    if (ov_origin - ox as f64).abs() > 0.01 {
        fail(format!(
            "overlay origin {ov_origin} disagrees with detail {ox}"
        ));
    }
    let _ = (w, adv);
    let ometa = std::fs::metadata(OVERLAY_SNAPSHOT)
        .unwrap_or_else(|e| fail(format!("overlay snapshot file missing: {e}")));
    if ometa.len() == 0 {
        fail("overlay snapshot file is empty".to_string());
    }

    step(13, "wait for the smoke clock, check the window's trace");
    let status = child
        .wait()
        .unwrap_or_else(|e| fail(format!("waiting for window failed: {e}")));
    if !status.success() {
        fail(format!("window exited with {status}"));
    }
    let trace =
        std::fs::read_to_string(TRACE).unwrap_or_else(|e| fail(format!("no trace file: {e}")));
    let mut clicked = false;
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&trace) {
        if let Some(arr) = v.as_array() {
            for e in arr {
                let kind = e.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                let detail = e.get("detail").and_then(|d| d.as_str()).unwrap_or("");
                if kind == "ext.click" && detail.contains("dash-action") {
                    clicked = true;
                    break;
                }
            }
        }
    }
    if !clicked {
        fail("trace has no ext.click record for dash-action".to_string());
    }

    println!("DRILL-PASS blocks map -> semantics -> align -> click -> binding -> align -> stale -> snapshot -> overlay (revs {r1}->{r2})");
}
