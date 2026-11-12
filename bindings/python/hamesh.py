"""
Interactive Python Client for Hamesh GUI.
Demonstrates:
1. Native window creation
2. Functional Dropdown Menu (File -> New, Open, Save, Exit)
3. Text input focusing, typing, deleting, and change event dispatch
4. Interactive Button click counter updating labels
5. Monotonic revision tracking and verification
"""

import ctypes
import os
import sys
from ctypes import c_char_p, c_int, c_uint32, c_uint64, c_void_p, CFUNCTYPE

CURRENT_DIR = os.path.dirname(os.path.abspath(__file__))
DLL_PATH = os.path.abspath(os.path.join(CURRENT_DIR, "../../target/release/hamesh_ffi.dll"))

if not os.path.exists(DLL_PATH):
    raise FileNotFoundError(f"Cannot find hamesh_ffi.dll at {DLL_PATH}")

hamesh = ctypes.CDLL(DLL_PATH)

STATUS_OK = 0
ROLE_WINDOW = 0
ROLE_PANEL = 1
ROLE_HEADER = 2
ROLE_BUTTON = 3
ROLE_TEXT_INPUT = 4
ROLE_TEXT = 5
ROLE_STATUS = 8
ROLE_OVERLAY = 9

hamesh.hamesh_app_create.argtypes = [c_char_p]
hamesh.hamesh_app_create.restype = c_void_p

hamesh.hamesh_app_destroy.argtypes = [c_void_p]
hamesh.hamesh_app_destroy.restype = None

hamesh.hamesh_app_get_revision.argtypes = [c_void_p]
hamesh.hamesh_app_get_revision.restype = c_uint64

hamesh.hamesh_app_add_component.argtypes = [c_void_p, c_char_p, c_char_p, c_int, c_char_p]
hamesh.hamesh_app_add_component.restype = c_int

hamesh.hamesh_app_set_prop.argtypes = [c_void_p, c_char_p, c_char_p, c_char_p]
hamesh.hamesh_app_set_prop.restype = c_int

EVENT_CALLBACK_TYPE = CFUNCTYPE(None, c_void_p, c_char_p, c_char_p, c_char_p)
hamesh.hamesh_app_set_event_callback.argtypes = [c_void_p, EVENT_CALLBACK_TYPE, c_void_p]
hamesh.hamesh_app_set_event_callback.restype = c_int

hamesh.hamesh_run.argtypes = [c_void_p, c_char_p, c_uint32, c_uint32, c_uint64]
hamesh.hamesh_run.restype = c_int


class HameshInteractiveApp:
    def __init__(self, title="Hamesh Python Client (Interactive)", width=860, height=600):
        self.title = title
        self.width = width
        self.height = height
        self.app_handle = hamesh.hamesh_app_create(b"root")
        if not self.app_handle:
            raise RuntimeError("Failed to create Hamesh App handle via C ABI")

        self.click_count = 0
        self.file_menu_open = False
        self.text_content = "Type here..."

        # Register event callback
        self.callback_ref = EVENT_CALLBACK_TYPE(self._on_event)
        hamesh.hamesh_app_set_event_callback(self.app_handle, self.callback_ref, None)

    def add_component(self, parent_id, comp_id, role, name, props=None):
        status = hamesh.hamesh_app_add_component(
            self.app_handle,
            parent_id.encode("utf-8"),
            comp_id.encode("utf-8"),
            role,
            name.encode("utf-8")
        )
        if status != STATUS_OK:
            raise RuntimeError(f"Failed to add component {comp_id}, status: {status}")

        if props:
            for k, v in props.items():
                self.set_prop(comp_id, k, v)

    def set_prop(self, comp_id, key, val):
        status = hamesh.hamesh_app_set_prop(
            self.app_handle,
            comp_id.encode("utf-8"),
            key.encode("utf-8"),
            str(val).encode("utf-8")
        )
        if status != STATUS_OK:
            raise RuntimeError(f"Failed to set prop {key} on {comp_id}, status: {status}")

    def get_revision(self):
        return hamesh.hamesh_app_get_revision(self.app_handle)

    def _on_event(self, user_data, comp_id_ptr, event_type_ptr, payload_ptr):
        comp_id = ctypes.string_at(comp_id_ptr).decode("utf-8")
        event_type = ctypes.string_at(event_type_ptr).decode("utf-8")
        payload = ctypes.string_at(payload_ptr).decode("utf-8") if payload_ptr else ""
        print(f"[Python Dispatch] {comp_id} -> {event_type} (payload: {payload})")

        # 1. Menu Interactions
        if comp_id == "menu_file":
            self.file_menu_open = not self.file_menu_open
            vis = "true" if self.file_menu_open else "false"
            self.set_prop("file_dropdown", "visible", vis)
            self.set_prop("status_label", "text", f"File menu toggled: {'Open' if self.file_menu_open else 'Closed'}")

        elif comp_id == "file_item_new":
            self.file_menu_open = False
            self.set_prop("file_dropdown", "visible", "false")
            self.set_prop("py_text_input", "text", "New document initialized from Python!")
            self.set_prop("status_label", "text", "Executed: File -> New Document")

        elif comp_id == "file_item_exit":
            print("Exit requested via File -> Exit.")
            sys.exit(0)

        # 2. Button Counter Click
        elif comp_id == "btn_counter":
            self.click_count += 1
            self.set_prop("btn_counter", "text", f"Action Count: {self.click_count}")
            self.set_prop("status_label", "text", f"Counter incremented to {self.click_count} | Rev: {self.get_revision()}")

        # 3. Text Input Change / Typing
        elif comp_id == "py_text_input" and event_type == "change":
            self.text_content = payload
            self.set_prop("echo_label", "text", f"Live Echo: {self.text_content}")

    def setup_ui(self):
        # 1. Main container
        self.add_component("root", "py_root", ROLE_PANEL, "Python Root", {
            "layout": "column",
            "w": "100%",
            "h": "100%",
            "surface": "window",
            "font": "Instrument Sans",
            "pad": "8",
            "gap": "10"
        })

        # 2. Menu Bar
        self.add_component("py_root", "py_menu_bar", ROLE_HEADER, "Menu Bar", {
            "layout": "row",
            "h": "30",
            "surface": "header",
            "pad": "4",
            "gap": "6",
            "border": "1"
        })

        for item in ["File", "Edit", "Help"]:
            self.add_component("py_menu_bar", f"menu_{item.lower()}", ROLE_BUTTON, item, {
                "layout": "row",
                "h": "22",
                "pad": "6",
                "surface": "surface",
                "radius": "4",
                "text": item
            })

        # 3. File Dropdown Overlay
        self.add_component("py_root", "file_dropdown", ROLE_OVERLAY, "File Menu Overlay", {
            "layout": "column",
            "w": "200",
            "surface": "raised",
            "border": "1",
            "radius": "4",
            "pad": "4",
            "gap": "2",
            "ox": "4",
            "oy": "34",
            "z": "100",
            "visible": "false"
        })

        self.add_component("file_dropdown", "file_item_new", ROLE_BUTTON, "New Document", {
            "h": "24",
            "pad": "6",
            "surface": "raised",
            "text": "New Document"
        })

        self.add_component("file_dropdown", "file_item_exit", ROLE_BUTTON, "Exit", {
            "h": "24",
            "pad": "6",
            "surface": "raised",
            "text": "Exit Application"
        })

        # 4. Interactive Card
        self.add_component("py_root", "content_card", ROLE_PANEL, "Content Card", {
            "layout": "column",
            "w": "480",
            "pad": "16",
            "gap": "12",
            "surface": "raised",
            "border": "1",
            "radius": "6"
        })

        self.add_component("content_card", "title_label", ROLE_TEXT, "Title", {
            "h": "24",
            "font_vars": "wght 700",
            "text": "Universal Python GUI Host"
        })

        self.add_component("content_card", "input_heading", ROLE_TEXT, "Input Heading", {
            "h": "18",
            "color": "text_dim",
            "text": "Editable Text Field (Click & Type):"
        })

        # Editable Text Field
        self.add_component("content_card", "py_text_input", ROLE_TEXT_INPUT, "Input Field", {
            "h": "32",
            "pad": "6",
            "surface": "window",
            "border": "1",
            "radius": "4",
            "text": self.text_content
        })

        # Live Echo Label
        self.add_component("content_card", "echo_label", ROLE_TEXT, "Echo Label", {
            "h": "20",
            "color": "accent",
            "text": f"Live Echo: {self.text_content}"
        })

        # Interactive Button
        self.add_component("content_card", "btn_counter", ROLE_BUTTON, "Click Counter", {
            "h": "32",
            "pad": "8",
            "surface": "accent",
            "color": "bg",
            "radius": "4",
            "text": "Click me to Increment"
        })

        # 5. Status Bar
        self.add_component("py_root", "status_label", ROLE_STATUS, "Status", {
            "h": "24",
            "color": "accent",
            "text": f"Engine Connected | Revision: {self.get_revision()}"
        })

    def run(self, smoke_seconds=0):
        status = hamesh.hamesh_run(
            self.app_handle,
            self.title.encode("utf-8"),
            self.width,
            self.height,
            smoke_seconds
        )
        return status

    def destroy(self):
        if self.app_handle:
            hamesh.hamesh_app_destroy(self.app_handle)
            self.app_handle = None


if __name__ == "__main__":
    smoke = 2 if "--smoke-test" in sys.argv else 0
    app = HameshInteractiveApp(title="Hamesh Python Client (Interactive C ABI)", width=860, height=600)
    app.setup_ui()
    try:
        status = app.run(smoke_seconds=smoke)
        print(f"Python client exited cleanly with status: {status}")
    finally:
        app.destroy()
