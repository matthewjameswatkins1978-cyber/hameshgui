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

#[test]
fn test_bat_13_c_abi_safety_and_error_codes() {
    unsafe {
        // 1. ABI Version
        assert_eq!(hamesh_abi_version(), 0x000402);

        // 2. Null pointer resilience
        assert_eq!(
            hamesh_app_add_component(
                std::ptr::null_mut(),
                b"root\0".as_ptr() as *const _,
                b"id\0".as_ptr() as *const _,
                0,
                b"Name\0".as_ptr() as *const _
            ),
            HameshStatus::NullPointer
        );

        let app = hamesh_app_create(b"root\0".as_ptr() as *const _);
        assert!(!app.is_null());

        // 3. Invalid role code (e.g. 99)
        let s = hamesh_app_add_component(
            app,
            b"root\0".as_ptr() as *const _,
            b"child1\0".as_ptr() as *const _,
            99,
            b"InvalidRole\0".as_ptr() as *const _,
        );
        assert_eq!(s, HameshStatus::InvalidRole);

        // 4. Missing parent
        let s = hamesh_app_add_component(
            app,
            b"ghost_parent\0".as_ptr() as *const _,
            b"child2\0".as_ptr() as *const _,
            1, // Button
            b"ValidButton\0".as_ptr() as *const _,
        );
        assert_eq!(s, HameshStatus::ParentNotFound);

        // 5. Normal add
        let s = hamesh_app_add_component(
            app,
            b"root\0".as_ptr() as *const _,
            b"child_ok\0".as_ptr() as *const _,
            1, // Button
            b"ValidButton\0".as_ptr() as *const _,
        );
        assert_eq!(s, HameshStatus::Ok);

        // 6. Duplicate ID check (prevents panic!)
        let s_dup = hamesh_app_add_component(
            app,
            b"root\0".as_ptr() as *const _,
            b"child_ok\0".as_ptr() as *const _,
            1,
            b"DuplicateButton\0".as_ptr() as *const _,
        );
        assert_eq!(s_dup, HameshStatus::DuplicateId);

        hamesh_app_destroy(app);
    }
}

#[test]
fn test_bat_14_event_polling_and_callback_isolation() {
    unsafe {
        let app = hamesh_app_create(b"root\0".as_ptr() as *const _);
        assert!(!app.is_null());

        // Register callback
        static mut CALLED: bool = false;
        static mut LAST_COMP: String = String::new();
        static mut LAST_TYPE: String = String::new();

        extern "C" fn on_event(
            _user_data: *mut std::ffi::c_void,
            comp_id: *const std::ffi::c_char,
            event_type: *const std::ffi::c_char,
            _payload: *const std::ffi::c_char,
        ) {
            unsafe {
                CALLED = true;
                LAST_COMP = std::ffi::CStr::from_ptr(comp_id).to_string_lossy().to_string();
                LAST_TYPE = std::ffi::CStr::from_ptr(event_type).to_string_lossy().to_string();
            }
        }

        let s = hamesh_app_set_event_callback(app, on_event, std::ptr::null_mut());
        assert_eq!(s, HameshStatus::Ok);

        // Queue event directly via handle
        {
            let handle = &*app;
            let mut queue = handle.event_queue.lock().unwrap();
            queue.push_back(hamesh_ffi::QueuedEvent {
                comp_id: "child_ok".to_string(),
                event_type: "click".to_string(),
                payload: "{\"test\": true}".to_string(),
            });
        }

        // Poll events - should invoke callback synchronously
        let polled = hamesh_app_poll_events(app);
        assert_eq!(polled, 1);
        assert!(CALLED);
        assert_eq!(LAST_COMP, "child_ok");
        assert_eq!(LAST_TYPE, "click");

        hamesh_app_destroy(app);
    }
}


