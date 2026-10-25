//! Hamesh icons: vendored Lucide path data rendered as strokes.
//!
//! Lucide icons are ISC-licensed 24x24 stroke glyphs (2px, round caps);
//! see `assets/icons/LICENSE.lucide`. The SVGs are vendored verbatim and
//! converted at runtime: `<path d>` via kurbo's SVG parser, `<circle>` /
//! `<rect>` / `<line>` / `<polyline>` / `<polygon>` via direct segment
//! construction. Anything else is rejected — the registry is explicit,
//! not a general SVG renderer.
//!
//! Both renderers stroke the same `BezPath`: the CPU path converts it to
//! tiny-skia, the GPU path feeds it to vello with an affine placement.

use kurbo::{BezPath, Circle, PathEl, Point, Rect as KRect, Shape};

const USERS: &str = include_str!("../../../assets/icons/users.svg");
const LIST_CHECKS: &str = include_str!("../../../assets/icons/list-checks.svg");
const TRIANGLE_ALERT: &str = include_str!("../../../assets/icons/triangle-alert.svg");
const ACTIVITY: &str = include_str!("../../../assets/icons/activity.svg");
const CHEVRON_DOWN: &str = include_str!("../../../assets/icons/chevron-down.svg");

/// Registry: stable icon names agents can put in the `icon` prop.
pub fn icon_svg(name: &str) -> Option<&'static str> {
    match name {
        "users" => Some(USERS),
        "list-checks" => Some(LIST_CHECKS),
        "triangle-alert" => Some(TRIANGLE_ALERT),
        "activity" => Some(ACTIVITY),
        "chevron-down" => Some(CHEVRON_DOWN),
        _ => None,
    }
}

fn attr(tag: &str, key: &str) -> Option<String> {
    let pat = format!("{key}=\"");
    let start = tag.find(&pat)? + pat.len();
    let end = tag[start..].find('"')?;
    Some(tag[start..start + end].to_string())
}

fn num(tag: &str, key: &str) -> Option<f64> {
    attr(tag, key)?.parse().ok()
}

fn points(tag: &str) -> Option<Vec<Point>> {
    let raw = attr(tag, "points")?;
    raw.split_whitespace()
        .map(|pair| {
            let mut it = pair.split(',');
            let x: f64 = it.next()?.parse().ok()?;
            let y: f64 = it.next()?.parse().ok()?;
            Some(Point::new(x, y))
        })
        .collect()
}

/// Convert one SVG document to a 24-grid `BezPath`. Supports exactly the
/// element subset above; returns `Err` naming anything else.
pub fn svg_to_bez(svg: &str) -> Result<BezPath, String> {
    let mut out = BezPath::new();
    let mut rest = svg;
    while let Some(lt) = rest.find('<') {
        rest = &rest[lt + 1..];
        let end = rest.find('>').ok_or("unterminated tag")?;
        let tag = &rest[..end];
        rest = &rest[end + 1..];
        let head = tag.split_whitespace().next().unwrap_or("");
        match head {
            "svg" | "/svg" | "?xml" | "!--" => {}
            "path" => {
                let d = attr(tag, "d").ok_or("path without d")?;
                let sub = BezPath::from_svg(&d).map_err(|e| format!("path parse: {e:?}"))?;
                out.extend(sub);
            }
            "circle" => {
                let (cx, cy, r) = (
                    num(tag, "cx").ok_or("circle without cx")?,
                    num(tag, "cy").ok_or("circle without cy")?,
                    num(tag, "r").ok_or("circle without r")?,
                );
                out.extend(Circle::new((cx, cy), r).to_path(0.1));
            }
            "rect" => {
                let (x, y, w, h) = (
                    num(tag, "x").unwrap_or(0.0),
                    num(tag, "y").unwrap_or(0.0),
                    num(tag, "width").ok_or("rect without width")?,
                    num(tag, "height").ok_or("rect without height")?,
                );
                let r = KRect::new(x, y, x + w, y + h);
                if let Some(rx) = num(tag, "rx") {
                    out.extend(kurbo::RoundedRect::new(x, y, x + w, y + h, rx).to_path(0.1));
                } else {
                    out.extend(r.to_path(0.1));
                }
            }
            "line" => {
                let segs = [
                    num(tag, "x1").ok_or("line without x1")?,
                    num(tag, "y1").ok_or("line without y1")?,
                    num(tag, "x2").ok_or("line without x2")?,
                    num(tag, "y2").ok_or("line without y2")?,
                ];
                out.move_to((segs[0], segs[1]));
                out.line_to((segs[2], segs[3]));
            }
            "polyline" | "polygon" => {
                let pts = points(tag).ok_or("bad points")?;
                let mut first = true;
                for p in &pts {
                    if first {
                        out.move_to(*p);
                        first = false;
                    } else {
                        out.line_to(*p);
                    }
                }
                if head == "polygon" {
                    out.close_path();
                }
            }
            other => return Err(format!("unsupported svg element: {other}")),
        }
    }
    Ok(out)
}

/// Registry lookup + conversion. `None` = unknown icon name.
pub fn icon_bez(name: &str) -> Option<BezPath> {
    svg_to_bez(icon_svg(name)?).ok()
}

/// Placement: icon box is 62% of the smaller side, centered.
pub fn icon_placement(w: f32, h: f32) -> (f64, f64, f64) {
    let side = w.min(h) as f64 * 0.62;
    let scale = side / 24.0;
    let dx = (w as f64 - side) / 2.0;
    let dy = (h as f64 - side) / 2.0;
    (scale, dx, dy)
}

/// Convert a 24-grid `BezPath` to a tiny-skia path in node space.
pub fn bez_to_tinyskia(bez: &BezPath, dx: f64, dy: f64, scale: f64) -> Option<tiny_skia::Path> {
    let mut pb = tiny_skia::PathBuilder::new();
    let px = |p: Point| ((dx + p.x * scale) as f32, (dy + p.y * scale) as f32);
    for el in bez.iter() {
        match el {
            PathEl::MoveTo(p) => {
                let (x, y) = px(p);
                pb.move_to(x, y);
            }
            PathEl::LineTo(p) => {
                let (x, y) = px(p);
                pb.line_to(x, y);
            }
            PathEl::QuadTo(a, b) => {
                let (x1, y1) = px(a);
                let (x2, y2) = px(b);
                pb.quad_to(x1, y1, x2, y2);
            }
            PathEl::CurveTo(a, b, c) => {
                let (x1, y1) = px(a);
                let (x2, y2) = px(b);
                let (x3, y3) = px(c);
                pb.cubic_to(x1, y1, x2, y2, x3, y3);
            }
            PathEl::ClosePath => pb.close(),
        }
    }
    pb.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_parses_all_vendored_icons() {
        for name in [
            "users",
            "list-checks",
            "triangle-alert",
            "activity",
            "chevron-down",
        ] {
            let bez = icon_bez(name).expect(name);
            assert!(!bez.is_empty(), "{name} converted empty");
        }
    }

    #[test]
    fn unknown_icon_is_none() {
        assert!(icon_bez("nope").is_none());
    }

    #[test]
    fn unsupported_elements_rejected() {
        assert!(svg_to_bez("<svg><ellipse cx=\"1\"/></svg>").is_err());
        assert!(svg_to_bez("<svg><g></g></svg>").is_err());
    }

    #[test]
    fn placement_centers_and_scales() {
        let (s, dx, dy) = icon_placement(100.0, 40.0);
        // side = 40*0.62 = 24.8, centered in 100x40.
        assert!((s - 24.8 / 24.0).abs() < 1e-9);
        assert!((dx - (100.0 - 24.8) / 2.0).abs() < 1e-9);
        assert!((dy - (40.0 - 24.8) / 2.0).abs() < 1e-9);
    }
}
