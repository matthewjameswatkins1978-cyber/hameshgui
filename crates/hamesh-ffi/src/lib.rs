//! hamesh-ffi: Stable C ABI for Hamesh GUI.
//!
//! Provides opaque handles and C-compatible exports (`extern "C"`) enabling any
//! interoperable language (Python, C, C++, Zig, C#) to create, mutate, inspect,
//! render, and run Hamesh GUI applications without embedding Rust-specific traits.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::{Arc, Mutex};

use hamesh_core::{App, Component, Role};
use hamesh_shell::{HameshApp, Shell, ShellConfig, ShellContext};

/// Status codes returned across the C ABI.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HameshStatus {
    Ok = 0,
    NullPointer = 1,
    InvalidUtf8 = 2,
    NotFound = 3,
    Error = 4,
}

/// Opaque App handle for foreign language callers.
pub struct HameshAppHandle {
    pub inner: Arc<Mutex<App>>,
    pub on_event: Option<extern "C" fn(*mut c_void, *const c_char, *const c_char)>,
    pub user_data: *mut c_void,
}

unsafe impl Send for HameshAppHandle {}
unsafe impl Sync for HameshAppHandle {}

/// Create a new Hamesh App instance with a root component.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_create(root_id: *const c_char) -> *mut HameshAppHandle {
    if root_id.is_null() {
        return std::ptr::null_mut();
    }
    let id_str = match CStr::from_ptr(root_id).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let app = App::new(id_str);
    let handle = Box::new(HameshAppHandle {
        inner: Arc::new(Mutex::new(app)),
        on_event: None,
        user_data: std::ptr::null_mut(),
    });
    Box::into_raw(handle)
}

/// Destroy an App instance and release all associated memory.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_destroy(app: *mut HameshAppHandle) {
    if !app.is_null() {
        drop(Box::from_raw(app));
    }
}

/// Get the current monotonic revision number of the app tree.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_get_revision(app: *mut HameshAppHandle) -> u64 {
    if app.is_null() {
        return 0;
    }
    let handle = &*app;
    let guard = match handle.inner.lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    guard.revision
}

/// Add or update a component in the app tree.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_add_component(
    app: *mut HameshAppHandle,
    parent_id: *const c_char,
    comp_id: *const c_char,
    role_code: c_int,
    name: *const c_char,
) -> HameshStatus {
    if app.is_null() || parent_id.is_null() || comp_id.is_null() || name.is_null() {
        return HameshStatus::NullPointer;
    }

    let parent_str = match CStr::from_ptr(parent_id).to_str() {
        Ok(s) => s,
        Err(_) => return HameshStatus::InvalidUtf8,
    };
    let id_str = match CStr::from_ptr(comp_id).to_str() {
        Ok(s) => s,
        Err(_) => return HameshStatus::InvalidUtf8,
    };
    let name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => return HameshStatus::InvalidUtf8,
    };

    let role = match role_code {
        0 => Role::Window,
        1 => Role::Panel,
        2 => Role::Header,
        3 => Role::Button,
        4 => Role::TextInput,
        5 => Role::Text,
        6 => Role::List,
        7 => Role::ListItem,
        8 => Role::Status,
        9 => Role::Overlay,
        _ => Role::Panel,
    };

    let handle = &*app;
    let mut guard = match handle.inner.lock() {
        Ok(g) => g,
        Err(_) => return HameshStatus::Error,
    };

    let comp = Component::new(id_str, role, name_str);
    guard.add(parent_str, comp);
    HameshStatus::Ok
}

/// Set a string property on an existing component.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_set_prop(
    app: *mut HameshAppHandle,
    comp_id: *const c_char,
    key: *const c_char,
    val: *const c_char,
) -> HameshStatus {
    if app.is_null() || comp_id.is_null() || key.is_null() || val.is_null() {
        return HameshStatus::NullPointer;
    }

    let id_str = match CStr::from_ptr(comp_id).to_str() {
        Ok(s) => s,
        Err(_) => return HameshStatus::InvalidUtf8,
    };
    let key_str = match CStr::from_ptr(key).to_str() {
        Ok(s) => s,
        Err(_) => return HameshStatus::InvalidUtf8,
    };
    let val_str = match CStr::from_ptr(val).to_str() {
        Ok(s) => s,
        Err(_) => return HameshStatus::InvalidUtf8,
    };

    let handle = &*app;
    let mut guard = match handle.inner.lock() {
        Ok(g) => g,
        Err(_) => return HameshStatus::Error,
    };

    if let Some(comp) = guard.get_mut(id_str) {
        comp.props.insert(key_str.to_string(), val_str.to_string());
        guard.revision += 1;
        HameshStatus::Ok
    } else {
        HameshStatus::NotFound
    }
}

/// Register a native foreign-language callback for UI events.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_set_event_callback(
    app: *mut HameshAppHandle,
    callback: extern "C" fn(*mut c_void, *const c_char, *const c_char),
    user_data: *mut c_void,
) -> HameshStatus {
    if app.is_null() {
        return HameshStatus::NullPointer;
    }
    let handle = &mut *app;
    handle.on_event = Some(callback);
    handle.user_data = user_data;
    HameshStatus::Ok
}

/// A Shell bridge proxy that drives an external HameshAppHandle.
struct FfiBridgeApp {
    handle: Arc<Mutex<App>>,
    on_event: Option<extern "C" fn(*mut c_void, *const c_char, *const c_char)>,
    user_data: *mut c_void,
}

unsafe impl Send for FfiBridgeApp {}

impl HameshApp for FfiBridgeApp {
    fn init(&mut self, ctx: &mut ShellContext) {
        if let Ok(guard) = self.handle.lock() {
            *ctx.app = guard.clone();
        }
        ctx.request_redraw();
    }

    fn on_mouse_click(&mut self, ctx: &mut ShellContext, _button: winit::event::MouseButton, state: winit::event::ElementState) {
        if state != winit::event::ElementState::Pressed {
            return;
        }
        let (x, y) = ctx.cursor;
        let (_, scene) = self.view(ctx.app, ctx.size, 1);
        if let Some(node) = scene.nodes.iter().rev().find(|n| n.rect.contains(x, y)) {
            if let Some(cb) = self.on_event {
                let id_c = CString::new(node.id.clone()).unwrap_or_default();
                let event_c = CString::new("click").unwrap_or_default();
                cb(self.user_data, id_c.as_ptr(), event_c.as_ptr());
            }
        }
        // Sync any mutations back to ctx.app
        if let Ok(guard) = self.handle.lock() {
            if guard.revision != ctx.app.revision {
                *ctx.app = guard.clone();
                ctx.request_redraw();
            }
        }
    }

    fn on_idle(&mut self, ctx: &mut ShellContext) {
        // Poll for mutations made by foreign language threads:
        if let Ok(guard) = self.handle.lock() {
            if guard.revision != ctx.app.revision {
                *ctx.app = guard.clone();
                ctx.request_redraw();
            }
        }
    }
}

/// Launch and run a native OS window driving the provided Hamesh App instance.
#[no_mangle]
pub unsafe extern "C" fn hamesh_run(
    app: *mut HameshAppHandle,
    title: *const c_char,
    width: u32,
    height: u32,
    smoke_seconds: u64,
) -> HameshStatus {
    if app.is_null() {
        return HameshStatus::NullPointer;
    }
    let title_str = if !title.is_null() {
        CStr::from_ptr(title).to_str().unwrap_or("Hamesh Window")
    } else {
        "Hamesh Window"
    };

    let handle = &*app;
    let bridge = FfiBridgeApp {
        handle: Arc::clone(&handle.inner),
        on_event: handle.on_event,
        user_data: handle.user_data,
    };

    let config = ShellConfig {
        title: title_str.to_string(),
        width,
        height,
        min_width: 320,
        min_height: 240,
        enable_inspect_server: true,
        inspect_port: None,
        smoke_seconds: if smoke_seconds > 0 { Some(smoke_seconds) } else { None },
    };

    match Shell::run(bridge, config) {
        Ok(()) => HameshStatus::Ok,
        Err(_) => HameshStatus::Error,
    }
}
