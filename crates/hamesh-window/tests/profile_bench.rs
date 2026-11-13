use std::time::Instant;

fn calc_percentiles(mut times_us: Vec<f64>) -> (f64, f64, f64) {
    times_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let len = times_us.len();
    let p50 = times_us[(len as f64 * 0.50) as usize];
    let p95 = times_us[((len as f64 * 0.95) as usize).min(len - 1)];
    let p99 = times_us[((len as f64 * 0.99) as usize).min(len - 1)];
    (p50, p95, p99)
}

#[test]
fn bench_profile_pipeline() {
    hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());

    // 1. Sheet composition (complex, many blocks, buttons, type ramp)
    let mut sh = hamesh_sheet::Sheet::new();
    let (app, _) = hamesh_sheet::refresh(&mut sh, 800.0, 600.0);

    let iters = 100;

    let mut layout_times = Vec::with_capacity(iters);
    let mut scene_times = Vec::with_capacity(iters);
    let mut render_times = Vec::with_capacity(iters);
    let mut conv_times = Vec::with_capacity(iters);

    let mut cpu_pixels = vec![0u32; 800 * 600];

    for _ in 0..iters {
        let t0 = Instant::now();
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        layout_times.push(t0.elapsed().as_micros() as f64);

        let t1 = Instant::now();
        let scene = hamesh_scene::build(&app, &layout, 1);
        scene_times.push(t1.elapsed().as_micros() as f64);

        let t2 = Instant::now();
        let frame = hamesh_render::render(&scene);
        render_times.push(t2.elapsed().as_micros() as f64);

        let t3 = Instant::now();
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
        conv_times.push(t3.elapsed().as_micros() as f64);
    }

    let (l_p50, l_p95, l_p99) = calc_percentiles(layout_times);
    let (s_p50, s_p95, s_p99) = calc_percentiles(scene_times.clone());
    let (r_p50, r_p95, r_p99) = calc_percentiles(render_times);
    let (c_p50, c_p95, c_p99) = calc_percentiles(conv_times);

    println!("\n=== PROFILE: 800x600 Design Sheet (100 iterations) ===");
    println!(
        "Layout:      p50 = {:>6.2} us | p95 = {:>6.2} us | p99 = {:>6.2} us",
        l_p50, l_p95, l_p99
    );
    println!(
        "Scene Build: p50 = {:>6.2} us | p95 = {:>6.2} us | p99 = {:>6.2} us",
        s_p50, s_p95, s_p99
    );
    println!(
        "CPU Render:  p50 = {:>6.2} us | p95 = {:>6.2} us | p99 = {:>6.2} us",
        r_p50, r_p95, r_p99
    );
    println!(
        "Pixel Conv:  p50 = {:>6.2} us | p95 = {:>6.2} us | p99 = {:>6.2} us",
        c_p50, c_p95, c_p99
    );
    let total_p50 = (l_p50 + s_p50 + r_p50 + c_p50) / 1000.0;
    let total_p95 = (l_p95 + s_p95 + r_p95 + c_p95) / 1000.0;
    let total_p99 = (l_p99 + s_p99 + r_p99 + c_p99) / 1000.0;
    println!(
        "Total Frame: p50 = {:>6.2} ms | p95 = {:>6.2} ms | p99 = {:>6.2} ms",
        total_p50, total_p95, total_p99
    );

    // 2. Larger resolution: 1920x1080
    let mut render_1080 = Vec::with_capacity(50);
    let mut conv_1080 = Vec::with_capacity(50);
    let mut cpu_pixels_1080 = vec![0u32; 1920 * 1080];
    let (app_1080, _) = hamesh_sheet::refresh(&mut sh, 1920.0, 1080.0);
    let layout_1080 = hamesh_layout::compute(&app_1080, 1920.0, 1080.0);
    let scene_1080 = hamesh_scene::build(&app_1080, &layout_1080, 1);

    for _ in 0..50 {
        let t2 = Instant::now();
        let frame = hamesh_render::render(&scene_1080);
        render_1080.push(t2.elapsed().as_micros() as f64);

        let t3 = Instant::now();
        let src = frame.pixmap.data();
        let (pixels, _) = src.as_chunks::<4>();
        for (pixel, chunk) in cpu_pixels_1080.iter_mut().zip(pixels) {
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
        conv_1080.push(t3.elapsed().as_micros() as f64);
    }

    let (r1080_p50, r1080_p95, r1080_p99) = calc_percentiles(render_1080);
    let (c1080_p50, c1080_p95, c1080_p99) = calc_percentiles(conv_1080);
    println!("\n=== PROFILE: 1920x1080 (50 iterations) ===");
    println!(
        "CPU Render:  p50 = {:>6.2} us | p95 = {:>6.2} us | p99 = {:>6.2} us",
        r1080_p50, r1080_p95, r1080_p99
    );
    println!(
        "Pixel Conv:  p50 = {:>6.2} us | p95 = {:>6.2} us | p99 = {:>6.2} us",
        c1080_p50, c1080_p95, c1080_p99
    );
}
