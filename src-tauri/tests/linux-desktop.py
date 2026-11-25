"""Real GTK/WebKit UI through AT-SPI, in a caller-owned isolated X11 session.

Run: dbus-run-session -- xvfb-run -a -s '-screen 0 1280x900x24' \
  python3 tests/linux-desktop.py /absolute/compass /absent/output
Requires python3-pyatspi, GTK 3, X11 and an extracted Linux package executable.
"""
import ctypes as C
import ctypes.util
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import struct
import sys
import time
import zlib

import pyatspi

source, output = map(lambda value: Path(value).resolve(), sys.argv[1:])
output.mkdir()
shutil.copy2(source, output / "compass")
(output / "portable.txt").touch()
data = output / "data"
data.mkdir()

# Capture the X root, including WebKit's child surfaces. Capturing only the
# parent X window can yield an empty image even when the web content is visible.
x11 = C.CDLL(ctypes.util.find_library("X11"))
x11.XOpenDisplay.restype = C.c_void_p
x11.XDefaultRootWindow.argtypes = [C.c_void_p]
x11.XDefaultRootWindow.restype = C.c_ulong
x11.XQueryTree.argtypes = [C.c_void_p, C.c_ulong, C.POINTER(C.c_ulong),
                          C.POINTER(C.c_ulong), C.POINTER(C.POINTER(C.c_ulong)),
                          C.POINTER(C.c_uint)]
x11.XFetchName.argtypes = [C.c_void_p, C.c_ulong, C.POINTER(C.c_char_p)]
x11.XFree.argtypes = [C.c_void_p]
x11.XResizeWindow.argtypes = [C.c_void_p, C.c_ulong, C.c_uint, C.c_uint]
x11.XSync.argtypes = [C.c_void_p, C.c_int]
x11.XCloseDisplay.argtypes = [C.c_void_p]
x11.XTranslateCoordinates.argtypes = [C.c_void_p, C.c_ulong, C.c_ulong, C.c_int, C.c_int,
                                      C.POINTER(C.c_int), C.POINTER(C.c_int), C.POINTER(C.c_ulong)]
x11.XGetImage.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_ulong, C.c_int]
x11.XGetImage.restype = C.c_void_p
x11.XGetPixel.argtypes = [C.c_void_p, C.c_int, C.c_int]
x11.XGetPixel.restype = C.c_ulong
x11.XDestroyImage.argtypes = [C.c_void_p]
x11.XSetInputFocus.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_ulong]
x11.XKeysymToKeycode.argtypes = [C.c_void_p, C.c_ulong]
x11.XKeysymToKeycode.restype = C.c_uint
xtest = C.CDLL(ctypes.util.find_library("Xtst"))
xtest.XTestFakeKeyEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
display = x11.XOpenDisplay(None)
assert display, "An isolated X11 display is required"


def walk(node):
    yield node
    for child in node:
        if child is not None:
            yield from walk(child)


def wait(check, timeout=30):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = check()
        if value:
            return value
        time.sleep(0.1)
    raise AssertionError("UI condition timed out")


def find(role, name):
    matches = [node for node in walk(application)
               if node.getRoleName() == role and node.name == name]
    assert len(matches) == 1, (role, name, len(matches))
    return matches[0]


def click(name):
    assert find("button", name).queryAction().doAction(0), name


def find_window(parent):
    root, owner = C.c_ulong(), C.c_ulong()
    children, count = C.POINTER(C.c_ulong)(), C.c_uint()
    assert x11.XQueryTree(display, parent, C.byref(root), C.byref(owner),
                          C.byref(children), C.byref(count))
    windows = list(children[:count.value])
    if children:
        x11.XFree(children)
    for window in windows:
        name = C.c_char_p()
        x11.XFetchName(display, window, C.byref(name))
        title = name.value
        if name:
            x11.XFree(name)
        if title == b"Mythic Compass":
            return window
        nested = find_window(window)
        if nested:
            return nested


def screenshot(name):
    bounds = frame.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
    root = x11.XDefaultRootWindow(display)
    px, py, child = C.c_int(), C.c_int(), C.c_ulong()
    assert x11.XTranslateCoordinates(display, window, root, 0, 0,
                                     C.byref(px), C.byref(py), C.byref(child))
    image = x11.XGetImage(display, root, px.value, py.value, bounds.width, bounds.height, C.c_ulong(-1), 2)
    assert image, "Cannot capture owned window bounds"
    try:
        raw = bytearray()
        colors = set()
        for y in range(bounds.height):
            raw.append(0)
            for x in range(bounds.width):
                pixel = x11.XGetPixel(image, x, y)
                colors.add(pixel)
                raw.extend(((pixel >> 16) & 255, (pixel >> 8) & 255, pixel & 255))
        assert len(colors) > 16, f"Blank native screenshot: {name}"
        def chunk(kind, payload):
            return struct.pack("!I", len(payload)) + kind + payload + struct.pack("!I", zlib.crc32(kind + payload) & 0xffffffff)
        png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack("!IIBBBBB", bounds.width, bounds.height, 8, 2, 0, 0, 0))
        (output / name).write_bytes(png + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))
    finally:
        x11.XDestroyImage(image)


matrix, starts = [], []
environment = {**os.environ, "GTK_MODULES": "atk-bridge", "NO_AT_BRIDGE": "0",
               "GDK_BACKEND": "x11", "WEBKIT_DISABLE_DMABUF_RENDERER": "1",
               "LIBGL_ALWAYS_SOFTWARE": "1", "WEBKIT_DISABLE_COMPOSITING_MODE": "1",
               "XDG_CONFIG_HOME": str(output / "config"),
               "XDG_DATA_HOME": str(output / "xdg-data")}
try:
    for locale in ("en", "zh-CN"):
        for theme in ("dark", "light"):
            (data / "compass.json").write_text(json.dumps({
                "apiUrl": "http://127.0.0.1:1", "hoardUrl": "http://127.0.0.1:1",
                "language": locale, "theme": theme,
                "download": {"concurrency": 4, "mirror": ""},
                "defaults": {"memoryMb": 2048},
            }))
            with (output / f"{locale}-{theme}.log").open("wb") as log:
                started = time.monotonic()
                app = subprocess.Popen([str(output / "compass")], cwd=output,
                                       env=environment, stdout=log, stderr=log)
                try:
                    def owned_application():
                        assert app.poll() is None, f"Native app exited: {app.returncode}"
                        return next((node for node in pyatspi.Registry.getDesktop(0)
                                     if node.get_process_id() == app.pid), None)
                    application = wait(owned_application)
                    home_heading = "Play Minecraft" if locale == "en" else "开始游戏"
                    wait(lambda: any(node.getRoleName() == "heading" and node.name == home_heading
                                     for node in walk(application)))
                    if starts:
                        wait(lambda: any(node.getRoleName() == "heading" and node.name == "LinuxAcceptance"
                                         for node in walk(application)))
                    starts.append({"locale": locale, "theme": theme,
                                   "readyMs": round((time.monotonic() - started) * 1000)})
                    frame = find("frame", "Mythic Compass")
                    window = find_window(x11.XDefaultRootWindow(display))
                    assert window
                    labels = ["Home", "Instances", "Content", "Logs", "Settings"] if locale == "en" else ["首页", "实例", "内容", "日志", "设置"]
                    headings = [home_heading, "Instances", "Content", "Logs", "Settings"] if locale == "en" else [home_heading, "实例", "内容", "日志", "设置"]
                    for width in (1200, 760):
                        x11.XResizeWindow(display, window, width, 800)
                        x11.XSync(display, 0)
                        wait(lambda: frame.queryComponent().getExtents(pyatspi.DESKTOP_COORDS).width == width)
                        for label, heading in zip(labels, headings):
                            click(label)
                            wait(lambda: any(node.getRoleName() == "heading" and node.name == heading
                                             for node in walk(application)))
                            time.sleep(0.3)
                            name = f"{locale}-{theme}-{width}-{labels.index(label)}.png"
                            screenshot(name)
                            matrix.append({"locale": locale, "theme": theme, "width": width,
                                           "page": labels.index(label), "heading": heading})
                    if locale == "en" and theme == "dark":
                        entries = [node for node in walk(application)
                                   if node.getRoleName() == "entry" and node.name.startswith("Offline player name")]
                        assert len(entries) == 1
                        x11.XSetInputFocus(display, window, 2, 0)
                        assert entries[0].queryComponent().grabFocus()
                        shift = x11.XKeysymToKeycode(display, 0xFFE1)
                        for character in "LinuxAcceptance":
                            key = x11.XKeysymToKeycode(display, ord(character.lower()))
                            if character.isupper():
                                xtest.XTestFakeKeyEvent(display, shift, 1, 0)
                            xtest.XTestFakeKeyEvent(display, key, 1, 0)
                            xtest.XTestFakeKeyEvent(display, key, 0, 0)
                            if character.isupper():
                                xtest.XTestFakeKeyEvent(display, shift, 0, 0)
                        x11.XSync(display, 0)
                        wait(lambda: entries[0].queryText().getText(0, -1) == "LinuxAcceptance")
                        click("Add offline account")
                        wait(lambda: (data / "accounts.json").exists())
                        accounts = json.loads((data / "accounts.json").read_text())
                        assert any(account["playerName"] == "LinuxAcceptance" and account["kind"] == "offline"
                                   for account in accounts["accounts"])
                finally:
                    app.terminate()
                    try:
                        app.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        app.kill()
                        app.wait()
    result = {"platform": "Linux GTK/WebKit AT-SPI", "matrix": matrix, "starts": starts,
              "offlineAccountCreatedThroughUi": True,
              "offlineAccountRestoredAfterRestart": True,
              "exeSha256": hashlib.sha256(source.read_bytes()).hexdigest(),
              "scope": "40 native navigation and screenshot cases; offline account UI and persistence; no online game launch"}
    (output / "result.json").write_text(json.dumps(result, ensure_ascii=False, indent=2))
    print(json.dumps({"directory": str(output), **result}, ensure_ascii=False))
finally:
    x11.XCloseDisplay(display)
