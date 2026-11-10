use hamesh_core::{App, Component, Role};
use hamesh_widgets::{handle_key, rebuild_feed_nodes, sync_composer, Focus, Key, Widgets};
use std::time::Instant;

#[derive(Clone, Debug, Default)]
pub struct BenchMetrics {
    pub name: String,
    pub iterations: usize,
    pub layout_us: Vec<f64>,
    pub scene_us: Vec<f64>,
    pub render_us: Vec<f64>,
    pub conv_us: Vec<f64>,
    pub total_us: Vec<f64>,
}

fn calc_percentiles(mut times: Vec<f64>) -> (f64, f64, f64) {
    if times.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let len = times.len();
    let p50 = times[(len as f64 * 0.50) as usize];
    let p95 = times[((len as f64 * 0.95) as usize).min(len - 1)];
    let p99 = times[((len as f64 * 0.99) as usize).min(len - 1)];
    (p50, p95, p99)
}

impl BenchMetrics {
    pub fn new(name: &str, iterations: usize) -> Self {
        Self {
            name: name.to_string(),
            iterations,
            layout_us: Vec::with_capacity(iterations),
            scene_us: Vec::with_capacity(iterations),
            render_us: Vec::with_capacity(iterations),
            conv_us: Vec::with_capacity(iterations),
            total_us: Vec::with_capacity(iterations),
        }
    }

    pub fn record(&mut self, l: f64, s: f64, r: f64, c: f64) {
        self.layout_us.push(l);
        self.scene_us.push(s);
        self.render_us.push(r);
        self.conv_us.push(c);
        self.total_us.push(l + s + r + c);
    }

    pub fn print_summary(&self) {
        let (l_p50, l_p95, l_p99) = calc_percentiles(self.layout_us.clone());
        let (s_p50, s_p95, s_p99) = calc_percentiles(self.scene_us.clone());
        let (r_p50, r_p95, r_p99) = calc_percentiles(self.render_us.clone());
        let (c_p50, c_p95, c_p99) = calc_percentiles(self.conv_us.clone());
        let (t_p50, t_p95, t_p99) = calc_percentiles(self.total_us.clone());

        println!(
            "\n=== BENCH: {} ({} frames) ===",
            self.name, self.iterations
        );
        println!(
            "  Layout:      p50 = {:>7.2} us | p95 = {:>7.2} us | p99 = {:>7.2} us",
            l_p50, l_p95, l_p99
        );
        println!(
            "  Scene Build: p50 = {:>7.2} us | p95 = {:>7.2} us | p99 = {:>7.2} us",
            s_p50, s_p95, s_p99
        );
        println!(
            "  CPU Render:  p50 = {:>7.2} us | p95 = {:>7.2} us | p99 = {:>7.2} us",
            r_p50, r_p95, r_p99
        );
        println!(
            "  Pixel Conv:  p50 = {:>7.2} us | p95 = {:>7.2} us | p99 = {:>7.2} us",
            c_p50, c_p95, c_p99
        );
        println!(
            "  TOTAL FRAME: p50 = {:>7.2} ms | p95 = {:>7.2} ms | p99 = {:>7.2} ms",
            t_p50 / 1000.0,
            t_p95 / 1000.0,
            t_p99 / 1000.0
        );
    }
}

fn convert_pixels(frame: &hamesh_render::Frame, cpu_pixels: &mut [u32]) {
    let src = frame.pixmap.data();
    let (pixels, _) = src.as_chunks::<4>();
    for (pixel, chunk) in cpu_pixels.iter_mut().zip(pixels) {
        let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
        let (r, g, b) = if a == 0 {
            (0, 0, 0)
        } else if a == 255 {
            (r as u32, g as u32, b as u32)
        } else {
            let a = a as u32;
            (
                (r as u32) * 255 / a,
                (g as u32) * 255 / a,
                (b as u32) * 255 / a,
            )
        };
        *pixel = (r << 16) | (g << 8) | b;
    }
}

fn setup_populated_m3(message_count: usize) -> (App, Widgets) {
    let mut app = App::new("win");
    app.add(
        "win",
        Component::new("header", Role::Header, "Agent X chat")
            .with_prop("h", "48")
            .with_prop("text", "Agent X — Hamesh dynamic benchmark"),
    );
    app.add(
        "win",
        Component::new("feed", Role::List, "feed").with_prop("layout", "column"),
    );
    app.add(
        "win",
        Component::new("composer", Role::TextInput, "composer")
            .with_prop("h", "40")
            .with_prop("text", ""),
    );
    let mut w = Widgets {
        composer_id: "composer".to_string(),
        feed_id: "feed".to_string(),
        feed_viewport: 500.0,
        ..Default::default()
    };
    for i in 0..message_count {
        let role = if i % 2 == 0 { "user" } else { "agent" };
        w.feed.push(
            role,
            &format!("Message {i}: the quick brown fox jumps over the lazy dog in session benchmark testing."),
        );
    }
    w.feed.relayout(780.0);
    w.feed.scroll_to_bottom(500.0);
    w.focus = Focus::rebuild(&app);
    sync_composer(&mut app, &w);
    rebuild_feed_nodes(&mut app, &mut w, "feed", 780.0);
    (app, w)
}

#[test]
fn bench_040_changing_content() {
    hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());

    // ------------------------------------------------------------------------
    // Workload 1: Streaming text into a populated Pi console
    // ------------------------------------------------------------------------
    {
        let (mut app, mut w) = setup_populated_m3(30);
        let stream_idx = w.feed.push_live("agent");
        let tokens = [
            " Analyzing",
            " the",
            " query",
            " execution",
            " plan",
            " and",
            " verifying",
            " layout",
            " bounds",
            " across",
            " all",
            " elements.",
            " The",
            " system",
            " demonstrates",
            " consistent",
            " sub-frame",
            " latency",
            " when",
            " streaming.",
        ];
        let iters = 100;
        let mut metrics = BenchMetrics::new("1. Streaming text into populated Pi console", iters);
        let mut cpu_pixels = vec![0u32; 800 * 600];

        for i in 0..iters {
            let token = tokens[i % tokens.len()];
            w.feed.append_to_message(stream_idx, token);
            w.feed.relayout(780.0);
            w.feed.scroll_to_bottom(500.0);
            rebuild_feed_nodes(&mut app, &mut w, "feed", 780.0);

            let t0 = Instant::now();
            let layout = hamesh_layout::compute(&app, 800.0, 600.0);
            let l_us = t0.elapsed().as_micros() as f64;

            let t1 = Instant::now();
            let scene = hamesh_scene::build(&app, &layout, (i + 1) as u64);
            let s_us = t1.elapsed().as_micros() as f64;

            let t2 = Instant::now();
            let frame = hamesh_render::render(&scene);
            let r_us = t2.elapsed().as_micros() as f64;

            let t3 = Instant::now();
            convert_pixels(&frame, &mut cpu_pixels);
            let c_us = t3.elapsed().as_micros() as f64;

            metrics.record(l_us, s_us, r_us, c_us);
        }
        metrics.print_summary();
    }

    // ------------------------------------------------------------------------
    // Workload 2: Scrolling a long virtualized list
    // ------------------------------------------------------------------------
    {
        let (mut app, mut w) = setup_populated_m3(200);
        w.feed.scroll_px = 0.0;
        let iters = 100;
        let mut metrics =
            BenchMetrics::new("2. Scrolling long virtualized list (200 items)", iters);
        let mut cpu_pixels = vec![0u32; 800 * 600];

        for i in 0..iters {
            w.feed.scroll_by(12.0, 500.0);
            rebuild_feed_nodes(&mut app, &mut w, "feed", 780.0);

            let t0 = Instant::now();
            let layout = hamesh_layout::compute(&app, 800.0, 600.0);
            let l_us = t0.elapsed().as_micros() as f64;

            let t1 = Instant::now();
            let scene = hamesh_scene::build(&app, &layout, (i + 1) as u64);
            let s_us = t1.elapsed().as_micros() as f64;

            let t2 = Instant::now();
            let frame = hamesh_render::render(&scene);
            let r_us = t2.elapsed().as_micros() as f64;

            let t3 = Instant::now();
            convert_pixels(&frame, &mut cpu_pixels);
            let c_us = t3.elapsed().as_micros() as f64;

            metrics.record(l_us, s_us, r_us, c_us);
        }
        metrics.print_summary();
    }

    // ------------------------------------------------------------------------
    // Workload 3: Updating one control in a complex interface (Sheet with 48+ nodes)
    // ------------------------------------------------------------------------
    {
        let mut sh = hamesh_sheet::Sheet::new();
        let (mut app, _) = hamesh_sheet::refresh(&mut sh, 800.0, 600.0);
        let iters = 100;
        let mut metrics = BenchMetrics::new(
            "3. Updating one control in complex interface (48+ nodes)",
            iters,
        );
        let mut cpu_pixels = vec![0u32; 800 * 600];

        for i in 0..iters {
            // Update a single control's prop without touching any siblings
            if let Some(c) = app.get_mut("sh_badge_1") {
                c.props.insert("text".to_string(), format!("Active {i}"));
            } else if let Some((_, c)) = app.components.iter_mut().next() {
                c.props.insert("text".to_string(), format!("Tick {i}"));
            }

            let t0 = Instant::now();
            let layout = hamesh_layout::compute(&app, 800.0, 600.0);
            let l_us = t0.elapsed().as_micros() as f64;

            let t1 = Instant::now();
            let scene = hamesh_scene::build(&app, &layout, (i + 1) as u64);
            let s_us = t1.elapsed().as_micros() as f64;

            let t2 = Instant::now();
            let frame = hamesh_render::render(&scene);
            let r_us = t2.elapsed().as_micros() as f64;

            let t3 = Instant::now();
            convert_pixels(&frame, &mut cpu_pixels);
            let c_us = t3.elapsed().as_micros() as f64;

            metrics.record(l_us, s_us, r_us, c_us);
        }
        metrics.print_summary();
    }

    // ------------------------------------------------------------------------
    // Workload 4: Editing text with continuous keyboard input
    // ------------------------------------------------------------------------
    {
        let (mut app, mut w) = setup_populated_m3(30);
        w.focus.current = w
            .focus
            .order
            .iter()
            .position(|id| id == &w.composer_id)
            .unwrap_or(0);
        let iters = 100;
        let mut metrics =
            BenchMetrics::new("4. Continuous keyboard text input into composer", iters);
        let mut cpu_pixels = vec![0u32; 800 * 600];

        let test_chars = "The quick brown fox jumps over the lazy dog. Continuous typing latency benchmark measurement!";
        let char_vec: Vec<char> = test_chars.chars().collect();

        for i in 0..iters {
            let ch = char_vec[i % char_vec.len()];
            handle_key(&mut app, &mut w, &Key::Char(ch));

            let t0 = Instant::now();
            let layout = hamesh_layout::compute(&app, 800.0, 600.0);
            let l_us = t0.elapsed().as_micros() as f64;

            let t1 = Instant::now();
            let scene = hamesh_scene::build(&app, &layout, (i + 1) as u64);
            let s_us = t1.elapsed().as_micros() as f64;

            let t2 = Instant::now();
            let frame = hamesh_render::render(&scene);
            let r_us = t2.elapsed().as_micros() as f64;

            let t3 = Instant::now();
            convert_pixels(&frame, &mut cpu_pixels);
            let c_us = t3.elapsed().as_micros() as f64;

            metrics.record(l_us, s_us, r_us, c_us);
        }
        metrics.print_summary();
    }

    // ------------------------------------------------------------------------
    // Workload 5: Resizing a populated native window
    // ------------------------------------------------------------------------
    {
        let (mut app, mut w) = setup_populated_m3(30);
        let iters = 60;
        let mut metrics = BenchMetrics::new(
            "5. Resizing populated window across variable dimensions",
            iters,
        );
        let mut cpu_pixels = vec![0u32; 1920 * 1080];

        for i in 0..iters {
            // Oscillate size between 800x600 and 1920x1080
            let factor = (i as f32 / iters as f32) * std::f32::consts::PI;
            let width = 800.0 + 1120.0 * factor.sin().abs();
            let height = 600.0 + 480.0 * factor.sin().abs();

            w.feed_viewport = height - 100.0;
            w.feed.relayout(width - 20.0);
            rebuild_feed_nodes(&mut app, &mut w, "feed", width - 20.0);

            let t0 = Instant::now();
            let layout = hamesh_layout::compute(&app, width, height);
            let l_us = t0.elapsed().as_micros() as f64;

            let t1 = Instant::now();
            let scene = hamesh_scene::build(&app, &layout, (i + 1) as u64);
            let s_us = t1.elapsed().as_micros() as f64;

            let t2 = Instant::now();
            let frame = hamesh_render::render(&scene);
            let r_us = t2.elapsed().as_micros() as f64;

            let t3 = Instant::now();
            let (pw, ph) = (
                frame.pixmap.width() as usize,
                frame.pixmap.height() as usize,
            );
            let needed = pw * ph;
            if needed <= cpu_pixels.len() {
                convert_pixels(&frame, &mut cpu_pixels[..needed]);
            }
            let c_us = t3.elapsed().as_micros() as f64;

            metrics.record(l_us, s_us, r_us, c_us);
        }
        metrics.print_summary();
    }

    // ------------------------------------------------------------------------
    // Workload 6: Cold vs Warm rendering at 800x600, 1920x1080 and HiDPI (scale 2.0)
    // ------------------------------------------------------------------------
    {
        println!("\n=== BENCH: 6. Cold vs Warm Rendering at Multiple Resolutions ===");
        let resolutions = [
            ("800x600 Standard", 800.0, 600.0, 1.0f32),
            ("1920x1080 Full HD", 1920.0, 1080.0, 1.0f32),
            ("High-DPI (1920x1080 @ 2.0x scale)", 1920.0, 1080.0, 2.0f32),
        ];

        for (label, w_f, h_f, scale) in resolutions {
            let (app, _) = setup_populated_m3(30);

            // Cold run (first frame)
            let t_cold_start = Instant::now();
            let l_cold = hamesh_layout::compute(&app, w_f * scale, h_f * scale);
            let l_cold_us = t_cold_start.elapsed().as_micros() as f64;

            let t_scene = Instant::now();
            let s_cold = hamesh_scene::build(&app, &l_cold, 1);
            let s_cold_us = t_scene.elapsed().as_micros() as f64;

            let t_render = Instant::now();
            let _frame_cold = hamesh_render::render(&s_cold);
            let r_cold_us = t_render.elapsed().as_micros() as f64;

            let cold_total_ms = (l_cold_us + s_cold_us + r_cold_us) / 1000.0;

            // Warm runs (50 frames)
            let mut warm_totals = Vec::with_capacity(50);
            for i in 0..50 {
                let t0 = Instant::now();
                let l = hamesh_layout::compute(&app, w_f * scale, h_f * scale);
                let s = hamesh_scene::build(&app, &l, (i + 2) as u64);
                let _frame = hamesh_render::render(&s);
                warm_totals.push(t0.elapsed().as_micros() as f64);
            }
            let (w_p50, w_p95, w_p99) = calc_percentiles(warm_totals);

            println!(
                "  [{}] Cold: {:>6.2} ms | Warm p50: {:>6.2} ms | Warm p95: {:>6.2} ms | Warm p99: {:>6.2} ms",
                label, cold_total_ms, w_p50 / 1000.0, w_p95 / 1000.0, w_p99 / 1000.0
            );
        }
    }
}
