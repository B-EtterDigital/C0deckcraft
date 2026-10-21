#!/usr/bin/env python3
"""Real Chromium/CDP smoke. Uses only Python's standard library and the built guest page."""

import argparse
import base64
import functools
import http.server
import json
import os
from pathlib import Path
import select
import subprocess
import tempfile
import threading
import time


ROOT = Path(__file__).resolve().parents[2]


class Browser:
    def __init__(self, profile, log):
        child_read, self.write = os.pipe()
        self.read, child_write = os.pipe()

        def descriptors():
            os.dup2(child_read, 3)
            os.dup2(child_write, 4)

        self.process = subprocess.Popen([
            "chromium", "--headless", "--no-sandbox", "--disable-dev-shm-usage",
            "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader",
            "--disable-features=Vulkan", "--disable-background-networking",
            "--renderer-process-limit=2", "--num-raster-threads=2",
            "--remote-debugging-pipe", f"--user-data-dir={profile}", "about:blank",
        ], preexec_fn=descriptors, pass_fds=tuple({3, 4, child_read, child_write}), stdout=log, stderr=log)
        os.close(child_read)
        os.close(child_write)
        self.buffer = b""
        self.sequence = 0
        self.events = []
        self.session = None

    def request(self, method, params=None, timeout=60):
        self.sequence += 1
        payload = {"id": self.sequence, "method": method, "params": params or {}}
        if self.session:
            payload["sessionId"] = self.session
        os.write(self.write, json.dumps(payload).encode() + b"\0")
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            while b"\0" in self.buffer:
                raw, self.buffer = self.buffer.split(b"\0", 1)
                message = json.loads(raw)
                if message.get("id") == self.sequence:
                    if "error" in message:
                        raise RuntimeError(message["error"])
                    return message.get("result", {})
                self.events.append(message)
            if select.select([self.read], [], [], min(1, max(0, deadline - time.monotonic())))[0]:
                chunk = os.read(self.read, 1024 * 1024)
                if not chunk:
                    raise RuntimeError("Chromium closed the debugging pipe")
                self.buffer += chunk
        raise TimeoutError(method)

    def evaluate(self, expression):
        result = self.request("Runtime.evaluate", {"expression": expression, "returnByValue": True, "awaitPromise": True, "userGesture": True})
        if "exceptionDetails" in result:
            raise RuntimeError(result["exceptionDetails"])
        return result.get("result", {}).get("value")

    def guest(self, method, params=None, ok=True):
        envelope = json.dumps({"method": method, "params": params or {}})
        reply = json.loads(self.evaluate(f"window.__c0hostRequest({json.dumps(envelope)})"))
        assert reply.get("ok") is ok, (method, reply)
        return reply.get("result") if ok else reply["error"]

    def close(self):
        self.process.terminate()
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=10)
        os.close(self.write)
        os.close(self.read)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--expect-startup-failure", action="store_true")
    parser.add_argument("--export-restricted-fixture", action="store_true")
    parser.add_argument("--dist", type=Path, default=ROOT / "dist/c0host")
    args = parser.parse_args()
    assert (args.dist / "index.html").is_file(), "Build c0/deckcraft-host first"

    class Handler(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *_args):
            pass

    handler = functools.partial(Handler, directory=str(args.dist))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    with tempfile.TemporaryDirectory(prefix="c0deck-chromium-") as profile, open("/tmp/c0deck-chromium.log", "w") as log:
        browser = Browser(profile, log)
        try:
            target = browser.request("Target.createTarget", {"url": f"http://127.0.0.1:{server.server_port}/?webgl"})["targetId"]
            browser.session = browser.request("Target.attachToTarget", {"targetId": target, "flatten": True})["sessionId"]
            browser.request("Runtime.enable")
            browser.request("Page.enable")
            browser.request("Emulation.setDeviceMetricsOverride", {"width": 1440, "height": 900, "deviceScaleFactor": 1, "mobile": False})
            deadline = time.monotonic() + (30 if args.expect_startup_failure else 120)
            ready = False
            while time.monotonic() < deadline:
                if browser.evaluate("typeof window.__deckcraftControl") == "function":
                    ready = True
                    break
                time.sleep(0.25)
            if args.expect_startup_failure:
                assert not ready, "Unpatched page unexpectedly started"
                errors = [e for e in browser.events if e.get("method") in ["Runtime.exceptionThrown", "Runtime.consoleAPICalled"]]
                print("UNPATCHED STARTUP EVENTS:", json.dumps(errors)[-12000:])
                return
            assert ready, "DeckCraft bridge did not start"
            assert browser.evaluate("typeof window.__c0hostRequest") == "function"
            # Let fonts install and the first frame paint before freezing presentation/timers.
            time.sleep(2)
            if args.export_restricted_fixture:
                data = base64.b64encode((ROOT / "crates/media/tests/fixtures/clip-h264.mp4").read_bytes()).decode()
                browser.guest("engine.execute", {"command": "insert.video", "params": {"name": "clip-h264.mp4", "data": data}})
                snapshot = browser.guest("c0.documentBytes", {"format": "pptx"})
                Path("/tmp/c0deck-restricted.pptx").write_bytes(base64.b64decode(snapshot["bytes"]))
                print("PASS codec-enabled build generated /tmp/c0deck-restricted.pptx from the project's CC0 fixture")
                return
            browser.evaluate("window.__c0savedRAF = window.requestAnimationFrame; window.__c0savedTimer = window.setTimeout; window.requestAnimationFrame = () => 0; window.setTimeout = () => 0")
            initial = browser.guest("c0.inspect")
            assert initial["slideCount"] == 1 and initial["slides"][0]["title"] == "", initial
            assert initial["dirty"] is False, initial
            print("PASS blank startup; no process::id crash")
            slide_id = initial["slides"][0]["id"]
            assert browser.guest("c0.goTo", {"slide": 0})["index"] == 0
            assert browser.guest("c0.goTo", {"slide": str(slide_id)})["index"] == 0
            assert browser.guest("c0.show", {"action": "start", "from": 0}) == {"showing": True}
            assert browser.guest("c0.show", {"action": "end"}) == {"showing": False}
            settings = browser.guest("c0.set", {"themeTokens": {"accent": "#259080", "dark": "#ffffff", "text": 3}, "panels": {
                "title_bar": False, "file_tab": False, "start_screen": False, "community_links": False, "status_bar": False,
                "ribbon_collapsed": True, "notes": False, "ruler": False, "unknown": True,
            }})
            assert "themeTokens.accent" in settings["applied"]
            assert "themeTokens.text" in settings["invalid"]
            assert "themeTokens.dark" in settings["unknown"] and "panels.unknown" in settings["unknown"]
            browser.guest("ui.set", {"dark": True})
            assert browser.guest("c0.inspect")["theme"] == "dark"
            browser.guest("ui.set", {"dark": False})
            for fmt in ["pptx", "deckcraft"]:
                before = browser.guest("c0.inspect")
                snapshot = browser.guest("c0.documentBytes", {"format": fmt})
                assert before == browser.guest("c0.inspect"), "Snapshot changed the document"
                assert snapshot["format"] == fmt and snapshot["bytes"]
                browser.guest("c0.openFile", {"name": snapshot["name"], "bytes": snapshot["bytes"]})
                assert browser.guest("c0.inspect")["slideCount"] == 1
            browser.guest("c0.newDocument", {"template": "blank"})
            assert browser.guest("c0.cancelFileDialog") == {"cancelled": False}
            print("PASS all eight c0 methods, PPTX/native round trips, themes and chrome with frames/timers paused")

            browser.request("Page.setInterceptFileChooserDialog", {"enabled": True})
            browser.guest("engine.execute", {"command": "app.openDialog"})
            assert browser.evaluate("!!document.getElementById('c0deck-file-picker')")
            browser.evaluate("window.__c0retiredInput = document.getElementById('c0deck-file-picker')")
            assert browser.guest("c0.cancelFileDialog") == {"cancelled": True}
            assert not browser.evaluate("!!document.getElementById('c0deck-file-picker')")
            browser.evaluate("window.__c0retiredInput.dispatchEvent(new Event('change'))")
            assert browser.guest("c0.inspect")["slideCount"] == 1
            print("PASS native browser picker cancellation and inert late change event")

            restricted = Path("/tmp/c0deck-restricted.pptx")
            if restricted.is_file():
                browser.guest("c0.openFile", {"name": restricted.name, "bytes": base64.b64encode(restricted.read_bytes()).decode()})
                error = browser.guest("engine.execute", {"command": "media.play"}, ok=False)
                assert "this format is not supported here" in error, error
                browser.guest("engine.execute", {"command": "media.stop"})
                browser.guest("c0.newDocument")
                print("PASS imported H.264/AAC PPTX opens, but explicit playback returns the app's unsupported-format error")

            for name, command in [("clip-h264.mp4", "insert.video"), ("clip-hevc.mp4", "insert.video"), ("tone.aac", "insert.audio"), ("tone.m4a", "insert.audio")]:
                data = base64.b64encode((ROOT / "crates/media/tests/fixtures" / name).read_bytes()).decode()
                error = browser.guest("engine.execute", {"command": command, "params": {"name": name, "data": data}}, ok=False)
                assert "this format is not supported here" in error, (name, error)
            print("PASS restricted H.264/HEVC/AAC insertions return clear app errors")

            for name, command in [("tone.mp3", "insert.audio"), ("clip-vp9.webm", "insert.video")]:
                data = base64.b64encode((ROOT / "crates/media/tests/fixtures" / name).read_bytes()).decode()
                inserted = browser.guest("engine.execute", {"command": command, "params": {"name": name, "data": data}})
                shape = inserted["id"]
                browser.guest("engine.execute", {"command": "media.play", "params": {"id": shape}})
                time.sleep(0.15)
                browser.guest("c0.inspect")
                info = browser.guest("engine.execute", {"command": "media.info", "params": {"id": shape}})
                assert info["probe"]["playable"] is True, info
                assert info["status"]["positionMs"] > 0, info
                browser.guest("engine.execute", {"command": "media.stop", "params": {"id": shape}})
            print("PASS MP3/VP9 browser playback positions advance with the existing host clock")

            browser.guest("engine.execute", {"command": "slide.new", "params": {"title": "Unsaved browser work"}})
            dirty = browser.guest("c0.inspect")
            assert dirty["dirty"] and dirty["slideCount"] == 2
            assert browser.guest("c0.newDocument") == {"pending": "saveChanges"}
            assert browser.guest("c0.cancelFileDialog") == {"cancelled": False}
            assert browser.guest("ui.inspect")["dialog"] == "close"
            browser.guest("ui.dialog.close")
            assert browser.guest("c0.inspect") == dirty
            snapshot = browser.guest("c0.documentBytes")
            assert browser.guest("c0.openFile", {"name": snapshot["name"], "bytes": snapshot["bytes"]}) == {"pending": "saveChanges"}
            browser.guest("ui.dialog.close")
            assert browser.guest("c0.inspect") == dirty
            print("PASS unsaved Open/New cancellation preserves document and revision")

            for method, params in [("c0.unknown", {}), ("c0.goTo", {"slide": -1}), ("c0.show", {"action": "start", "from": 999}),
                                   ("c0.openFile", {"name": "../bad.pptx", "bytes": "AAAA"}), ("c0.set", {"panels": []})]:
                browser.guest(method, params, ok=False)
            malformed = json.loads(browser.evaluate("window.__c0hostRequest('bad json')"))
            assert malformed["ok"] is False
            browser.guest("engine.execute", {"command": "app.links"}, ok=False)
            assert "app.links" not in json.dumps(browser.guest("engine.commands"))
            print("PASS malformed requests and hidden app.links")

            browser.evaluate("window.requestAnimationFrame = window.__c0savedRAF; window.setTimeout = window.__c0savedTimer")
            # Pausing RAF deliberately strands eframe's scheduled callback. Reload for an
            # independent painted-UI check rather than inspecting a stale slideshow frame.
            browser.evaluate("window.__c0smokeOldPage = true")
            browser.request("Page.reload")
            deadline = time.monotonic() + 120
            painted_ready = False
            while time.monotonic() < deadline:
                try:
                    painted_ready = browser.evaluate("!window.__c0smokeOldPage && typeof window.__c0hostRequest === 'function'")
                    if painted_ready:
                        break
                except RuntimeError:
                    pass  # The old execution context can disappear during navigation.
                time.sleep(0.25)
            assert painted_ready, "Fresh painted page did not start"
            browser.guest("c0.set", {"themeTokens": {"accent": "#259080"}, "panels": {
                "title_bar": False, "file_tab": False, "start_screen": False, "community_links": False,
                "status_bar": False, "ribbon_collapsed": False, "notes": False, "ruler": False,
            }})
            browser.guest("ui.set", {"dark": False})
            browser.guest("engine.execute", {"command": "app.about"})
            time.sleep(1)
            screenshot = browser.request("Page.captureScreenshot", {"format": "png"})["data"]
            Path("/tmp/c0deck-browser.png").write_bytes(base64.b64decode(screenshot))
            browser.guest("ui.dialog.close")
            exceptions = [e for e in browser.events if e.get("method") == "Runtime.exceptionThrown"]
            assert not exceptions, exceptions
            print("PASS real painted page; screenshot /tmp/c0deck-browser.png; zero JavaScript exceptions")
        finally:
            browser.close()
            server.shutdown()


if __name__ == "__main__":
    main()
