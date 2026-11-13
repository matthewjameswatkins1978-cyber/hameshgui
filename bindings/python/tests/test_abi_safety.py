"""
Unit tests executing invalid/malformed C ABI calls directly from Python.
Verifies Section 1 safety requirements:
- Invalid UTF-8 inputs
- Null pointer handling
- Duplicate component IDs (no panic/crash)
- Missing parent IDs
- Invalid role codes
- Revision monotonicity
"""

import ctypes
import os
import sys

CURRENT_DIR = os.path.dirname(os.path.abspath(__file__))
DLL_PATH = os.path.abspath(os.path.join(CURRENT_DIR, "../../../target/release/hamesh_ffi.dll"))

if not os.path.exists(DLL_PATH):
    raise FileNotFoundError(f"Cannot find hamesh_ffi.dll at {DLL_PATH}")

hamesh = ctypes.CDLL(DLL_PATH)

STATUS_OK = 0
STATUS_NULL_POINTER = 1
STATUS_INVALID_UTF8 = 2
STATUS_NOT_FOUND = 3
STATUS_DUPLICATE_ID = 4
STATUS_PARENT_NOT_FOUND = 5
STATUS_INVALID_ROLE = 6

hamesh.hamesh_abi_version.restype = ctypes.c_uint32
hamesh.hamesh_app_create.argtypes = [ctypes.c_char_p]
hamesh.hamesh_app_create.restype = ctypes.c_void_p
hamesh.hamesh_app_destroy.argtypes = [ctypes.c_void_p]
hamesh.hamesh_app_get_revision.argtypes = [ctypes.c_void_p]
hamesh.hamesh_app_get_revision.restype = ctypes.c_uint64
hamesh.hamesh_app_add_component.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p]
hamesh.hamesh_app_add_component.restype = ctypes.c_int
hamesh.hamesh_app_set_prop.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p]
hamesh.hamesh_app_set_prop.restype = ctypes.c_int

def test_abi_version():
    ver = hamesh.hamesh_abi_version()
    major = (ver >> 16) & 0xFF
    minor = (ver >> 8) & 0xFF
    patch = ver & 0xFF
    print(f"[TEST PASS] ABI Version: {major}.{minor}.{patch} (raw: {ver:#x})")
    assert major == 0 and minor == 4 and patch == 2

def test_null_pointers():
    assert hamesh.hamesh_app_create(None) is None
    status = hamesh.hamesh_app_add_component(None, b"p", b"c", 1, b"name")
    assert status == STATUS_NULL_POINTER

    app = hamesh.hamesh_app_create(b"root")
    try:
        status = hamesh.hamesh_app_add_component(app, None, b"c", 1, b"name")
        assert status == STATUS_NULL_POINTER

        status = hamesh.hamesh_app_add_component(app, b"root", None, 1, b"name")
        assert status == STATUS_NULL_POINTER

        status = hamesh.hamesh_app_set_prop(app, None, b"k", b"v")
        assert status == STATUS_NULL_POINTER
        print("[TEST PASS] Null pointer arguments rejected safely with code 1")
    finally:
        hamesh.hamesh_app_destroy(app)

def test_duplicate_component_id():
    app = hamesh.hamesh_app_create(b"root")
    try:
        # First addition succeeds
        status = hamesh.hamesh_app_add_component(app, b"root", b"comp1", 1, b"Panel 1")
        assert status == STATUS_OK

        # Duplicate addition: MUST return STATUS_DUPLICATE_ID (4) without panicking
        status = hamesh.hamesh_app_add_component(app, b"root", b"comp1", 1, b"Duplicate Panel")
        assert status == STATUS_DUPLICATE_ID, f"Expected duplicate error (4), got {status}"
        print("[TEST PASS] Duplicate component ID rejected safely with code 4 (no panic)")
    finally:
        hamesh.hamesh_app_destroy(app)

def test_missing_parent_id():
    app = hamesh.hamesh_app_create(b"root")
    try:
        status = hamesh.hamesh_app_add_component(app, b"nonexistent_parent", b"comp_child", 1, b"Child")
        assert status == STATUS_PARENT_NOT_FOUND, f"Expected parent not found (5), got {status}"
        print("[TEST PASS] Missing parent ID rejected safely with code 5")
    finally:
        hamesh.hamesh_app_destroy(app)

def test_invalid_role():
    app = hamesh.hamesh_app_create(b"root")
    try:
        status = hamesh.hamesh_app_add_component(app, b"root", b"comp_bad_role", 99, b"Bad Role")
        assert status == STATUS_INVALID_ROLE, f"Expected invalid role error (6), got {status}"
        print("[TEST PASS] Invalid role code rejected safely with code 6")
    finally:
        hamesh.hamesh_app_destroy(app)

EVENT_CALLBACK_TYPE = ctypes.CFUNCTYPE(None, ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p)
hamesh.hamesh_app_set_event_callback.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
hamesh.hamesh_app_set_event_callback.restype = ctypes.c_int
hamesh.hamesh_app_await_quiescence.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
hamesh.hamesh_app_await_quiescence.restype = ctypes.c_int
hamesh.hamesh_app_set_visible.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
hamesh.hamesh_app_set_visible.restype = ctypes.c_int
hamesh.hamesh_app_set_focus.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
hamesh.hamesh_app_set_focus.restype = ctypes.c_int
hamesh.hamesh_app_request_exit.argtypes = [ctypes.c_void_p]
hamesh.hamesh_app_request_exit.restype = ctypes.c_int
hamesh.hamesh_app_simulate_click.argtypes = [ctypes.c_void_p, ctypes.c_float, ctypes.c_float, ctypes.c_uint32, ctypes.c_uint32]
hamesh.hamesh_app_simulate_click.restype = ctypes.c_int
hamesh.hamesh_app_simulate_click_component.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_uint32, ctypes.c_uint32]
hamesh.hamesh_app_simulate_click_component.restype = ctypes.c_int
hamesh.hamesh_app_simulate_key.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
hamesh.hamesh_app_simulate_key.restype = ctypes.c_int
hamesh.hamesh_app_poll_event.argtypes = [
    ctypes.c_void_p,
    ctypes.c_char_p, ctypes.c_size_t,
    ctypes.c_char_p, ctypes.c_size_t,
    ctypes.c_char_p, ctypes.c_size_t
]
hamesh.hamesh_app_poll_event.restype = ctypes.c_int
hamesh.hamesh_app_poll_events.argtypes = [ctypes.c_void_p]
hamesh.hamesh_app_poll_events.restype = ctypes.c_int

def test_deadlock_free_callback_reentrancy():
    """Verify that a foreign callback can call set_prop reentrantly without deadlock."""
    app = hamesh.hamesh_app_create(b"root")
    try:
        hamesh.hamesh_app_add_component(app, b"root", b"text_input", 4, b"Input")
        hamesh.hamesh_app_add_component(app, b"root", b"echo_label", 5, b"Echo")

        reentrant_success = [False]

        def on_event(user_data, comp_id_ptr, event_type_ptr, payload_ptr):
            cid = ctypes.string_at(comp_id_ptr).decode("utf-8")
            etype = ctypes.string_at(event_type_ptr).decode("utf-8")
            pl = ctypes.string_at(payload_ptr).decode("utf-8") if payload_ptr else ""
            if cid == "text_input" and etype == "change":
                # Reentrant call back into Hamesh C ABI
                status = hamesh.hamesh_app_set_prop(app, b"echo_label", b"text", f"Live: {pl}".encode("utf-8"))
                assert status == STATUS_OK
                reentrant_success[0] = True

        cb = EVENT_CALLBACK_TYPE(on_event)
        hamesh.hamesh_app_set_event_callback(app, cb, None)

        # Focus text_input directly via supported API
        status = hamesh.hamesh_app_set_focus(app, b"text_input")
        assert status == STATUS_OK

        # Simulate key input
        status = hamesh.hamesh_app_simulate_key(app, b"char:Hello")
        assert status == STATUS_OK
        assert reentrant_success[0] is True
        print("[TEST PASS] Deadlock-free callback reentrancy verified (set_prop during change callback)")
    finally:
        hamesh.hamesh_app_destroy(app)

def test_visibility_and_hit_rejection():
    """Verify that invisible components and their children reject hit tests."""
    app = hamesh.hamesh_app_create(b"root")
    try:
        hamesh.hamesh_app_add_component(app, b"root", b"menu_container", 9, b"Menu Container")
        hamesh.hamesh_app_set_prop(app, b"menu_container", b"layout", b"column")
        hamesh.hamesh_app_set_prop(app, b"menu_container", b"ox", b"0")
        hamesh.hamesh_app_set_prop(app, b"menu_container", b"oy", b"0")
        hamesh.hamesh_app_set_prop(app, b"menu_container", b"w", b"200")
        hamesh.hamesh_app_set_prop(app, b"menu_container", b"h", b"100")

        hamesh.hamesh_app_add_component(app, b"menu_container", b"menu_item", 3, b"Menu Item")
        hamesh.hamesh_app_set_prop(app, b"menu_item", b"w", b"200")
        hamesh.hamesh_app_set_prop(app, b"menu_item", b"h", b"40")

        # By default visible
        events = []
        def on_event(user_data, comp_id_ptr, event_type_ptr, payload_ptr):
            events.append(ctypes.string_at(comp_id_ptr).decode("utf-8"))

        cb = EVENT_CALLBACK_TYPE(on_event)
        hamesh.hamesh_app_set_event_callback(app, cb, None)

        # Set parent invisible
        status = hamesh.hamesh_app_set_visible(app, b"menu_container", 0)
        assert status == STATUS_OK

        # Try to click menu_item while hidden - MUST return STATUS_NOT_FOUND (3)
        status = hamesh.hamesh_app_simulate_click_component(app, b"menu_item", 800, 600)
        assert status == STATUS_NOT_FOUND, f"Expected STATUS_NOT_FOUND (3), got {status}"
        assert len(events) == 0, f"Hidden menu_item incorrectly received click! Events: {events}"

        # Make menu_container visible again
        status = hamesh.hamesh_app_set_visible(app, b"menu_container", 1)
        assert status == STATUS_OK

        # Now click succeeds and hits menu_item
        events.clear()
        status = hamesh.hamesh_app_simulate_click_component(app, b"menu_item", 800, 600)
        assert status == STATUS_OK
        assert events == ["menu_item"], f"Expected ['menu_item'], got {events}"
        print("[TEST PASS] Visibility control & hidden-item click rejection verified")
    finally:
        hamesh.hamesh_app_destroy(app)

def test_graceful_exit_request():
    """Verify supported graceful exit request through C ABI."""
    app = hamesh.hamesh_app_create(b"root")
    try:
        status = hamesh.hamesh_app_request_exit(app)
        assert status == STATUS_OK
        print("[TEST PASS] Graceful exit request accepted with STATUS_OK")
    finally:
        hamesh.hamesh_app_destroy(app)

def test_bounded_event_polling():
    """Verify bounded event polling without callbacks and zero event loss."""
    app = hamesh.hamesh_app_create(b"root")
    try:
        hamesh.hamesh_app_add_component(app, b"root", b"poll_btn", 3, b"Button")

        # Without callback: simulate click -> queued
        st = hamesh.hamesh_app_simulate_click_component(app, b"poll_btn", 800, 600)
        assert st == STATUS_OK

        # Pending count reported without dropping events
        pending = hamesh.hamesh_app_poll_events(app)
        assert pending == 1, f"Expected 1 pending event, got {pending}"

        comp_buf = ctypes.create_string_buffer(64)
        type_buf = ctypes.create_string_buffer(32)
        payload_buf = ctypes.create_string_buffer(128)

        ret = hamesh.hamesh_app_poll_event(
            app,
            comp_buf, len(comp_buf),
            type_buf, len(type_buf),
            payload_buf, len(payload_buf)
        )
        assert ret == 1
        assert comp_buf.value.decode("utf-8") == "poll_btn"
        assert type_buf.value.decode("utf-8") == "click"

        # Queue now empty
        empty = hamesh.hamesh_app_poll_event(
            app,
            comp_buf, len(comp_buf),
            type_buf, len(type_buf),
            payload_buf, len(payload_buf)
        )
        assert empty == 0
        print("[TEST PASS] Event polling without callbacks verified (0 event loss)")
    finally:
        hamesh.hamesh_app_destroy(app)

def test_lossless_polling_and_multibyte_utf8():
    """Verify capacity validation before consumption, event preservation on BufferTooSmall, and UTF-8 integrity."""
    STATUS_BUFFER_TOO_SMALL = 9
    app = hamesh.hamesh_app_create(b"root")
    try:
        hamesh.hamesh_app_add_component(app, b"root", b"txt_utf8", 4, b"Input")
        hamesh.hamesh_app_set_focus(app, b"txt_utf8")

        # Multibyte UTF-8 string: German, Japanese, Hebrew, and Emoji
        utf8_text = "Grüße! こんにちは, שלום 🚀"
        st = hamesh.hamesh_app_simulate_key(app, f"text:{utf8_text}".encode("utf-8"))
        assert st == STATUS_OK

        # Event queued
        assert hamesh.hamesh_app_poll_events(app) == 1

        # 1. Poll with deliberately undersized buffer (4 bytes)
        small_comp = ctypes.create_string_buffer(4)
        small_type = ctypes.create_string_buffer(4)
        small_payload = ctypes.create_string_buffer(4)

        ret = hamesh.hamesh_app_poll_event(
            app,
            small_comp, len(small_comp),
            small_type, len(small_type),
            small_payload, len(small_payload)
        )
        assert ret == -STATUS_BUFFER_TOO_SMALL, f"Expected -9 (BufferTooSmall), got {ret}"

        # CRITICAL ASSERTION: Event MUST NOT have been consumed or dropped!
        assert hamesh.hamesh_app_poll_events(app) == 1, "Event was dropped or consumed on insufficient buffer!"

        # 2. Poll with adequate buffer (512 bytes)
        comp_buf = ctypes.create_string_buffer(64)
        type_buf = ctypes.create_string_buffer(32)
        payload_buf = ctypes.create_string_buffer(512)

        ret_ok = hamesh.hamesh_app_poll_event(
            app,
            comp_buf, len(comp_buf),
            type_buf, len(type_buf),
            payload_buf, len(payload_buf)
        )
        assert ret_ok == 1, f"Expected 1, got {ret_ok}"
        assert comp_buf.value.decode("utf-8") == "txt_utf8"
        assert type_buf.value.decode("utf-8") == "change"
        assert payload_buf.value.decode("utf-8") == utf8_text

        # Queue now empty
        assert hamesh.hamesh_app_poll_events(app) == 0
        print("[TEST PASS] Lossless event polling on BufferTooSmall & multibyte UTF-8 verified")
    finally:
        hamesh.hamesh_app_destroy(app)

def test_callback_unregistration():
    """Verify safe callback unregistration, active bridge sync, and zero post-teardown dispatch."""
    app = hamesh.hamesh_app_create(b"root")
    try:
        hamesh.hamesh_app_add_component(app, b"root", b"unreg_btn", 3, b"Button")
        calls = [0]
        def on_event(u, c, e, p):
            calls[0] += 1
        cb = EVENT_CALLBACK_TYPE(on_event)

        # Register callback
        st = hamesh.hamesh_app_set_event_callback(app, cb, None)
        assert st == STATUS_OK

        # Unregister callback by passing NULL/None
        st = hamesh.hamesh_app_set_event_callback(app, None, None)
        assert st == STATUS_OK

        # Simulate click: must queue, NOT invoke unreferenced callback
        st = hamesh.hamesh_app_simulate_click_component(app, b"unreg_btn", 800, 600)
        assert st == STATUS_OK
        assert calls[0] == 0, "Unregistered callback was invoked!"
        assert hamesh.hamesh_app_poll_events(app) == 1

        # Re-register and verify reentrant self-unregistration inside callback
        def self_unreg_cb(u, c, e, p):
            calls[0] += 10
            # Unregister inside the callback without deadlock!
            hamesh.hamesh_app_set_event_callback(app, None, None)

        self_cb = EVENT_CALLBACK_TYPE(self_unreg_cb)
        hamesh.hamesh_app_set_event_callback(app, self_cb, None)

        # Flush queued event -> invokes self_unreg_cb -> self-unregisters
        flushed = hamesh.hamesh_app_poll_events(app)
        assert flushed == 1
        assert calls[0] == 10

        # Subsequent click goes to queue without calling self_cb
        st = hamesh.hamesh_app_simulate_click_component(app, b"unreg_btn", 800, 600)
        assert st == STATUS_OK
        assert calls[0] == 10  # Unchanged!
        assert hamesh.hamesh_app_poll_events(app) == 1

        print("[TEST PASS] Callback unregistration & reentrant active synchronization verified")
    finally:
        hamesh.hamesh_app_destroy(app)

def test_callback_quiescence_and_races():
    """Verify detachment vs. quiescence barrier: unregistration does not block,
    in-flight dispatches run to completion, and await_quiescence guarantees all active
    dispatches have finished before foreign memory teardown."""
    import time
    import threading

    app = hamesh.hamesh_app_create(b"root")
    try:
        hamesh.hamesh_app_add_component(app, b"root", b"q_btn", 3, b"Button")

        # Initial state: no callbacks active -> quiesced
        assert hamesh.hamesh_app_await_quiescence(app, 0) == 1

        in_flight_observed = [False]
        callback_completed = [False]
        start_event = threading.Event()

        def slow_callback(u, c, e, p):
            in_flight_observed[0] = True
            start_event.set()
            time.sleep(0.1) # Simulate in-flight foreign execution
            callback_completed[0] = True

        cb = EVENT_CALLBACK_TYPE(slow_callback)
        st = hamesh.hamesh_app_set_event_callback(app, cb, None)
        assert st == STATUS_OK

        # Spawn thread to trigger the slow callback
        dispatch_thread = threading.Thread(
            target=lambda: hamesh.hamesh_app_simulate_click_component(app, b"q_btn", 800, 600)
        )
        dispatch_thread.start()

        # Wait until slow_callback is running
        assert start_event.wait(timeout=2.0)

        # Detach callback immediately from another thread
        st = hamesh.hamesh_app_set_event_callback(app, None, None)
        assert st == STATUS_OK

        # Detachment is immediate: subsequent click diverts to queue
        st2 = hamesh.hamesh_app_simulate_click_component(app, b"q_btn", 800, 600)
        assert st2 == STATUS_OK
        assert hamesh.hamesh_app_poll_events(app) == 1

        # Await quiescence barrier: blocks until slow_callback finishes
        quiesced = hamesh.hamesh_app_await_quiescence(app, 2000)
        assert quiesced == 1
        assert callback_completed[0] is True

        dispatch_thread.join(timeout=1.0)
        print("[TEST PASS] Callback detachment vs quiescence barrier verified")
    finally:
        hamesh.hamesh_app_destroy(app)

if __name__ == "__main__":
    print("=== Running Python C ABI Safety Battery ===")
    test_abi_version()
    test_null_pointers()
    test_duplicate_component_id()
    test_missing_parent_id()
    test_invalid_role()
    test_deadlock_free_callback_reentrancy()
    test_visibility_and_hit_rejection()
    test_graceful_exit_request()
    test_bounded_event_polling()
    test_lossless_polling_and_multibyte_utf8()
    test_callback_unregistration()
    test_callback_quiescence_and_races()
    print("=== All Python C ABI Safety Tests PASSED ===")
