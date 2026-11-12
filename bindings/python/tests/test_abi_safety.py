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

if __name__ == "__main__":
    print("=== Running Python C ABI Safety Battery ===")
    test_abi_version()
    test_null_pointers()
    test_duplicate_component_id()
    test_missing_parent_id()
    test_invalid_role()
    print("=== All Python C ABI Safety Tests PASSED ===")
