#!/usr/bin/env python3
"""Serve the built Salatiga Event Map site and proxy /api to the Axum API.

Run from anywhere:
    python3 /workspace/event-map/deploy/serve.py

Then open:
    http://<tailscale-ip>:8090/     (from the phone)
    http://localhost:8090/          (from this sandbox)
"""
import http.server
import mimetypes
import os
import socketserver
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(os.environ.get(
    "SEM_ROOT",
    str(Path(__file__).resolve().parents[1] / "target" / "dx" / "app" / "release" / "web" / "public"),
))
API = os.environ.get("SEM_API", "http://127.0.0.1:8080")
PORT = int(os.environ.get("SEM_PORT", "8090"))

mimetypes.add_type("application/wasm", ".wasm")
mimetypes.add_type("text/javascript", ".js")
mimetypes.add_type("text/css", ".css")


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(ROOT), **kwargs)

    def log_message(self, fmt, *args):
        super().log_message(fmt, *args)

    def _proxy(self):
        # Forward the request to the Axum API on the same origin path.
        target = API + self.path
        length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(length) if length else None
        headers = {}
        for key in ("Content-Type", "Authorization", "Accept"):
            if self.headers.get(key):
                headers[key] = self.headers[key]
        req = urllib.request.Request(target, data=body, headers=headers, method=self.command)
        try:
            with urllib.request.urlopen(req, timeout=15) as resp:
                data = resp.read()
                self.send_response(resp.status)
                for k, v in resp.headers.items():
                    if k.lower() not in ("transfer-encoding", "content-length", "connection"):
                        self.send_header(k, v)
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
        except urllib.error.HTTPError as e:
            data = e.read()
            self.send_response(e.code)
            self.send_header("Content-Type", e.headers.get("Content-Type", "application/json"))
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
        except Exception as e:
            msg = ('{"error":"api unavailable: %s"}' % str(e)).encode()
            self.send_response(502)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(msg)))
            self.end_headers()
            self.wfile.write(msg)

    def do_GET(self):
        if self.path.startswith("/api/"):
            return self._proxy()
        return super().do_GET()

    def do_POST(self):
        if self.path.startswith("/api/"):
            return self._proxy()
        self.send_error(405)

    def do_PUT(self):
        if self.path.startswith("/api/"):
            return self._proxy()
        self.send_error(405)

    def do_DELETE(self):
        if self.path.startswith("/api/"):
            return self._proxy()
        self.send_error(405)

    def end_headers(self):
        # Always allow the app shell to refresh; static assets can cache briefly.
        if self.path == "/" or self.path.endswith(".html"):
            self.send_header("Cache-Control", "no-cache")
        super().end_headers()


class Server(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
    allow_reuse_address = True


if __name__ == "__main__":
    if not (ROOT / "index.html").exists():
        raise SystemExit(f"Build output missing: {ROOT}")
    os.chdir(ROOT)
    with Server(("0.0.0.0", PORT), Handler) as httpd:
        print(f"Salatiga Event Map -> http://0.0.0.0:{PORT}/  (API {API})", flush=True)
        httpd.serve_forever()
