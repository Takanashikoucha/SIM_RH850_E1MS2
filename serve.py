#!/usr/bin/env python3
"""RH850/E1M-S2 simulator — zero-dependency static server.

Serves the prebuilt web app (web/dist/) from Python's standard library.
No Node.js, no pip packages required — works on any system with Python 3.

Usage:
    python3 serve.py            # port 8080
    python3 serve.py 9000       # custom port
"""
import http.server
import os
import socketserver
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "web", "dist")


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".mjs": "text/javascript",
        ".css": "text/css",
    }

    def end_headers(self):
        # Allow the app to be embedded and keep CORS-free localhost happy.
        self.send_header("Cache-Control", "no-cache")
        super().end_headers()


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
    if not os.path.isdir(ROOT):
        sys.exit(
            f"Error: {ROOT} not found.\n"
            "The prebuilt app is missing — build it first:\n"
            "    npm install --prefix web && npm run build --prefix web\n"
            "or use the development server: ./start.sh --dev"
        )
    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("0.0.0.0", port), lambda *a, **kw: Handler(*a, directory=ROOT, **kw)) as httpd:
        print(f"RH850/E1M-S2 simulator running at http://127.0.0.1:{port}")
        print("Press Ctrl+C to stop.")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nStopped.")


if __name__ == "__main__":
    main()
