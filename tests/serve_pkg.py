#!/usr/bin/env python3
"""Static file server for tests that disables browser caching.

`python3 -m http.server` sends no Cache-Control header, so the persistent
chromium profile (tests/fixtures.ts) disk-caches unhashed assets like
tool.js/tool-audio.js across runs — regenerated pages then execute stale
JS. This wrapper serves the same directory with `Cache-Control: no-store`
so every request hits the current build.

Usage: serve_pkg.py <directory> <port>
"""

import contextlib
import socket
import sys
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer


class NoStoreHandler(SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


class DualStackServer(ThreadingHTTPServer):
    # Same dual-stack bind as `python3 -m http.server`, so localhost works
    # whether it resolves to 127.0.0.1 or ::1.
    address_family = socket.AF_INET6

    def server_bind(self):
        with contextlib.suppress(OSError):
            self.socket.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 0)
        return super().server_bind()


def main() -> None:
    directory, port = sys.argv[1], int(sys.argv[2])
    handler = partial(NoStoreHandler, directory=directory)
    DualStackServer(("::", port), handler).serve_forever()


if __name__ == "__main__":
    main()
