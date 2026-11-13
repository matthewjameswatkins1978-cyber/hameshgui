//! Standalone consumer of `hamesh-widgets` and `hamesh-theme` outside Workbench.
//! Demonstrates that the extracted controls are genuinely reusable components.

use hamesh_core::{App, Component, Role};
use hamesh_theme::VisualWorld;
use hamesh_widgets::{
    CheckboxControl, CommandRegistry, FocusRing, MenuBar, MenuCategory, MenuItem,
    SelectableListControl, SplitterControl, StatusBarControl, TabControl,
};

fn main() {
    println!("=== Hamesh Standalone Consumer Test ===");

    // 1. Initialize component tree
    let mut app = App::new("root");
    let world = VisualWorld::EditorialPaper;

    let mut root = Component::new("root", Role::Window, "Root Window");
    root.props
        .insert("theme".to_string(), world.name().to_string());
    app.components.insert("root".to_string(), root);

    // 2. Build MenuBar
    let mut menu_bar = MenuBar::new();
    menu_bar.add_menu(
        MenuCategory::new("File")
            .with_item(MenuItem::new("item_new", "New Document").with_command("file_new"))
            .with_item(MenuItem::new("item_save", "Save Document").with_command("file_save")),
    );
    menu_bar.add_menu(
        MenuCategory::new("View")
            .with_item(MenuItem::new("item_theme_classic", "Classic").with_command("theme_classic"))
            .with_item(MenuItem::new("item_theme_modern", "Modern").with_command("theme_modern")),
    );

    menu_bar.build_components(&mut app, "root", world);
    println!(
        "MenuBar components built: {} total components in tree",
        app.components.len()
    );
    assert!(app.components.contains_key("menu_bar"));

    // 3. Build TabControl
    let tabs = TabControl::new(
        "sample_tabs",
        vec![("doc", "Document"), ("settings", "Settings")],
    );
    tabs.build_components(&mut app, "root", world);
    println!(
        "TabControl components built: {} total components in tree",
        app.components.len()
    );
    assert!(app.components.contains_key("sample_tabs"));

    // 4. Build Checkbox & Splitter
    let chk = CheckboxControl::new("chk_autosave", "Enable Autosave", true);
    chk.build_component(&mut app, "root", false, world);

    let splitter = SplitterControl::new("doc_splitter", 300.0, 100.0, 600.0);
    splitter.build_component(&mut app, "root", world);

    // 5. Build SelectableList
    let list = SelectableListControl::new(
        "recent_files",
        vec!["contract.txt", "notes.md", "architecture.rs"],
    );
    list.build_components(&mut app, "root", None, world);

    // 6. Build StatusBar
    let mut status = StatusBarControl::new("app_status", "Document loaded cleanly.");
    status.set_profile_tag("Editorial / Paper [editorial]");
    status.set_telemetry("Encoding: UTF-8");
    status.build_components(&mut app, "root", world);

    // 7. Verify FocusRing over app
    let mut focus_ring = FocusRing::new(vec![]);
    focus_ring.rebuild_from_app(&app);
    println!(
        "FocusRing auto-discovered {} focusable controls",
        focus_ring.order.len()
    );
    assert!(!focus_ring.order.is_empty());

    // 8. Command Registry
    let mut registry = CommandRegistry::new();
    registry.register("file_save", "Save File", Some("Ctrl+S"), "File");
    assert_eq!(
        registry.find_by_shortcut("Ctrl+S").map(|c| c.id.as_str()),
        Some("file_save")
    );

    // 9. Compute Layout and Verify Scene Generation
    let layout = hamesh_layout::compute(&app, 1024.0, 768.0);
    for node in layout.nodes.values() {
        assert!(!node.rect.x.is_nan());
        assert!(!node.rect.y.is_nan());
        assert!(!node.rect.w.is_nan());
        assert!(!node.rect.h.is_nan());
    }
    assert!(!layout.nodes.is_empty());

    let scene = hamesh_scene::build(&app, &layout, 1);
    assert!(!scene.nodes.is_empty());
    println!("Scene generated with {} nodes cleanly.", scene.nodes.len());

    println!("SUCCESS: Standalone consumer validated all extracted desktop controls!");
}
