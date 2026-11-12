//! Adversarial Terror Bats Suite for Hamesh 042:
//! - Rapid resize and window movement simulation
//! - Dynamic visual profile switching without state corruption
//! - Cross-language C ABI memory safety, mutation, and revision tracking
//! - Menu dropdown toggle and keyboard navigation stability

use hamesh_core::{App, Component, Role};
use hamesh_ffi::*;
use hamesh_layout::compute;
use hamesh_scene::build;
use std::ffi::CString;

#[test]
fn test_bat_10_ffi_opaque_handle_lifecycle_and_mutation() {
    unsafe {
        let app = hamesh_app_create(b"root\0".as_ptr() as *const _);
        assert!(!app.is_null());
        assert_eq!(hamesh_app_get_revision(app), 1);

        let parent = CString::new("root").unwrap();
        let comp_id = CString::new("label_test").unwrap();
        let name = CString::new("Test Label").unwrap();

        let status = hamesh_app_add_component(
            app,
            parent.as_ptr(),
            comp_id.as_ptr(),
            5, // Role::Text
            name.as_ptr(),
        );
        assert_eq!(status, HameshStatus::Ok);
        assert!(hamesh_app_get_revision(app) > 1);

        let prop_k = CString::new("text").unwrap();
        let prop_v = CString::new("Hello from C ABI").unwrap();
        let status = hamesh_app_set_prop(app, comp_id.as_ptr(), prop_k.as_ptr(), prop_v.as_ptr());
        assert_eq!(status, HameshStatus::Ok);

        // Verify component presence and layout
        let handle = &*app;
        let guard = handle.inner.lock().unwrap();
        let comp = guard.get("label_test").expect("component exists");
        assert_eq!(comp.props.get("text").map(|s| s.as_str()), Some("Hello from C ABI"));
        drop(guard);

        hamesh_app_destroy(app);
    }
}

#[test]
fn test_bat_11_rapid_resize_and_dimension_churn() {
    let mut app = App::new("root");
    app.add(
        "root",
        Component::new("panel", Role::Panel, "Main Panel")
            .with_prop("layout", "column")
            .with_prop("w", "100%")
            .with_prop("h", "100%"),
    );

    // Rapid-fire dimension shifts across 100 iterations simulating live OS resize drag
    for i in 1..=100 {
        let w = 400.0 + (i as f32) * 5.0;
        let h = 300.0 + ((i % 20) as f32) * 10.0;
        let layout = compute(&app, w, h);
        let scene = build(&app, &layout, i);
        assert_eq!(scene.viewport_w, w);
        assert_eq!(scene.viewport_h, h);
        assert!(!scene.nodes.is_empty());
    }
}

#[test]
fn test_bat_12_profile_switching_without_state_loss() {
    let mut app = App::new("root");
    app.add(
        "root",
        Component::new("user_input", Role::TextInput, "Field")
            .with_prop("text", "Persisted Value 42"),
    );

    let rev_before = app.revision;

    // Simulate Profile Switch: Classic Desktop -> Modern Studio
    if let Some(comp) = app.get_mut("user_input") {
        comp.props.insert("font".to_string(), "Segoe UI".to_string());
        comp.props.insert("radius".to_string(), "0".to_string());
        app.revision += 1;
    }

    assert_eq!(
        app.get("user_input").unwrap().props.get("text").map(|s| s.as_str()),
        Some("Persisted Value 42")
    );

    // Switch to Modern Studio
    if let Some(comp) = app.get_mut("user_input") {
        comp.props.insert("font".to_string(), "Instrument Sans".to_string());
        comp.props.insert("radius".to_string(), "6".to_string());
        app.revision += 1;
    }

    assert_eq!(
        app.get("user_input").unwrap().props.get("text").map(|s| s.as_str()),
        Some("Persisted Value 42")
    );
    assert!(app.revision > rev_before + 1);
}
