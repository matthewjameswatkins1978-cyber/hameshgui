"""
Interactive Python Client for Hamesh GUI.
Demonstrates:
1. Native window creation via C ABI
2. Functional Dropdown Menu (File -> New, Open, Save, Exit) with proper visibility control
3. Text input focusing, typing, deleting, and deadlock-free live echo updates
4. Interactive Button click counter updating labels
5. Supported graceful exit request via File -> Exit
6. Exactly-once event dispatch guarantee
"""

import ctypes
import os
import sys
from ctypes import c_char_p, c_int, c_uint32, c_uint64, c_void_p, c_float, CFUNCTYPE

CURRENT_DIR = os.path.dirname(os.path.abspath(__file__))
DLL_PATH = os.path.abspath(os.path.join(CURRENT_DIR, "../../target/release/hamesh_ffi.dll"))

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
STATUS_PANIC_CAUGHT = 7
STATUS_ERROR = 8
STATUS_BUFFER_TOO_SMALL = 9
ROLE_WINDOW = 0
ROLE_PANEL = 1
ROLE_HEADER = 2
ROLE_BUTTON = 3
ROLE_TEXT_INPUT = 4
ROLE_TEXT = 5
ROLE_STATUS = 8
ROLE_OVERLAY = 9

hamesh.hamesh_abi_version.restype = c_uint32

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

hamesh.hamesh_app_set_visible.argtypes = [c_void_p, c_char_p, c_int]
hamesh.hamesh_app_set_visible.restype = c_int

hamesh.hamesh_app_set_focus.argtypes = [c_void_p, c_char_p]
hamesh.hamesh_app_set_focus.restype = c_int

hamesh.hamesh_app_request_exit.argtypes = [c_void_p]
hamesh.hamesh_app_request_exit.restype = c_int

EVENT_CALLBACK_TYPE = CFUNCTYPE(None, c_void_p, c_char_p, c_char_p, c_char_p)
hamesh.hamesh_app_set_event_callback.argtypes = [c_void_p, c_void_p, c_void_p]
hamesh.hamesh_app_set_event_callback.restype = c_int

hamesh.hamesh_app_poll_event.argtypes = [
    c_void_p,
    c_char_p, ctypes.c_size_t,
    c_char_p, ctypes.c_size_t,
    c_char_p, ctypes.c_size_t
]
hamesh.hamesh_app_poll_event.restype = c_int

hamesh.hamesh_app_poll_events.argtypes = [c_void_p]
hamesh.hamesh_app_poll_events.restype = c_int

hamesh.hamesh_app_get_component_bounds.argtypes = [
    c_void_p, c_char_p, c_uint32, c_uint32,
    ctypes.POINTER(c_float), ctypes.POINTER(c_float), ctypes.POINTER(c_float), ctypes.POINTER(c_float)
]
hamesh.hamesh_app_get_component_bounds.restype = c_int

hamesh.hamesh_app_simulate_click.argtypes = [c_void_p, c_float, c_float, c_uint32, c_uint32]
hamesh.hamesh_app_simulate_click.restype = c_int

hamesh.hamesh_app_simulate_click_component.argtypes = [c_void_p, c_char_p, c_uint32, c_uint32]
hamesh.hamesh_app_simulate_click_component.restype = c_int

hamesh.hamesh_app_simulate_key.argtypes = [c_void_p, c_char_p]
hamesh.hamesh_app_simulate_key.restype = c_int

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
        self.dark_theme = True
        self.text_content = "Type here..."
        self.dispatched_events = []

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

    def set_visible(self, comp_id, visible):
        """Set component visibility directly using Component.visible."""
        status = hamesh.hamesh_app_set_visible(
            self.app_handle,
            comp_id.encode("utf-8"),
            1 if visible else 0
        )
        if status != STATUS_OK:
            raise RuntimeError(f"Failed to set_visible on {comp_id}, status: {status}")

    def request_exit(self):
        """Gracefully request window shutdown through supported Hamesh operation."""
        return hamesh.hamesh_app_request_exit(self.app_handle)

    def set_focus(self, comp_id):
        return hamesh.hamesh_app_set_focus(self.app_handle, comp_id.encode("utf-8"))

    def simulate_click(self, x, y):
        return hamesh.hamesh_app_simulate_click(self.app_handle, float(x), float(y), self.width, self.height)

    def simulate_click_component(self, comp_id):
        return hamesh.hamesh_app_simulate_click_component(self.app_handle, comp_id.encode("utf-8"), self.width, self.height)

    def simulate_key(self, action):
        return hamesh.hamesh_app_simulate_key(self.app_handle, action.encode("utf-8"))

    def get_revision(self):
        return hamesh.hamesh_app_get_revision(self.app_handle)

    def get_component_bounds(self, comp_id):
        """Query component layout bounds (x, y, width, height) relative to the window client area."""
        x = c_float(0.0)
        y = c_float(0.0)
        w = c_float(0.0)
        h = c_float(0.0)
        st = hamesh.hamesh_app_get_component_bounds(
            self.app_handle,
            comp_id.encode("utf-8"),
            self.width,
            self.height,
            ctypes.byref(x),
            ctypes.byref(y),
            ctypes.byref(w),
            ctypes.byref(h),
        )
        if st == STATUS_OK:
            return (x.value, y.value, w.value, h.value)
        return None

    def poll_event(self, comp_id_len=128, event_type_len=64, payload_len=512):
        """Poll a single queued event into bounded buffers without a callback."""
        comp_id_buf = ctypes.create_string_buffer(comp_id_len)
        event_type_buf = ctypes.create_string_buffer(event_type_len)
        payload_buf = ctypes.create_string_buffer(payload_len)
        res = hamesh.hamesh_app_poll_event(
            self.app_handle,
            comp_id_buf, len(comp_id_buf),
            event_type_buf, len(event_type_buf),
            payload_buf, len(payload_buf),
        )
        if res == 1:
            return (
                comp_id_buf.value.decode("utf-8"),
                event_type_buf.value.decode("utf-8"),
                payload_buf.value.decode("utf-8"),
            )
        elif res == -STATUS_BUFFER_TOO_SMALL:
            return -STATUS_BUFFER_TOO_SMALL
        return None

    def poll_events(self):
        """Report pending queued events, or flush to callback if registered."""
        return hamesh.hamesh_app_poll_events(self.app_handle)

    def _on_event(self, user_data, comp_id_ptr, event_type_ptr, payload_ptr):
        comp_id = ctypes.string_at(comp_id_ptr).decode("utf-8")
        event_type = ctypes.string_at(event_type_ptr).decode("utf-8")
        payload = ctypes.string_at(payload_ptr).decode("utf-8") if payload_ptr else ""
        print(f"[Python Dispatch] {comp_id} -> {event_type} (payload: {payload})")
        self.dispatched_events.append((comp_id, event_type, payload))

        # 1. Menu Interactions
        if comp_id == "menu_file":
            self.file_menu_open = not self.file_menu_open
            self.set_visible("file_dropdown", self.file_menu_open)
            self.set_prop("status_label", "text", f"File menu toggled: {'Open' if self.file_menu_open else 'Closed'}")

        elif comp_id == "file_item_new":
            self.file_menu_open = False
            self.set_visible("file_dropdown", False)
            self.text_content = "New document initialized from Python!"
            self.set_prop("py_text_input", "text", self.text_content)
            self.set_prop("echo_label", "text", f"Live Echo: {self.text_content}")
            self.set_prop("status_label", "text", "Executed: File -> New Document")

        elif comp_id in ("menu_theme", "file_item_theme"):
            if comp_id == "file_item_theme":
                self.file_menu_open = False
                self.set_visible("file_dropdown", False)
            self.dark_theme = not self.dark_theme
            theme_name = "dark" if self.dark_theme else "light"
            surface_color = "window" if self.dark_theme else "raised"
            self.set_prop("py_root", "surface", surface_color)
            self.set_prop("status_label", "text", f"Theme switched to: {theme_name}")

        elif comp_id == "file_item_exit":
            print("[Python] Exit requested via File -> Exit.")
            self.request_exit()

        # 2. Button Counter Click
        elif comp_id == "btn_counter":
            self.click_count += 1
            self.set_prop("btn_counter", "text", f"Action Count: {self.click_count}")
            self.set_prop("status_label", "text", f"Counter incremented to {self.click_count} | Rev: {self.get_revision()}")

        # 3. Text Input Change / Typing (Reentrantly calls set_prop without deadlock!)
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

        for item in ["File", "Edit", "Theme", "Help"]:
            self.add_component("py_menu_bar", f"menu_{item.lower()}", ROLE_BUTTON, item, {
                "layout": "row",
                "pad": "4",
                "h": "22",
                "surface": "surface",
                "radius": "4",
                "text": item
            })

        # Dropdown Menu Container (File Dropdown Overlay) - initial visible: false
        self.add_component("py_root", "file_dropdown", ROLE_OVERLAY, "File Menu Dropdown", {
            "layout": "column",
            "w": "160",
            "h": "144",
            "pad": "4",
            "gap": "2",
            "surface": "surface",
            "border": "1",
            "radius": "4",
            "color": "border"
        })
        self.set_visible("file_dropdown", False)

        for item_id, item_label in [
            ("file_item_new", "New Document"),
            ("file_item_theme", "Toggle Theme"),
            ("file_item_open", "Open..."),
            ("file_item_save", "Save"),
            ("file_item_exit", "Exit"),
        ]:
            self.add_component("file_dropdown", item_id, ROLE_BUTTON, item_label, {
                "layout": "row",
                "h": "24",
                "pad": "4",
                "text": item_label
            })

        # 3. Heading Banner
        self.add_component("py_root", "heading_banner", ROLE_HEADER, "Banner", {
            "layout": "column",
            "pad": "12",
            "gap": "4",
            "surface": "surface",
            "radius": "6",
            "border": "1"
        })
        self.add_component("heading_banner", "title_label", ROLE_TEXT, "Title", {
            "h": "24",
            "font_vars": "wght 700",
            "text": "Hamesh Universal GUI — Python Client (Interactive)"
        })
        self.add_component("heading_banner", "sub_label", ROLE_TEXT, "Subtitle", {
            "h": "18",
            "color": "muted",
            "text": "Live two-way event dispatch with deadlock-free synchronous callbacks"
        })

        # 4. Interactive Card (Text Input + Echo + Button)
        self.add_component("py_root", "content_card", ROLE_PANEL, "Content Card", {
            "layout": "column",
            "flex_grow": "1",
            "pad": "16",
            "gap": "12",
            "surface": "surface",
            "radius": "6",
            "border": "1"
        })

        self.add_component("content_card", "input_label", ROLE_TEXT, "Input Field Label", {
            "h": "18",
            "font_vars": "wght 600",
            "text": "Interactive Text Field (Type characters or Backspace):"
        })

        # Editable Text Field
        self.add_component("content_card", "py_text_input", ROLE_TEXT_INPUT, "Python TextInput", {
            "h": "36",
            "pad": "8",
            "surface": "window",
            "border": "2",
            "color": "accent",
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
            hamesh.hamesh_app_set_event_callback(self.app_handle, None, None)
            hamesh.hamesh_app_destroy(self.app_handle)
            self.app_handle = None
            self.callback_ref = None


if __name__ == "__main__":
    smoke = 2 if "--smoke-test" in sys.argv else 0
    app = HameshInteractiveApp(title="Hamesh Python Client (Interactive C ABI)", width=860, height=600)
    app.setup_ui()
    try:
        status = app.run(smoke_seconds=smoke)
        print(f"Python client exited cleanly with status: {status}")
    finally:
        app.destroy()
