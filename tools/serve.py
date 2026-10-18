"""Static file server for the game (run by serve.sh, which finds the port).

Borrowed from midnight-playground's tools/serve.py: the web build
(dist/web/) is laid over the repo root file by file, so / is the game and
everything else in the tree (tools/, shots/) is where it is. Pages and
modules are sent no-store, so a phone never mixes an old module with a new
one; a precompressed `<file>.br` / `<file>.gz` beside a file under dist/ is
sent when the browser takes it.

The page POSTs what it starts on, its crashes and the tabs that vanished
to `log` (web/index.html); each report is appended to logs/client.jsonl
with the time and the address it came from, and summed up on stdout.
"""
import argparse
import datetime
import functools
import http.server
import json
import os
from pathlib import Path

LOG_MAX = 64 * 1024

NOT_BUILT = b"""<!doctype html><meta charset=utf-8><title>Snowboarding</title>
<body style="font:16px system-ui;background:#0b1424;color:#dfe8f5;padding:2rem">
<h1>Snowboarding</h1><p>No web build yet. Run <code>cargo xtask web --release</code>, then reload.</p>"""


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map,
                      '.wasm': 'application/wasm', '.js': 'text/javascript'}

    def translate_path(self, path):
        file = super().translate_path(path)
        root = os.path.abspath(self.directory)
        rel = os.path.relpath(file, root)
        if rel == '.':
            rel = ''
        over = os.path.join(root, 'dist', 'web', rel)
        if os.path.isfile(over) or os.path.isfile(os.path.join(over, 'index.html')):
            return over
        return file

    def send_head(self):
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
            f = open(file + ext, 'rb')
            self.send_response(200)
            self.send_header('Content-Type', self.guess_type(file))
            self.send_header('Content-Encoding', encoding)
            self.send_header('Content-Length', str(os.fstat(f.fileno()).st_size))
            self.send_header('Vary', 'Accept-Encoding')
            self.end_headers()
            return f
        return None

    def do_POST(self):
        if self.path.split('?', 1)[0] != '/log':
            self.send_error(404)
            return
        n = int(self.headers.get('Content-Length') or 0)
        if not 0 < n <= LOG_MAX:
            self.send_error(413)
            return
        body = self.rfile.read(n)
        try:
            report = json.loads(body)
            if not isinstance(report, dict):
                raise ValueError
        except ValueError:
            report = {'kind': 'unparsed', 'body': body[:2000].decode('utf-8', 'replace')}
        report = {'at': datetime.datetime.now().isoformat(timespec='seconds'),
                  'from': self.client_address[0], **report}
        logs = Path(self.directory) / 'logs'
        logs.mkdir(exist_ok=True)
        with open(logs / 'client.jsonl', 'a') as f:
            f.write(json.dumps(report) + '\n')
        gone = report.get('vanished')
        print(f"client {report['from']} {report.get('kind')} {report.get('backend')} "
              f"screen={report.get('screen')} mem={report.get('memMB')}"
              + (f" why={str(report.get('why'))[:200]!r}" if report.get('why') else '')
              + (f" VANISHED on {gone.get('screen')} mem={gone.get('memMB')} hidden={gone.get('hidden')}" if gone else ''),
              flush=True)
        self.send_response(204)
        self.end_headers()

    def end_headers(self):
        self.send_header('Cache-Control', 'no-store')
        super().end_headers()


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--port', type=int, required=True)
    ap.add_argument('--bind', default='0.0.0.0')
    args = ap.parse_args()
    root = Path(__file__).resolve().parent.parent
    handler = functools.partial(Handler, directory=str(root))
    with http.server.ThreadingHTTPServer((args.bind, args.port), handler) as httpd:
        print(f'Serving {root} on http://{args.bind}:{args.port}/', flush=True)
        httpd.serve_forever()


if __name__ == '__main__':
    main()
