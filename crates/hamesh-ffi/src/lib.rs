//! hamesh-ffi: Stable C ABI for Hamesh GUI.
//!
//! Provides opaque handles and C-compatible exports (`extern "C"`) enabling any
//! interoperable language (Python, C, C++, Zig, C#) to create, mutate, inspect,
//! render, and run Hamesh GUI applications without embedding Rust-specific traits.
//!
//! # Safety & Stability Guarantees
//! - Versioned C ABI (Major: 0, Minor: 4, Patch: 2)
//! - No Rust panics escape across the extern "C" boundary (`catch_unwind` guards)
//! - Explicit ownership: client creates handles with `hamesh_app_create` and must free them with `hamesh_app_destroy`
//! - Result codes for every mutating operation (`HameshStatus::Ok`, `NullPointer`, `InvalidUtf8`, `DuplicateId`, `ParentNotFound`, `InvalidRole`)
//! - Thread-safe internal synchronisation via `Arc<Mutex<App>>`
//! - Deadlock-free event delivery: all internal locks are released before invoking foreign callbacks
//! - Exactly-once event dispatch: events are dispatched synchronously to the registered callback, or queued for polling only when no callback is registered
//!
//! # Lifecycle, Threading, and Pointer Safety Contract
//! 1. **Callback Ownership & Lifetime**:
//!    - Foreign callers own the callback function pointer and `user_data` context pointer.
//!    - The foreign language runtime is strictly responsible for ensuring the callback remains
//!      valid and memory-backed as long as it is registered.
//!    - Callers can unregister callbacks at any time by calling `hamesh_app_set_event_callback(app, NULL, NULL)`.
//!    - `hamesh_app_destroy` explicitly unregisters and zeroes the callback pointer before freeing handle memory,
//!      guaranteeing that teardown cannot invoke deallocated foreign callbacks.
//! 2. **Pointer Validity & Boundaries**:
//!    - All pointers passed into the C ABI must be valid, aligned pointers previously returned by `hamesh_app_create`
//!      or valid null-terminated UTF-8 C string buffers.
//!    - Null pointers are defensively checked and rejected with `HameshStatus::NullPointer`.
//!    - Hamesh does NOT claim arbitrary dangling-pointer, use-after-free, or double-free safety against corrupted foreign memory.
//! 3. **Thread Affinity & Concurrency**:
//!    - `hamesh_run` runs the native OS event loop and MUST be invoked from the main application thread.
//!    - Component mutations (`hamesh_app_add_component`, `hamesh_app_set_prop`, `hamesh_app_set_visible`)
//!      are thread-safe and can be called from background worker threads; they safely serialize on internal mutexes.
//!    - Reentrant or concurrent calls to `hamesh_run` using the same handle are unsupported.
//! 4. **Polling & Callback Event Dispatch Contract**:
//!    - Push Mode: When a callback is registered (`hamesh_app_set_event_callback`), UI events dispatch directly
//!      to the foreign callback with **zero internal locks held**.
//!    - Poll Mode: When no callback is registered, events accumulate in a bounded FIFO queue (`event_queue`).
//!      Events are retrieved one-by-one via `hamesh_app_poll_event` into caller-provided bounded buffers.
//!    - No silent event loss: `hamesh_app_poll_events` returns pending queue count without dropping events if no callback is registered.
//!    - No duplicate delivery: Events are either dispatched to the callback or queued for polling, never both.

#![allow(clippy::missing_safety_doc, clippy::identity_op)]

use std::collections::VecDeque;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::panic::catch_unwind;
use std::sync::atomic::{AtomicBool, Ordering};
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

/// An event delivered from the UI thread to a foreign language callback or poll queue.
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
    pub focused_id: Arc<Mutex<Option<String>>>,
    pub exit_requested: Arc<AtomicBool>,
}

unsafe impl Send for HameshAppHandle {}
unsafe impl Sync for HameshAppHandle {}

impl HameshAppHandle {
    pub fn dispatch_event(&self, comp_id: &str, event_type: &str, payload: &str) {
        if let Some(cb) = self.on_event {
            let id_c = CString::new(comp_id).unwrap_or_default();
            let type_c = CString::new(event_type).unwrap_or_default();
            let payload_c = CString::new(payload).unwrap_or_default();
            cb(
                self.user_data,
                id_c.as_ptr(),
                type_c.as_ptr(),
                payload_c.as_ptr(),
            );
        } else if let Ok(mut q) = self.event_queue.lock() {
            q.push_back(QueuedEvent {
                comp_id: comp_id.to_string(),
                event_type: event_type.to_string(),
                payload: payload.to_string(),
            });
        }
    }
}

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
            focused_id: Arc::new(Mutex::new(None)),
            exit_requested: Arc::new(AtomicBool::new(false)),
        });
        Box::into_raw(handle)
    });

    result.unwrap_or(std::ptr::null_mut())
}

/// Destroy an App instance and release all associated memory.
///
/// Unregisters and zeroes any active event callbacks before deallocation,
/// ensuring that shutdown does not invoke deallocated foreign memory.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_destroy(app: *mut HameshAppHandle) {
    let _ = catch_unwind(|| {
        if !app.is_null() {
            let mut handle = Box::from_raw(app);
            handle.on_event = None;
            handle.user_data = std::ptr::null_mut();
            drop(handle);
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

        // Duplicate Component ID pre-check (Prevents panic in App::add)
        if guard.components.contains_key(id_str) {
            return HameshStatus::DuplicateId;
        }

        // Missing Parent ID pre-check
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
///
/// Note: Setting key "visible" also updates `component.visible` boolean directly.
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
            if key_str == "visible" {
                comp.visible = val_str == "true" || val_str == "1";
            }
            guard.revision += 1;
            HameshStatus::Ok
        } else {
            HameshStatus::NotFound
        }
    });

    result.unwrap_or(HameshStatus::PanicCaught)
}

/// Set visibility on an existing component.
///
/// When `visible` is 0 (false), the component and all its descendants are marked
/// effectively invisible: they do not paint and are ignored by `hamesh_scene::hit_test`.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_set_visible(
    app: *mut HameshAppHandle,
    comp_id: *const c_char,
    visible: c_int,
) -> HameshStatus {
    let result = catch_unwind(|| {
        if app.is_null() || comp_id.is_null() {
            return HameshStatus::NullPointer;
        }

        let id_str = match CStr::from_ptr(comp_id).to_str() {
            Ok(s) => s,
            Err(_) => return HameshStatus::InvalidUtf8,
        };

        let handle = &*app;
        let mut guard = match handle.inner.lock() {
            Ok(g) => g,
            Err(_) => return HameshStatus::Error,
        };

        if let Some(comp) = guard.get_mut(id_str) {
            comp.visible = visible != 0;
            comp.props.insert(
                "visible".to_string(),
                if comp.visible {
                    "true".to_string()
                } else {
                    "false".to_string()
                },
            );
            guard.revision += 1;
            HameshStatus::Ok
        } else {
            HameshStatus::NotFound
        }
    });

    result.unwrap_or(HameshStatus::PanicCaught)
}

/// Request graceful window shutdown through supported Hamesh shell operation.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_request_exit(app: *mut HameshAppHandle) -> HameshStatus {
    let result = catch_unwind(|| {
        if app.is_null() {
            return HameshStatus::NullPointer;
        }
        let handle = &*app;
        handle.exit_requested.store(true, Ordering::SeqCst);
        HameshStatus::Ok
    });

    result.unwrap_or(HameshStatus::PanicCaught)
}

/// Copy a Rust UTF-8 string into a caller-provided bounded C buffer with guaranteed null termination.
unsafe fn copy_str_to_c_buf(src: &str, dst: *mut c_char, dst_len: usize) {
    if dst.is_null() || dst_len == 0 {
        return;
    }
    let bytes = src.as_bytes();
    let copy_len = bytes.len().min(dst_len.saturating_sub(1));
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst as *mut u8, copy_len);
    *dst.add(copy_len) = 0;
}

/// Register a foreign-language callback for UI events, or unregister by passing `None` / NULL.
///
/// Signature: `callback(user_data, comp_id, event_type, payload)`
///
/// Contract: Events are dispatched synchronously and exactly once to this callback
/// after internal application locks have been released, guaranteeing safe reentrancy
/// (`set_prop`, `set_visible`, `request_exit`).
///
/// Passing `None` / NULL safely unregisters the callback and clears `user_data`.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_set_event_callback(
    app: *mut HameshAppHandle,
    callback: Option<extern "C" fn(*mut c_void, *const c_char, *const c_char, *const c_char)>,
    user_data: *mut c_void,
) -> HameshStatus {
    let result = catch_unwind(|| {
        if app.is_null() {
            return HameshStatus::NullPointer;
        }
        let handle = &mut *app;
        handle.on_event = callback;
        handle.user_data = if callback.is_some() {
            user_data
        } else {
            std::ptr::null_mut()
        };
        HameshStatus::Ok
    });

    result.unwrap_or(HameshStatus::PanicCaught)
}

/// Retrieve a single queued event from the application event queue into caller-provided bounded buffers.
///
/// Writes null-terminated UTF-8 strings into the provided buffers:
/// - `comp_id_buf` / `comp_id_len`: buffer for target component ID.
/// - `event_type_buf` / `event_type_len`: buffer for event type ("click", "change", "submit").
/// - `payload_buf` / `payload_len`: buffer for event payload.
///
/// # Returns
/// - `1`: An event was retrieved and written to the output buffers.
/// - `0`: The event queue was empty (no event available).
/// - `-1` (`NullPointer`): `app` or any required buffer pointer was null.
/// - `-8` (`Error`): Buffer capacity was 0.
/// - `-7` (`PanicCaught`): Internal panic caught safely.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_poll_event(
    app: *mut HameshAppHandle,
    comp_id_buf: *mut c_char,
    comp_id_len: usize,
    event_type_buf: *mut c_char,
    event_type_len: usize,
    payload_buf: *mut c_char,
    payload_len: usize,
) -> c_int {
    let result = catch_unwind(|| {
        if app.is_null()
            || comp_id_buf.is_null()
            || event_type_buf.is_null()
            || payload_buf.is_null()
        {
            return -(HameshStatus::NullPointer as c_int);
        }
        if comp_id_len == 0 || event_type_len == 0 || payload_len == 0 {
            return -(HameshStatus::Error as c_int);
        }

        let handle = &*app;
        let mut queue = match handle.event_queue.lock() {
            Ok(q) => q,
            Err(_) => return -(HameshStatus::Error as c_int),
        };

        if let Some(ev) = queue.pop_front() {
            copy_str_to_c_buf(&ev.comp_id, comp_id_buf, comp_id_len);
            copy_str_to_c_buf(&ev.event_type, event_type_buf, event_type_len);
            copy_str_to_c_buf(&ev.payload, payload_buf, payload_len);
            1
        } else {
            0
        }
    });

    result.unwrap_or(-(HameshStatus::PanicCaught as c_int))
}

/// Flush and dispatch queued events to the registered callback, or report the count of pending events.
///
/// If a callback is registered (`hamesh_app_set_event_callback`):
/// Drains all currently queued events into a local buffer, releases internal locks,
/// and invokes the callback for each event with **zero internal locks held**.
/// Returns the number of events dispatched.
///
/// If NO callback is registered:
/// Events remain safely queued in `event_queue` for retrieval via `hamesh_app_poll_event`.
/// Returns the number of pending events without consuming them (no silent event loss).
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_poll_events(app: *mut HameshAppHandle) -> c_int {
    let result = catch_unwind(|| {
        if app.is_null() {
            return 0;
        }
        let handle = &*app;
        if let Some(cb) = handle.on_event {
            // Drain queue into a local Vec so the lock is dropped BEFORE invoking foreign callback
            let events: Vec<QueuedEvent> = {
                let mut queue = match handle.event_queue.lock() {
                    Ok(q) => q,
                    Err(_) => return 0,
                };
                queue.drain(..).collect()
            };

            let count = events.len();
            for ev in events {
                let id_c = CString::new(ev.comp_id).unwrap_or_default();
                let type_c = CString::new(ev.event_type).unwrap_or_default();
                let payload_c = CString::new(ev.payload).unwrap_or_default();
                cb(
                    handle.user_data,
                    id_c.as_ptr(),
                    type_c.as_ptr(),
                    payload_c.as_ptr(),
                );
            }
            count as c_int
        } else {
            let queue = match handle.event_queue.lock() {
                Ok(q) => q,
                Err(_) => return 0,
            };
            queue.len() as c_int
        }
    });

    result.unwrap_or(0)
}

/// Query the layout bounding rect and visibility of a component by ID.
///
/// Computes layout for the given viewport width and height, looks up the component in the scene,
/// and writes its bounding box to the output pointers.
///
/// Returns `HameshStatus::Ok` on success, or `HameshStatus::NotFound` if the component does not
/// exist in the scene or is marked invisible.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_get_component_bounds(
    app: *mut HameshAppHandle,
    comp_id: *const c_char,
    width: u32,
    height: u32,
    out_x: *mut f32,
    out_y: *mut f32,
    out_w: *mut f32,
    out_h: *mut f32,
) -> HameshStatus {
    let result = catch_unwind(|| {
        if app.is_null()
            || comp_id.is_null()
            || out_x.is_null()
            || out_y.is_null()
            || out_w.is_null()
            || out_h.is_null()
        {
            return HameshStatus::NullPointer;
        }

        let id_str = match CStr::from_ptr(comp_id).to_str() {
            Ok(s) => s,
            Err(_) => return HameshStatus::InvalidUtf8,
        };

        let handle = &*app;
        let guard = match handle.inner.lock() {
            Ok(g) => g,
            Err(_) => return HameshStatus::Error,
        };

        let w = if width > 0 { width as f32 } else { 860.0 };
        let h = if height > 0 { height as f32 } else { 600.0 };
        let layout = hamesh_layout::compute(&guard, w, h);
        let scene = hamesh_scene::build(&guard, &layout, guard.revision);

        if let Some(node) = scene.nodes.iter().find(|n| n.id == id_str) {
            if !node.visible {
                return HameshStatus::NotFound;
            }
            *out_x = node.rect.x;
            *out_y = node.rect.y;
            *out_w = node.rect.w;
            *out_h = node.rect.h;
            HameshStatus::Ok
        } else {
            HameshStatus::NotFound
        }
    });

    result.unwrap_or(HameshStatus::PanicCaught)
}

/// Set keyboard focus on a specific component by ID.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_set_focus(
    app: *mut HameshAppHandle,
    comp_id: *const c_char,
) -> HameshStatus {
    let result = catch_unwind(|| {
        if app.is_null() || comp_id.is_null() {
            return HameshStatus::NullPointer;
        }
        let id_str = match CStr::from_ptr(comp_id).to_str() {
            Ok(s) => s,
            Err(_) => return HameshStatus::InvalidUtf8,
        };
        let handle = &*app;
        let guard = match handle.inner.lock() {
            Ok(g) => g,
            Err(_) => return HameshStatus::Error,
        };
        if !guard.components.contains_key(id_str) {
            return HameshStatus::NotFound;
        }
        drop(guard);
        if let Ok(mut f) = handle.focused_id.lock() {
            *f = Some(id_str.to_string());
        }
        HameshStatus::Ok
    });
    result.unwrap_or(HameshStatus::PanicCaught)
}

/// Programmatically simulate a mouse click at (x, y) coordinates.
///
/// Uses Hamesh's visibility-aware and clipping-aware hit-testing.
/// If a component is hit, dispatches a "click" event outside of locks.
/// Returns `HameshStatus::Ok` if a visible component was hit, or `HameshStatus::NotFound` if not.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_simulate_click(
    app: *mut HameshAppHandle,
    x: f32,
    y: f32,
    width: u32,
    height: u32,
) -> HameshStatus {
    let result = catch_unwind(|| {
        if app.is_null() {
            return HameshStatus::NullPointer;
        }
        let handle = &*app;
        let mut hit_id_opt = None;
        let mut is_text_input = false;

        {
            let guard = match handle.inner.lock() {
                Ok(g) => g,
                Err(_) => return HameshStatus::Error,
            };
            let w = if width > 0 { width as f32 } else { 800.0 };
            let h = if height > 0 { height as f32 } else { 600.0 };
            let layout = hamesh_layout::compute(&guard, w, h);
            let scene = hamesh_scene::build(&guard, &layout, guard.revision);
            if let Some(id) = hamesh_scene::hit_test(&guard, &scene, x, y) {
                if let Some(comp) = guard.components.get(&id) {
                    if comp.role == Role::TextInput {
                        is_text_input = true;
                    }
                }
                hit_id_opt = Some(id);
            }
        }

        if let Some(ref hit_id) = hit_id_opt {
            if is_text_input {
                if let Ok(mut f) = handle.focused_id.lock() {
                    *f = Some(hit_id.clone());
                }
            }
            // Dispatched with ZERO locks held
            handle.dispatch_event(hit_id, "click", "");
            HameshStatus::Ok
        } else {
            HameshStatus::NotFound
        }
    });

    result.unwrap_or(HameshStatus::PanicCaught)
}

/// Programmatically simulate a click on a component by ID.
///
/// Uses Hamesh's scene construction and visibility-aware, clipping-aware hit-testing.
/// If the component is invisible, covered, or clipped away, returns `HameshStatus::NotFound`.
/// If visible and hit-testable, dispatches "click" and returns `HameshStatus::Ok`.
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_simulate_click_component(
    app: *mut HameshAppHandle,
    comp_id: *const c_char,
    width: u32,
    height: u32,
) -> HameshStatus {
    let result = catch_unwind(|| {
        if app.is_null() || comp_id.is_null() {
            return HameshStatus::NullPointer;
        }
        let id_str = match CStr::from_ptr(comp_id).to_str() {
            Ok(s) => s,
            Err(_) => return HameshStatus::InvalidUtf8,
        };

        let handle = &*app;
        let mut hit_id_opt = None;
        let mut is_text_input = false;

        {
            let guard = match handle.inner.lock() {
                Ok(g) => g,
                Err(_) => return HameshStatus::Error,
            };
            let w = if width > 0 { width as f32 } else { 860.0 };
            let h = if height > 0 { height as f32 } else { 600.0 };
            let layout = hamesh_layout::compute(&guard, w, h);
            let scene = hamesh_scene::build(&guard, &layout, guard.revision);

            // Find target node in scene
            if let Some(node) = scene.nodes.iter().find(|n| n.id == id_str) {
                // If effectively invisible, cannot receive clicks:
                if !node.visible {
                    return HameshStatus::NotFound;
                }
                let cx = node.rect.x + (node.rect.w / 2.0).max(1.0);
                let cy = node.rect.y + (node.rect.h / 2.0).max(1.0);

                if let Some(hit) = hamesh_scene::hit_test(&guard, &scene, cx, cy) {
                    if hit == id_str {
                        if let Some(comp) = guard.components.get(id_str) {
                            if comp.role == Role::TextInput {
                                is_text_input = true;
                            }
                        }
                        hit_id_opt = Some(id_str.to_string());
                    }
                }
            }
        }

        if let Some(ref hit_id) = hit_id_opt {
            if is_text_input {
                if let Ok(mut f) = handle.focused_id.lock() {
                    *f = Some(hit_id.clone());
                }
            }
            handle.dispatch_event(hit_id, "click", "");
            HameshStatus::Ok
        } else {
            HameshStatus::NotFound
        }
    });

    result.unwrap_or(HameshStatus::PanicCaught)
}

/// Programmatically simulate keyboard input into the currently focused component.
///
/// Actions supported:
/// - "char:<string>": inserts text characters
/// - "backspace": deletes preceding character
/// - "space": inserts space
/// - "enter": dispatches submit event
#[no_mangle]
pub unsafe extern "C" fn hamesh_app_simulate_key(
    app: *mut HameshAppHandle,
    key_action: *const c_char,
) -> HameshStatus {
    let result = catch_unwind(|| {
        if app.is_null() || key_action.is_null() {
            return HameshStatus::NullPointer;
        }
        let act_str = match CStr::from_ptr(key_action).to_str() {
            Ok(s) => s,
            Err(_) => return HameshStatus::InvalidUtf8,
        };

        let handle = &*app;
        let focused_opt = handle.focused_id.lock().ok().and_then(|f| f.clone());
        let Some(focused) = focused_opt else {
            return HameshStatus::NotFound;
        };

        let mut event_to_dispatch: Option<(&'static str, String)> = None;
        {
            let mut guard = match handle.inner.lock() {
                Ok(g) => g,
                Err(_) => return HameshStatus::Error,
            };

            if let Some(comp) = guard.get_mut(&focused) {
                let current_text = comp.props.entry("text".to_string()).or_default();
                if act_str == "backspace" {
                    current_text.pop();
                    event_to_dispatch = Some(("change", current_text.clone()));
                } else if act_str == "space" {
                    current_text.push(' ');
                    event_to_dispatch = Some(("change", current_text.clone()));
                } else if act_str == "enter" {
                    event_to_dispatch = Some(("submit", current_text.clone()));
                } else if let Some(chars) = act_str.strip_prefix("char:") {
                    current_text.push_str(chars);
                    event_to_dispatch = Some(("change", current_text.clone()));
                } else if let Some(text) = act_str.strip_prefix("text:") {
                    current_text.push_str(text);
                    event_to_dispatch = Some(("change", current_text.clone()));
                }
                if event_to_dispatch.is_some() {
                    guard.revision += 1;
                }
            }
            // Lock is explicitly released before callback dispatch!
        }

        if let Some((event_type, payload)) = event_to_dispatch {
            handle.dispatch_event(&focused, event_type, &payload);
        }

        HameshStatus::Ok
    });

    result.unwrap_or(HameshStatus::PanicCaught)
}

/// A Shell bridge proxy that drives an external HameshAppHandle with focus and interactive editing.
struct FfiBridgeApp {
    handle: Arc<Mutex<App>>,
    on_event: Option<extern "C" fn(*mut c_void, *const c_char, *const c_char, *const c_char)>,
    user_data: *mut c_void,
    event_queue: Arc<Mutex<VecDeque<QueuedEvent>>>,
    focused_id: Arc<Mutex<Option<String>>>,
    exit_requested: Arc<AtomicBool>,
}

unsafe impl Send for FfiBridgeApp {}

impl FfiBridgeApp {
    fn dispatch_event(&self, comp_id: &str, event_type: &str, payload: &str) {
        if let Some(cb) = self.on_event {
            let id_c = CString::new(comp_id).unwrap_or_default();
            let type_c = CString::new(event_type).unwrap_or_default();
            let payload_c = CString::new(payload).unwrap_or_default();
            cb(
                self.user_data,
                id_c.as_ptr(),
                type_c.as_ptr(),
                payload_c.as_ptr(),
            );
        } else if let Ok(mut q) = self.event_queue.lock() {
            q.push_back(QueuedEvent {
                comp_id: comp_id.to_string(),
                event_type: event_type.to_string(),
                payload: payload.to_string(),
            });
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

    fn on_mouse_click(
        &mut self,
        ctx: &mut ShellContext,
        button: winit::event::MouseButton,
        state: ElementState,
    ) {
        if state != ElementState::Pressed || button != winit::event::MouseButton::Left {
            return;
        }

        if self.exit_requested.load(Ordering::SeqCst) {
            ctx.exit();
            return;
        }

        let (x, y) = ctx.cursor;
        let (_, scene) = self.view(ctx.app, ctx.size, 1);

        // Visibility-aware and clipping-aware hit test:
        if let Some(clicked_id) = hamesh_scene::hit_test(ctx.app, &scene, x, y) {
            if let Some(comp) = ctx.app.components.get(&clicked_id) {
                if comp.role == Role::TextInput {
                    if let Ok(mut f) = self.focused_id.lock() {
                        *f = Some(clicked_id.clone());
                    }
                }
            }

            // Phase 1: Dispatch event with ZERO internal locks held!
            self.dispatch_event(&clicked_id, "click", "");

            // Phase 2: Sync any mutations performed during foreign callback back to ShellContext
            if let Ok(guard) = self.handle.lock() {
                if guard.revision != ctx.app.revision {
                    *ctx.app = guard.clone();
                    ctx.request_redraw();
                }
            }

            if self.exit_requested.load(Ordering::SeqCst) {
                ctx.exit();
            }
        }
    }

    fn on_key(&mut self, ctx: &mut ShellContext, key: &Key, state: ElementState, _repeat: bool) {
        if state != ElementState::Pressed {
            return;
        }

        if self.exit_requested.load(Ordering::SeqCst) {
            ctx.exit();
            return;
        }

        let focused_opt = self.focused_id.lock().ok().and_then(|f| f.clone());
        let Some(focused) = focused_opt else {
            return;
        };

        // Phase 1: State Mutation under lock
        let mut event_to_dispatch: Option<(&'static str, String)> = None;
        {
            if let Ok(mut guard) = self.handle.lock() {
                if let Some(comp) = guard.get_mut(&focused) {
                    let current_text = comp.props.entry("text".to_string()).or_default();
                    match key {
                        Key::Named(NamedKey::Backspace) => {
                            current_text.pop();
                            event_to_dispatch = Some(("change", current_text.clone()));
                        }
                        Key::Named(NamedKey::Space) => {
                            current_text.push(' ');
                            event_to_dispatch = Some(("change", current_text.clone()));
                        }
                        Key::Character(s) => {
                            current_text.push_str(s.as_str());
                            event_to_dispatch = Some(("change", current_text.clone()));
                        }
                        Key::Named(NamedKey::Enter) => {
                            let text_val = current_text.clone();
                            event_to_dispatch = Some(("submit", text_val));
                        }
                        _ => {}
                    }
                    if event_to_dispatch.is_some() {
                        guard.revision += 1;
                    }
                }
            }
            // LOCK IS EXPLICITLY DROPPED HERE BEFORE DISPATCH
        }

        // Phase 2: Callback Dispatch (ZERO locks held!)
        if let Some((event_type, payload)) = event_to_dispatch {
            self.dispatch_event(&focused, event_type, &payload);
        }

        // Phase 3: Sync & Redraw
        if let Ok(guard) = self.handle.lock() {
            if guard.revision != ctx.app.revision {
                *ctx.app = guard.clone();
                ctx.request_redraw();
            }
        }

        if self.exit_requested.load(Ordering::SeqCst) {
            ctx.exit();
        }
    }

    fn on_idle(&mut self, ctx: &mut ShellContext) {
        if self.exit_requested.load(Ordering::SeqCst) {
            ctx.exit();
            return;
        }

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
            focused_id: Arc::clone(&handle.focused_id),
            exit_requested: Arc::clone(&handle.exit_requested),
        };

        let config = ShellConfig {
            title: title_str.to_string(),
            width,
            height,
            min_width: 320,
            min_height: 240,
            enable_inspect_server: true,
            inspect_port: None,
            smoke_seconds: if smoke_seconds > 0 {
                Some(smoke_seconds)
            } else {
                None
            },
        };

        match Shell::run(bridge, config) {
            Ok(()) => HameshStatus::Ok,
            Err(_) => HameshStatus::Error,
        }
    });

    result.unwrap_or(HameshStatus::PanicCaught)
}
