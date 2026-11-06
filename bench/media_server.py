"""Serveur media (stdlib seule) : sert un mp4 sur http://127.0.0.1:<port>/v/<n>.mp4

  python media_server.py --port 18082 --file video.mp4 [--make]
--make genere le mp4 (10 s, ~2-3 Mo) avec ffmpeg local s'il est absent (hors git).
"""

import argparse, os, shutil, subprocess, sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def make(path):
    ff = shutil.which("ffmpeg")
    if not ff:
        sys.exit("ffmpeg introuvable : generer le mp4 a la main (voir README)")
    subprocess.run(
        [
            ff,
            "-y",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=25:duration=10",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=10",
            "-c:v",
            "libx264",
            "-b:v",
            "1.8M",
            "-maxrate",
            "1.8M",
            "-bufsize",
            "3M",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
            path,
        ],
        check=True,
    )


class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):
        pass

    def do_HEAD(self):
        self.serve(False)

    def do_GET(self):
        self.serve(True)

    def serve(self, body):
        if not (self.path.startswith("/v/") and self.path.endswith(".mp4")):
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        data = self.server.data
        self.send_response(200)
        self.send_header("Content-Type", "video/mp4")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        if body:
            self.wfile.write(data)


if __name__ == "__main__":
    a = argparse.ArgumentParser()
    a.add_argument("--port", type=int, default=18082)
    a.add_argument("--file", required=True)
    a.add_argument("--make", action="store_true")
    a = a.parse_args()
    if a.make and not os.path.exists(a.file):
        make(a.file)
    s = ThreadingHTTPServer(("127.0.0.1", a.port), H)
    s.data = open(a.file, "rb").read()
    s.serve_forever()
