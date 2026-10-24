//! hamesh-core: canonical component model, identity, state, commands.
//!
//! One canonical meaning for components, state, commands and revisions.
//! All other crates operate on this truth; they never duplicate it.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Stable component identity. Never reused within an app revision history.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ComponentId(pub String);

impl ComponentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Semantic role. Maps to accessibility roles later (AccessKit).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Window,
    Panel,
    Header,
    Button,
    TextInput,
    Text,
    List,
    ListItem,
    Status,
    Overlay,
    Other(String),
}

/// A component in the retained tree.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Component {
    pub id: ComponentId,
    pub parent: Option<ComponentId>,
    pub role: Role,
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    /// Free-form typed properties (label text, value, placeholder...).
    pub props: HashMap<String, String>,
    pub children: Vec<ComponentId>,
}

impl Component {
    pub fn new(id: &str, role: Role, name: &str) -> Self {
        Self {
            id: ComponentId::new(id),
            parent: None,
            role,
            name: name.to_string(),
            enabled: true,
            visible: true,
            props: HashMap::new(),
            children: Vec::new(),
        }
    }

    pub fn with_prop(mut self, k: &str, v: &str) -> Self {
        self.props.insert(k.to_string(), v.to_string());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }
}

/// The application: retained tree + revision counter + command registry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct App {
    pub revision: u64,
    pub root: ComponentId,
    pub components: HashMap<String, Component>,
    /// Command id -> component id that owns it (button activation, etc).
    pub commands: HashMap<String, String>,
    /// Simple key-value app state (counter value, input text...).
    pub state: HashMap<String, String>,
}

impl App {
    pub fn new(root_id: &str) -> Self {
        let root = Component::new(root_id, Role::Window, "root");
        let mut components = HashMap::new();
        components.insert(root_id.to_string(), root);
        Self {
            revision: 1,
            root: ComponentId::new(root_id),
            components,
            commands: HashMap::new(),
            state: HashMap::new(),
        }
    }

    pub fn add(&mut self, parent: &str, mut comp: Component) {
        comp.parent = Some(ComponentId::new(parent));
        let id = comp.id.0.clone();
        if let Some(p) = self.components.get_mut(parent) {
            if !p.children.iter().any(|c| c.0 == id) {
                p.children.push(ComponentId::new(&id));
            }
        }
        self.components.insert(id, comp);
        self.bump();
    }

    pub fn get(&self, id: &str) -> Option<&Component> {
        self.components.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Component> {
        self.components.get_mut(id)
    }

    pub fn register_command(&mut self, command_id: &str, owner: &str) {
        self.commands
            .insert(command_id.to_string(), owner.to_string());
        self.bump();
    }

    pub fn set_state(&mut self, k: &str, v: &str) {
        self.state.insert(k.to_string(), v.to_string());
        self.bump();
    }

    pub fn bump(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }
}

/// Which interaction path was used. Tests must record this.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mechanism {
    /// Through hit-test + focus + event routing, like a human pointer/keyboard.
    HumanEquivalent,
    /// Direct typed capability invocation. Must never masquerade as a GUI test.
    DirectOp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_and_revision() {
        let mut app = App::new("win");
        app.add("win", Component::new("btn", Role::Button, "Increment"));
        assert!(app.get("btn").is_some());
        let r1 = app.revision;
        app.set_state("counter", "1");
        assert!(app.revision > r1);
        assert_eq!(app.get("btn").unwrap().role, Role::Button);
    }
}
