//! hamesh-scene: resolved visual representation.
//!
//! The scene is derived from App truth + Layout truth. It carries paint order,
//! resolved colors/text and per-node visibility facts. Revisions tie frames
//! to the exact app/layout state they were built from.

use hamesh_core::App;
use hamesh_layout::{LayoutOutput, Rect};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneNode {
    pub id: String,
    pub role: String,
    pub name: String,
    pub rect: Rect,
    pub z: i32,
    pub visible: bool,
    pub clipped: bool,
    pub enabled: bool,
    pub color: [u8; 4],
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scene {
    pub scene_revision: u64,
    pub app_revision: u64,
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub nodes: Vec<SceneNode>,
}

fn role_color(role: &str, name: &str) -> [u8; 4] {
    // One polished dark theme: near-black surfaces, restrained teal/green.
    match role {
        s if s.contains("Window") => [14, 20, 22, 255],
        s if s.contains("Header") => [20, 30, 33, 255],
        s if s.contains("Button") => {
            if name.to_lowercase().contains("primary") || name.to_lowercase().contains("increment")
            {
                [45, 151, 134, 255] // teal accent
            } else {
                [38, 52, 56, 255]
            }
        }
        s if s.contains("Overlay") => [120, 40, 40, 180], // translucent red for fault drills
        // Text/Status nodes are labels on their parent surface: no fill of
        // their own (transparent), so the parent shows through. Their
        // content is the rasterized text, not the rect.
        s if s.contains("Text") | s.contains("Status") => [0, 0, 0, 0],
        _ => [30, 42, 46, 255],
    }
}

pub fn build(app: &App, layout: &LayoutOutput, scene_revision: u64) -> Scene {
    let mut nodes: Vec<SceneNode> = app
        .components
        .values()
        .filter_map(|c| {
            let l = layout.nodes.get(&c.id.0)?;
            let role = format!("{:?}", c.role);
            // Only explicit `text` props become rendered text. The
            // component name is identity (for the Eye), never content:
            // painting names of containers ("root", "main") as text
            // was a real bug caught by the frame PNG.
            let text = c.props.get("text").cloned().unwrap_or_default();
            Some(SceneNode {
                id: c.id.0.clone(),
                role: role.clone(),
                name: c.name.clone(),
                rect: l.rect.clone(),
                z: l.z,
                visible: l.effectively_visible,
                clipped: l.clipped_by_parent,
                enabled: c.enabled,
                color: role_color(&role, &c.name),
                text,
            })
        })
        .collect();
    nodes.sort_by_key(|n| n.z);
    Scene {
        scene_revision,
        app_revision: app.revision,
        viewport_w: layout.viewport.w,
        viewport_h: layout.viewport.h,
        nodes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hamesh_core::{Component, Role};

    #[test]
    fn scene_sorted_and_referenced() {
        let mut app = App::new("win");
        app.add("win", Component::new("b1", Role::Button, "Increment"));
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = build(&app, &layout, 7);
        assert_eq!(scene.app_revision, app.revision);
        assert_eq!(scene.scene_revision, 7);
        assert!(scene.nodes.iter().any(|n| n.id == "b1"));
    }
}
