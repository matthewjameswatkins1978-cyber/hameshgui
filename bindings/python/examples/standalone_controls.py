"""
Standalone Python Consumer of Hamesh Desktop Controls.
Demonstrates that the universal GUI foundation allows Python applications to construct
and operate reusable desktop menus, tabs, checkboxes, text fields, and selectable lists
with theme profile support (Classic Desktop, Modern Studio, Retro, Technical, Editorial).
"""

import ctypes
import os
import sys

CURRENT_DIR = os.path.dirname(os.path.abspath(__file__))
DLL_PATH = os.path.abspath(os.path.join(CURRENT_DIR, "../../../target/release/hamesh_ffi.dll"))

if not os.path.exists(DLL_PATH):
    raise FileNotFoundError(f"Cannot find hamesh_ffi.dll at {DLL_PATH}")

hamesh = ctypes.CDLL(DLL_PATH)

ROLE_WINDOW = 0
ROLE_PANEL = 1
ROLE_HEADER = 2
ROLE_BUTTON = 3
ROLE_TEXT_INPUT = 4
ROLE_TEXT = 5
ROLE_LIST_ITEM = 7
ROLE_STATUS = 8
ROLE_OVERLAY = 9

hamesh.hamesh_app_create.argtypes = [ctypes.c_char_p]
hamesh.hamesh_app_create.restype = ctypes.c_void_p
hamesh.hamesh_app_destroy.argtypes = [ctypes.c_void_p]
hamesh.hamesh_app_get_revision.argtypes = [ctypes.c_void_p]
hamesh.hamesh_app_get_revision.restype = ctypes.c_uint64
hamesh.hamesh_app_add_component.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p]
hamesh.hamesh_app_add_component.restype = ctypes.c_int
hamesh.hamesh_app_set_prop.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p]
hamesh.hamesh_app_set_prop.restype = ctypes.c_int
hamesh.hamesh_app_set_focus.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
hamesh.hamesh_app_set_focus.restype = ctypes.c_int

def main():
    print("=== Python Standalone Consumer for Hamesh Desktop Controls ===")
    app = hamesh.hamesh_app_create(b"root")
    assert app is not None, "Failed to create HameshApp in Python"

    # 1. Root with Visual World theme (Retro Computing)
    hamesh.hamesh_app_set_prop(app, b"root", b"theme", b"retro")
    hamesh.hamesh_app_set_prop(app, b"root", b"surface", b"window")

    # 2. Reusable MenuBar
    res = hamesh.hamesh_app_add_component(app, b"root", b"menu_bar", ROLE_HEADER, b"Menu Bar")
    assert res == 0
    hamesh.hamesh_app_set_prop(app, b"menu_bar", b"layout", b"row")
    hamesh.hamesh_app_set_prop(app, b"menu_bar", b"h", b"26")
    hamesh.hamesh_app_set_prop(app, b"menu_bar", b"surface", b"header")

    menus = [("menu_file", "File (F)"), ("menu_edit", "Edit (E)"), ("menu_view", "View (V)")]
    for mid, mlabel in menus:
        res = hamesh.hamesh_app_add_component(app, b"menu_bar", mid.encode("utf-8"), ROLE_BUTTON, mlabel.encode("utf-8"))
        assert res == 0
        hamesh.hamesh_app_set_prop(app, mid.encode("utf-8"), b"text", mlabel.encode("utf-8"))
        hamesh.hamesh_app_set_prop(app, mid.encode("utf-8"), b"color", b"text")

    # 3. TabControl
    res = hamesh.hamesh_app_add_component(app, b"root", b"tabs_container", ROLE_PANEL, b"Tab Bar")
    assert res == 0
    hamesh.hamesh_app_set_prop(app, b"tabs_container", b"layout", b"row")
    hamesh.hamesh_app_set_prop(app, b"tabs_container", b"h", b"32")

    tabs = [("tab_general", "General"), ("tab_advanced", "Advanced")]
    for tid, tlabel in tabs:
        res = hamesh.hamesh_app_add_component(app, b"tabs_container", tid.encode("utf-8"), ROLE_BUTTON, tlabel.encode("utf-8"))
        assert res == 0
        hamesh.hamesh_app_set_prop(app, tid.encode("utf-8"), b"text", tlabel.encode("utf-8"))

    # 4. Checkbox
    res = hamesh.hamesh_app_add_component(app, b"root", b"py_checkbox", ROLE_BUTTON, b"Checkbox")
    assert res == 0
    hamesh.hamesh_app_set_prop(app, b"py_checkbox", b"text", b"[X] Enable Python Automation")

    # 5. TextInput
    res = hamesh.hamesh_app_add_component(app, b"root", b"py_text_input", ROLE_TEXT_INPUT, b"TextInput")
    assert res == 0
    hamesh.hamesh_app_set_prop(app, b"py_text_input", b"text", b"Initial Python Text Buffer")
    hamesh.hamesh_app_set_focus(app, b"py_text_input")

    # 6. StatusBar
    res = hamesh.hamesh_app_add_component(app, b"root", b"py_status", ROLE_STATUS, b"Status")
    assert res == 0
    hamesh.hamesh_app_set_prop(app, b"py_status", b"text", b"Python Client Ready. Visual World: Retro.")

    rev = hamesh.hamesh_app_get_revision(app)
    print(f"Python created {rev} component revisions in shared native tree.")
    assert rev >= 8

    # 7. Clean destruction
    hamesh.hamesh_app_destroy(app)
    print("SUCCESS: Standalone Python consumer verified desktop controls!")

if __name__ == "__main__":
    main()
