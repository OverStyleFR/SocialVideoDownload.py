"""Faux serveur Telegram Bot API (stdlib seule) pour mesurer un bot de telechargement.

  python fake_telegram.py --port 18081

Endpoints /bot<token>/<methode> (tout jeton accepte) : getMe, getUpdates (distribue les
messages injectes), sendMessage, editMessageText, sendVideo/sendDocument/sendAudio
(multipart : corps lu en entier, reception horodatee) ; toute autre methode repond ok:true.
Controle : POST /_inject?n=10&users=10&url=http://127.0.0.1:18082/v/{i}.mp4
           GET /_stats   POST /_stop
"""

import argparse, json, re, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

S = {
    "lock": threading.Lock(),
    "queue": [],
    "msgs": {},
    "files": [],
    "first_getupdates": None,
    "getupdates_calls": 0,
    "next_id": 1,
    "method_counts": {},
}
SENDS = {"sendvideo", "senddocument", "sendaudio"}


def reply(h, obj, code=200):
    b = json.dumps(obj).encode()
    h.send_response(code)
    h.send_header("Content-Type", "application/json")
    h.send_header("Content-Length", str(len(b)))
    h.end_headers()
    h.wfile.write(b)


class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):
        pass

    def do_GET(self):
        self.route()

    def do_POST(self):
        self.route()

    def route(self):
        u = urlparse(self.path)
        q = parse_qs(u.query)
        t_start = time.time()
        n = int(self.headers.get("Content-Length") or 0)
        body = b""
        while len(body) < n:
            c = self.rfile.read(min(65536, n - len(body)))
            if not c:
                break
            body += c
        t_end = time.time()
        if u.path == "/_inject":
            return reply(
                self,
                {
                    "ok": True,
                    "injected": inject(
                        int(q.get("n", ["10"])[0]),
                        int(q.get("users", ["10"])[0]),
                        q["url"][0],
                    ),
                },
            )
        if u.path == "/_stats":
            with S["lock"]:
                return reply(
                    self,
                    {
                        k: S[k]
                        for k in (
                            "first_getupdates",
                            "getupdates_calls",
                            "msgs",
                            "files",
                            "method_counts",
                        )
                    },
                )
        if u.path == "/_stop":
            reply(self, {"ok": True})
            threading.Thread(target=self.server.shutdown).start()
            return
        m = re.match(r"^/bot[^/]+/(\w+)$", u.path)
        if not m:
            return reply(self, {"ok": False, "description": "not found"}, 404)
        meth = m.group(1)
        low = meth.lower()
        with S["lock"]:
            S["method_counts"][meth] = S["method_counts"].get(meth, 0) + 1
        if low == "getme":
            return reply(
                self,
                {
                    "ok": True,
                    "result": {
                        "id": 123,
                        "is_bot": True,
                        "first_name": "Fake",
                        "username": "fake_bot",
                    },
                },
            )
        if low == "getupdates":
            with S["lock"]:
                S["getupdates_calls"] += 1
                if S["first_getupdates"] is None:
                    S["first_getupdates"] = t_start
            for _ in range(
                10
            ):  # long-poll court : rend la main des qu'un message arrive
                with S["lock"]:
                    if S["queue"]:
                        ups, S["queue"] = S["queue"], []
                        now = time.time()
                        for up in ups:
                            S["msgs"][str(up["message"]["message_id"])]["served"] = now
                        return reply(self, {"ok": True, "result": ups})
                time.sleep(0.1)
            return reply(self, {"ok": True, "result": []})
        chat = re.search(rb'name="chat_id"\r\n\r\n(-?\d+)', body)
        rep = re.search(rb'name="reply_to_message_id"\r\n\r\n(\d+)', body)
        if chat:
            chat_id = int(chat.group(1))
            reply_to = int(rep.group(1)) if rep else None
        else:
            try:
                d = json.loads(body or b"{}")
            except Exception:
                d = {}
            chat_id = d.get("chat_id", 0)
            reply_to = d.get("reply_to_message_id")
        if low in SENDS:
            with S["lock"]:
                S["files"].append(
                    {
                        "method": meth,
                        "reply_to": reply_to,
                        "chat_id": chat_id,
                        "t_start": t_start,
                        "t_end": t_end,
                        "bytes": len(body),
                    }
                )
        with S["lock"]:
            S["next_id"] += 1
            mid = 100000 + S["next_id"]
        if low.startswith("send") or low.startswith("edit"):
            return reply(
                self,
                {
                    "ok": True,
                    "result": {
                        "message_id": mid,
                        "date": int(t_end),
                        "chat": {"id": chat_id, "type": "private"},
                    },
                },
            )
        return reply(self, {"ok": True, "result": True})


def inject(n, users, url_tpl):
    now = time.time()
    with S["lock"]:
        for i in range(1, n + 1):
            uid = 1000 + (i - 1) % users
            url = url_tpl.format(i=i)
            S["msgs"][str(i)] = {
                "user": uid,
                "url": url,
                "injected": now,
                "served": None,
            }
            S["queue"].append(
                {
                    "update_id": 5000 + i,
                    "message": {
                        "message_id": i,
                        "date": int(now),
                        "text": url,
                        "chat": {"id": uid, "type": "private", "username": f"u{uid}"},
                        "from": {
                            "id": uid,
                            "is_bot": False,
                            "first_name": f"U{uid}",
                            "username": f"u{uid}",
                        },
                    },
                }
            )
    return n


if __name__ == "__main__":
    a = argparse.ArgumentParser()
    a.add_argument("--port", type=int, default=18081)
    a.add_argument("--host", default="127.0.0.1")  # 0.0.0.0 pour un bot en conteneur
    a = a.parse_args()
    ThreadingHTTPServer((a.host, a.port), H).serve_forever()
