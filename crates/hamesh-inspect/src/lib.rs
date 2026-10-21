//! hamesh-inspect: the Eye. Machine-readable views of the live interface.
//!
//! Progressive disclosure: `compact_map` for the whole window, `detail` for
//! one component, `frame_info` to tie screenshots to revisions.

use hamesh_core::App;
use hamesh_layout::LayoutOutput;
use hamesh_scene::Scene;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapNode {
    pub id: String,
    pub role: String,
    pub name: String,
    pub rect: hamesh_layout::Rect,
    pub visible: bool,
    pub clipped: bool,
    pub enabled: bool,
    pub z: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompactMap {
    pub app_revision: u64,
    pub scene_revision: u64,
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub nodes: Vec<MapNode>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComponentDetail {
    pub id: String,
    pub role: String,
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub effectively_visible: bool,
    pub clipped_by_parent: bool,
    pub rect: Option<hamesh_layout::Rect>,
    pub z: Option<i32>,
    pub props: std::collections::HashMap<String, String>,
    pub state_keys: Vec<String>,
    pub diagnostics: Vec<String>,
}

pub fn compact_map(app: &App, _layout: &LayoutOutput, scene: &Scene) -> CompactMap {
    CompactMap {
        app_revision: app.revision,
        scene_revision: scene.scene_revision,
        viewport_w: scene.viewport_w,
        viewport_h: scene.viewport_h,
        nodes: scene
            .nodes
            .iter()
            .map(|n| MapNode {
                id: n.id.clone(),
                role: n.role.clone(),
                name: n.name.clone(),
                rect: n.rect.clone(),
                visible: n.visible,
                clipped: n.clipped,
                enabled: n.enabled,
                z: n.z,
            })
            .collect(),
    }
}

pub fn detail(app: &App, layout: &LayoutOutput, id: &str) -> Option<ComponentDetail> {
    let c = app.components.get(id)?;
    let l = layout.nodes.get(id);
    let mut diagnostics = Vec::new();
    if !c.enabled {
        diagnostics.push(
            "component is disabled: human-equivalent clicks will hit it but have no effect"
                .to_string(),
        );
    }
    if !c.visible {
        diagnostics.push("component.visible == false: not painted, not hit-testable".to_string());
    }
    if let Some(n) = l {
        if !n.effectively_visible {
            diagnostics.push(
                "effectively invisible via ancestor: not painted, not hit-testable".to_string(),
            );
        }
        if n.clipped_by_parent {
            diagnostics.push(
                "rect extends beyond parent rect: partially/fully clipped, may be unreachable"
                    .to_string(),
            );
        }
    } else {
        diagnostics.push("no layout node: outside computed tree".to_string());
    }
    Some(ComponentDetail {
        id: id.to_string(),
        role: format!("{:?}", c.role),
        name: c.name.clone(),
        enabled: c.enabled,
        visible: c.visible,
        effectively_visible: l.map(|n| n.effectively_visible).unwrap_or(false),
        clipped_by_parent: l.map(|n| n.clipped_by_parent).unwrap_or(false),
        rect: l.map(|n| n.rect.clone()),
        z: l.map(|n| n.z),
        props: c.props.clone(),
        state_keys: app.state.keys().cloned().collect(),
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hamesh_core::{Component, Role};

    #[test]
    fn map_and_detail() {
        let mut app = App::new("win");
        app.add("win", Component::new("b1", Role::Button, "Increment"));
        let layout = hamesh_layout::compute(&app, 400.0, 300.0);
        let scene = hamesh_scene::build(&app, &layout, 3);
        let map = compact_map(&app, &layout, &scene);
        assert!(map.nodes.iter().any(|n| n.id == "b1"));
        let d = detail(&app, &layout, "b1").unwrap();
        assert!(d.effectively_visible);
        assert!(d.diagnostics.is_empty());
    }
}
