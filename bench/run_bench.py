"""Banc commun : lance faux Telegram + serveur media, lance le bot vise, injecte N messages
simultanes (U utilisateurs), mesure latence message->fichier recu, debit, p50/p95, demarrage.

  python run_bench.py --media-file video.mp4 --bot-cmd "python main.py" --bot-cwd ../svd-ancien \
      --out resultat.json [--messages 10 --users 10 --fake-port 18081 --media-port 18082 --timeout 180]

Le bot est configure par l'environnement : BOT_TOKEN=123:ABC, TELEGRAM_API_URL=http://127.0.0.1:<fake-port>,
UNSAFE_ALLOW_PRIVATE_HOSTS=1. Sans --bot-cmd, on attend un bot lance a part (memes variables).
"""

import argparse, json, math, os, shlex, subprocess, sys, time, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))


def get(url, data=None):
    return json.loads(
        urllib.request.urlopen(
            urllib.request.Request(
                url, data=data, method="POST" if data is not None else "GET"
            ),
            timeout=10,
        ).read()
    )


def pct(v, p):
    v = sorted(v)
    return v[min(len(v) - 1, math.ceil(p / 100 * len(v)) - 1)] if v else None


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--media-file", required=True)
    a.add_argument("--bot-cmd", default="")
    a.add_argument("--bot-cwd", default=".")
    a.add_argument("--out", required=True)
    a.add_argument("--messages", type=int, default=10)
    a.add_argument("--users", type=int, default=10)
    a.add_argument("--fake-port", type=int, default=18081)
    a.add_argument("--media-port", type=int, default=18082)
    a.add_argument("--timeout", type=int, default=180)
    a.add_argument("--startup-timeout", type=int, default=120)
    a = a.parse_args()
    py = sys.executable
    procs = []
    try:
        procs.append(
            subprocess.Popen(
                [py, os.path.join(HERE, "fake_telegram.py"), "--port", str(a.fake_port)]
            )
        )
        procs.append(
            subprocess.Popen(
                [
                    py,
                    os.path.join(HERE, "media_server.py"),
                    "--port",
                    str(a.media_port),
                    "--file",
                    a.media_file,
                ]
            )
        )
        base = f"http://127.0.0.1:{a.fake_port}"
        for _ in range(50):
            try:
                get(base + "/_stats")
                break
            except Exception:
                time.sleep(0.1)
        bot = None
        t_launch = time.time()
        if a.bot_cmd:
            env = dict(
                os.environ,
                BOT_TOKEN="123:ABC",
                TELEGRAM_API_URL=base,
                UNSAFE_ALLOW_PRIVATE_HOSTS="1",
                PYTHONUNBUFFERED="1",
            )
            bot = subprocess.Popen(
                shlex.split(a.bot_cmd),
                cwd=a.bot_cwd,
                env=env,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            procs.append(bot)
        while (
            time.time() - t_launch < a.startup_timeout
            and get(base + "/_stats")["first_getupdates"] is None
        ):
            time.sleep(0.05)
        st = get(base + "/_stats")
        if st["first_getupdates"] is None:
            sys.exit("le bot n'a jamais appele getUpdates")
        startup_s = st["first_getupdates"] - t_launch
        time.sleep(1)  # laisse finir l'initialisation (menu, threads)
        url_tpl = f"http://127.0.0.1:{a.media_port}/v/{{i}}.mp4"
        get(
            f"{base}/_inject?n={a.messages}&users={a.users}&url="
            + urllib.request.quote(url_tpl, safe=""),
            b"",
        )
        t0 = time.time()
        while time.time() - t0 < a.timeout:
            st = get(base + "/_stats")
            if len({f["reply_to"] for f in st["files"]}) >= a.messages:
                break
            time.sleep(0.2)
        lat, size = {}, 0
        for f in st["files"]:
            k = str(f["reply_to"])
            if k in st["msgs"] and k not in lat:
                lat[k] = f["t_end"] - st["msgs"][k]["served"]
                size += f["bytes"]
        v = sorted(lat.values())
        wall = (
            (
                max(f["t_end"] for f in st["files"])
                - min(m["served"] for m in st["msgs"].values())
            )
            if st["files"]
            else None
        )
        res = {
            "messages": a.messages,
            "users": a.users,
            "files_received": len(lat),
            "startup_s": round(startup_s, 3),
            "latency_p50_s": round(pct(v, 50), 3) if v else None,
            "latency_p95_s": round(pct(v, 95), 3) if v else None,
            "latency_max_s": round(v[-1], 3) if v else None,
            "wall_s": round(wall, 3) if wall else None,
            "throughput_files_per_s": round(len(lat) / wall, 3) if wall else None,
            "throughput_mb_per_s": round(size / 1e6 / wall, 3) if wall else None,
            "latencies_s": {
                k: round(x, 3)
                for k, x in sorted(lat.items(), key=lambda kv: int(kv[0]))
            },
            "method_counts": st["method_counts"],
        }
        json.dump(res, open(a.out, "w"), indent=1)
        print(
            json.dumps(
                {k: res[k] for k in res if k not in ("latencies_s", "method_counts")}
            )
        )
        return 0 if len(lat) == a.messages else 1
    finally:
        for p in procs:
            p.terminate()
        for p in procs:
            try:
                p.wait(5)
            except Exception:
                p.kill()


if __name__ == "__main__":
    sys.exit(main())
