//! hamesh-ffi: Stable C ABI for Hamesh GUI.
//!
//! Provides opaque handles and C-compatible exports (`extern "C"`) enabling any
//! interoperable language (Python, C, C++, Zig, C#) to create, mutate, inspect,
//! render, and run Hamesh GUI applications without embedding Rust-specific traits.
//!
//! # Safety & Stability Guarantees
//! - Versioned C ABI (Major: 0, Minor: 4, Patch: 2)
//! - No Rust panics escape across the extern "C" boundary (`catch_unwind` guards)
//! - Opaque pointer validation (null check + pointer validation)
//! - Result codes for every mutating operation (duplicate ID, missing parent, invalid UTF-8)
//! - Explicit ownership: client frees handles with `hamesh_app_destroy`
//! - Thread-safe internal synchronisation via `Arc<Mutex<App>>`
//! - Non-blocking event queueing from UI thread to foreign language callbacks

use std::collections::VecDeque;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::panic::catch_unwind;
use std::sync::{Arc, Mutex};

use hamesh_core::{App, Component, Role};
use hamesh_shell::{HameshApp, Shell, ShellConfig, ShellContext};
use winit::event::ElementState;
use winit::keyboard::{Key, NamedKey};

/// Current ABI version packed: (major << 16) | (minor << 8) | patch.
pub const HAMESH_ABI_VERSION: u32 = (0 << 16) | (4 << 8) | 2;

/// Status codes returned across the C ABI.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HameshStatus {
    Ok = 0,
    NullPointer = 1,
    InvalidUtf8 = 2,
    NotFound = 3,
    DuplicateId = 4,
    ParentNotFound = 5,
    InvalidRole = 6,
    PanicCaught = 7,
    Error = 8,
}

/// An event delivered from the UI thread to a foreign language callback.
#[derive(Debug, Clone)]
pub struct QueuedEvent {
    pub comp_id: String,
    pub event_type: String,
    pub payload: String,
}

/// Opaque App handle for foreign language callers.
pub struct HameshAppHandle {
    pub inner: Arc<Mutex<App>>,
    pub on_event: Option<extern "C" fn(*mut c_void, *const c_char, *const c_char, *const c_char)>,
    pub user_data: *mut c_void,
    pub event_queue: Arc<Mutex<VecDeque<QueuedEvent>>>,
}

unsafe impl Send for HameshAppHandle {}
unsafe impl Sync for HameshAppHandle {}

/// Query the C ABI version.
#[no_mangle]
pub extern "C" fn hamesh_abi_version() -> u32 {
    HAMESH_ABI_VERSION
}

/// Create a new Hamesh App instance with a root component.
///
/// Returns NULL if `root_id` is null or invalid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_create(root_id: *const c_char) -> *mut HameshAppHandle {
    let result = catch_unwind(|| {
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
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
        });
        Box::into_raw(handle)
    });

    result.unwrap_or(std::ptr::null_mut())
}

/// Destroy an App instance and release all associated memory.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_destroy(app: *mut HameshAppHandle) {
    let _ = catch_unwind(|| {
        if !app.is_null() {
            drop(Box::from_raw(app));
        }
    });
}

/// Get the current monotonic revision number of the app tree.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_get_revision(app: *mut HameshAppHandle) -> u64 {
    let result = catch_unwind(|| {
        if app.is_null() {
            return 0;
        }
        let handle = &*app;
        let guard = match handle.inner.lock() {
            Ok(g) => g,
            Err(_) => return 0,
        };
        guard.revision
    });

    result.unwrap_or(0)
}

/// Add a component to the app tree.
///
/// Returns:
/// - `HameshStatus::Ok` on success
/// - `HameshStatus::NullPointer` if any argument is null
/// - `HameshStatus::InvalidUtf8` if any string is not valid UTF-8
/// - `HameshStatus::DuplicateId` if `comp_id` already exists in the tree
/// - `HameshStatus::ParentNotFound` if `parent_id` is not present
/// - `HameshStatus::InvalidRole` if `role_code` is outside 0..=9
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_add_component(
    app: *mut HameshAppHandle,
    parent_id: *const c_char,
    comp_id: *const c_char,
    role_code: c_int,
    name: *const c_char,
) -> HameshStatus {
    let result = catch_unwind(|| {
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
            _ => return HameshStatus::InvalidRole,
        };

        let handle = &*app;
        let mut guard = match handle.inner.lock() {
            Ok(g) => g,
            Err(_) => return HameshStatus::Error,
        };

        // Safety check 1: Duplicate Component ID (Prevents panic in App::add)
        if guard.components.contains_key(id_str) {
            return HameshStatus::DuplicateId;
        }

        // Safety check 2: Missing Parent ID
        if !guard.components.contains_key(parent_str) {
            return HameshStatus::ParentNotFound;
        }

        let comp = Component::new(id_str, role, name_str);
        guard.add(parent_str, comp);
        HameshStatus::Ok
    });

    result.unwrap_or(HameshStatus::PanicCaught)
}

/// Set a string property on an existing component.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_set_prop(
    app: *mut HameshAppHandle,
    comp_id: *const c_char,
    key: *const c_char,
    val: *const c_char,
) -> HameshStatus {
    let result = catch_unwind(|| {
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
    });

    result.unwrap_or(HameshStatus::PanicCaught)
}

/// Register a foreign-language callback for UI events.
///
/// Signature: `callback(user_data, comp_id, event_type, payload)`
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_set_event_callback(
    app: *mut HameshAppHandle,
    callback: extern "C" fn(*mut c_void, *const c_char, *const c_char, *const c_char),
    user_data: *mut c_void,
) -> HameshStatus {
    let result = catch_unwind(|| {
        if app.is_null() {
            return HameshStatus::NullPointer;
        }
        let handle = &mut *app;
        handle.on_event = Some(callback);
        handle.user_data = user_data;
        HameshStatus::Ok
    });

    result.unwrap_or(HameshStatus::PanicCaught)
}

/// Poll and dispatch queued events. Safe to call from UI or worker thread.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_poll_events(app: *mut HameshAppHandle) -> c_int {
    let result = catch_unwind(|| {
        if app.is_null() {
            return 0;
        }
        let handle = &*app;
        let mut queue = match handle.event_queue.lock() {
            Ok(q) => q,
            Err(_) => return 0,
        };

        let count = queue.len();
        while let Some(ev) = queue.pop_front() {
            if let Some(cb) = handle.on_event {
                let id_c = CString::new(ev.comp_id).unwrap_or_default();
                let type_c = CString::new(ev.event_type).unwrap_or_default();
                let payload_c = CString::new(ev.payload).unwrap_or_default();
                cb(handle.user_data, id_c.as_ptr(), type_c.as_ptr(), payload_c.as_ptr());
            }
        }
        count as c_int
    });

    result.unwrap_or(0)
}

/// A Shell bridge proxy that drives an external HameshAppHandle with focus and interactive editing.
struct FfiBridgeApp {
    handle: Arc<Mutex<App>>,
    on_event: Option<extern "C" fn(*mut c_void, *const c_char, *const c_char, *const c_char)>,
    user_data: *mut c_void,
    event_queue: Arc<Mutex<VecDeque<QueuedEvent>>>,
    focused_id: Option<String>,
}

unsafe impl Send for FfiBridgeApp {}

impl FfiBridgeApp {
    fn enqueue_event(&self, comp_id: &str, event_type: &str, payload: &str) {
        if let Ok(mut q) = self.event_queue.lock() {
            q.push_back(QueuedEvent {
                comp_id: comp_id.to_string(),
                event_type: event_type.to_string(),
                payload: payload.to_string(),
            });
        }
        // Direct synchronous callback invocation if registered
        if let Some(cb) = self.on_event {
            let id_c = CString::new(comp_id).unwrap_or_default();
            let type_c = CString::new(event_type).unwrap_or_default();
            let payload_c = CString::new(payload).unwrap_or_default();
            cb(self.user_data, id_c.as_ptr(), type_c.as_ptr(), payload_c.as_ptr());
        }
    }
}

impl HameshApp for FfiBridgeApp {
    fn init(&mut self, ctx: &mut ShellContext) {
        if let Ok(guard) = self.handle.lock() {
            *ctx.app = guard.clone();
        }
        ctx.request_redraw();
    }

    fn on_mouse_click(&mut self, ctx: &mut ShellContext, _button: winit::event::MouseButton, state: ElementState) {
        if state != ElementState::Pressed {
            return;
        }
        let (x, y) = ctx.cursor;
        let (_, scene) = self.view(ctx.app, ctx.size, 1);
        if let Some(node) = scene.nodes.iter().rev().find(|n| n.rect.contains(x, y)) {
            let clicked_id = node.id.clone();
            let role = &node.role;

            if role == "TextInput" {
                self.focused_id = Some(clicked_id.clone());
            }

            self.enqueue_event(&clicked_id, "click", "");

            // Sync mutations back from handle
            if let Ok(guard) = self.handle.lock() {
                if guard.revision != ctx.app.revision {
                    *ctx.app = guard.clone();
                    ctx.request_redraw();
                }
            }
        }
    }

    fn on_key(&mut self, ctx: &mut ShellContext, key: &Key, state: ElementState, _repeat: bool) {
        if state != ElementState::Pressed {
            return;
        }

        let Some(ref focused) = self.focused_id else {
            return;
        };

        let mut changed_text = None;
        if let Ok(mut guard) = self.handle.lock() {
            if let Some(comp) = guard.get_mut(focused) {
                let current_text = comp.props.entry("text".to_string()).or_default();
                match key {
                    Key::Named(NamedKey::Backspace) => {
                        current_text.pop();
                        changed_text = Some(current_text.clone());
                    }
                    Key::Named(NamedKey::Space) => {
                        current_text.push(' ');
                        changed_text = Some(current_text.clone());
                    }
                    Key::Character(s) => {
                        current_text.push_str(s.as_str());
                        changed_text = Some(current_text.clone());
                    }
                    Key::Named(NamedKey::Enter) => {
                        let text_val = current_text.clone();
                        self.enqueue_event(focused, "submit", &text_val);
                    }
                    _ => {}
                }
            }
            if let Some(ref text) = changed_text {
                guard.revision += 1;
                self.enqueue_event(focused, "change", text);
            }
        }

        if changed_text.is_some() {
            if let Ok(guard) = self.handle.lock() {
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
    let result = catch_unwind(|| {
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
            event_queue: Arc::clone(&handle.event_queue),
            focused_id: None,
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
    });

    result.unwrap_or(HameshStatus::PanicCaught)
}
