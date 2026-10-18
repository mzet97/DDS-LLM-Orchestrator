#!/usr/bin/env python3
"""Proxy HTTP que FALHA a primeira request e repassa as demais (B1/retry)."""
import argparse
import http.server
import urllib.request

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--upstream", required=True)
    ap.add_argument("--port", type=int, default=8902)
    ap.add_argument("--fail-first", action="store_true")
    args = ap.parse_args()
    state = {"failed": 0}

    class H(http.server.BaseHTTPRequestHandler):
        def do_POST(self):
            length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(length)
            if args.fail_first and state["failed"] < 1:
                state["failed"] += 1
                self.send_response(503)
                self.end_headers()
                self.wfile.write(b'{"error":"flaky first"}')
                return
            req = urllib.request.Request(
                args.upstream + self.path, data=body,
                headers={"Content-Type": "application/json"})
            try:
                with urllib.request.urlopen(req, timeout=300) as up:
                    data = up.read()
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    self.wfile.write(data)
            except Exception as e:
                self.send_response(502)
                self.end_headers()
                self.wfile.write(str(e).encode())

        def log_message(self, *a):
            pass

    http.server.HTTPServer(("127.0.0.1", args.port), H).serve_forever()

if __name__ == "__main__":
    main()
