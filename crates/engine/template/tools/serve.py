"""Static file server for the game (run by serve.sh, which finds the port).

The web build (DIST, `cargo xtask web`) is laid over the repository root
file by file, so / is the game and everything else in the tree (tools/,
reports) is where it is; the build itself still answers at /<DIST>/ too.

python3 -m http.server sends Last-Modified but no Cache-Control, so browsers
guess how long each file stays fresh, and a phone then reloads with some
modules new and some hours old. Everything is sent no-store, so every load
is one consistent version, except the PINNED directories (big, rarely
changed files: fonts, recorded audio), which are no-cache: always
revalidated, but a 304 saves re-downloading them. A precompressed
`<file>.br` or `<file>.gz` beside a file under dist/ is sent with
Content-Encoding when the browser takes it (brotli only on https, so phones
on the tailnet front get it and plain http gets gzip), so load times are as
they will be. Before the first build, / is a page saying so.

The port is `--port`, required, with no default: serve.sh passes $PORT or
`proj port`.
"""
import argparse
import functools
import http.server
import os
from pathlib import Path

# The web build, from the repository root.
DIST = os.path.join('dist', 'next')
# URL prefixes sent no-cache instead of no-store.
PINNED = ('/assets/fonts/',)
TITLE = 'New Game'

NOT_BUILT = (f"""<!doctype html><meta charset=utf-8><title>{TITLE}</title>
<body style="font:16px system-ui;background:#0b1424;color:#dfe8f5;padding:2rem">
<h1>{TITLE}</h1><p>No web build yet. Run <code>cargo xtask web --release</code>, then reload.</p>""").encode()


class Handler(http.server.SimpleHTTPRequestHandler):
    # The wasm must be application/wasm for streaming compilation.
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map,
                      '.wasm': 'application/wasm', '.js': 'text/javascript', '.mjs': 'text/javascript'}

    def pinned(self):
        # No path yet when the request line itself is bad (e.g. a browser
        # trying https on this port), and send_error still sends headers.
        return getattr(self, 'path', '').startswith(PINNED)

    def translate_path(self, path):
        """The build over the root file by file, as `cp -r dist/next/. _site/`
        lays it on Pages: a directory only counts when the build has its page."""
        file = super().translate_path(path)
        root = os.path.abspath(self.directory)
        rel = os.path.relpath(file, root)
        if rel == '.':
            rel = ''
        over = os.path.join(root, DIST, rel)
        if os.path.isfile(over) or os.path.isfile(os.path.join(over, 'index.html')):
            return over
        return file

    def send_head(self):
        # Never answer 304 for a file the browser was told not to keep: an
        # old copy cached before this server existed must be replaced.
        if not self.pinned():
            del self.headers['If-Modified-Since']
        path = getattr(self, 'path', '').split('?', 1)[0].split('#', 1)[0]
        if path == '/' and not os.path.isfile(self.translate_path('/index.html')):
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
            self.send_header('Content-Length', str(len(NOT_BUILT)))
            self.end_headers()
            self.wfile.write(NOT_BUILT)
            return None
        return self.precompressed(path) or super().send_head()

    def precompressed(self, path):
        file = self.translate_path(path)
        if not file.startswith(os.path.join(os.path.abspath(self.directory), 'dist') + os.sep):
            return None
        if not os.path.isfile(file):
            return None
        accepted = {t.split(';', 1)[0].strip().lower()
                    for t in self.headers.get('Accept-Encoding', '').split(',')}
        for encoding, ext in (('br', '.br'), ('gzip', '.gz')):
            if encoding not in accepted or not os.path.isfile(file + ext):
                continue
            try:
                f = open(file + ext, 'rb')
            except OSError:
                continue
            self.send_response(200)
            self.send_header('Content-Type', self.guess_type(file))
            self.send_header('Content-Encoding', encoding)
            self.send_header('Content-Length', str(os.fstat(f.fileno()).st_size))
            self.send_header('Vary', 'Accept-Encoding')
            self.end_headers()
            return f
        return None

    def end_headers(self):
        self.send_header('Cache-Control', 'no-cache' if self.pinned() else 'no-store')
        super().end_headers()


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--port', type=int, required=True, help='from serve.sh: $PORT or `proj port`; no default')
    ap.add_argument('--bind', default='0.0.0.0')
    args = ap.parse_args()
    root = Path(__file__).resolve().parent.parent
    handler = functools.partial(Handler, directory=str(root))
    with http.server.ThreadingHTTPServer((args.bind, args.port), handler) as httpd:
        print(f'Serving {root} on http://{args.bind}:{args.port}/', flush=True)
        httpd.serve_forever()


if __name__ == '__main__':
    main()
