# Agent Guidelines: SocialVideoDownload.py

## Project Overview
A **Telegram bot** that downloads videos and music from social media links (YouTube, TikTok, etc.) and sends them back to users. Deployed as a Docker container to GitHub Packages.

- **Language**: Python 3.11 (target, required — Python 3.13+ is incompatible with `python-telegram-bot==13.7`)
- **Primary file**: `main.py` (~80 lines, orchestration only)
- **Framework**: `python-telegram-bot==13.7` — **critical**: this is the old v13 synchronous API (`Updater`, `Dispatcher`, `use_context=True`). Do NOT use modern v20+ async patterns (`Application`, `ContextTypes`, etc.); they are incompatible.
- **Deployment target**: Docker image → `ghcr.io/...` (GitHub Container Registry)

## Project Structure

```
SocialVideoDownload.py/
├── main.py                  # Entry point — token loading, handler registration, polling loop
├── config.py                # Centralized constants (VERSION, FFMPEG_PATH, retention, disk thresholds)
├── requirements.txt         # Python deps
├── Dockerfile               # Multi-stage build (ffmpeg → builder → final)
├── .env.example             # Template for BOT_TOKEN and other env vars
├── .github/workflows/       # CI/CD (deploy.yml)
├── commands/                # Telegram command handlers
│   ├── start.py
│   ├── help.py
│   ├── download.py
│   ├── music.py
│   ├── stats.py
│   └── auto_download.py
└── utils/                   # Shared utilities
    ├── token_loader.py      # Reads BOT_TOKEN from .env (auto-creates template if missing)
    ├── logger.py            # Colored console + file logging
    ├── file_manager.py      # Hash-based deduplication (SHA-256 of URLs in downloads/hashes.txt)
    ├── disk_manager.py      # Free-space monitoring, emergency cleanup of downloads/
    ├── cache.py             # JSON metadata cache (download_temp/cache_metadata.json) with TTL
    ├── retention.py         # Sets file mtime to future based on retention policy
    ├── upload.py            # Telegram upload (< 35 MB) or external fallback via curl.libriciel.fr
    ├── curl_uploader.py     # PycURL-based upload to curl.libriciel.fr with progress callback
    └── progress_file.py     # Progress tracking helpers
```

## Essential Commands

| Command | Purpose |
|---------|---------|
| `python main.py` | Run the bot locally |
| `docker build -t socialvideodownload .` | Build Docker image |
| `docker run -e BOT_TOKEN=your_token socialvideodownload` | Run container (pass token via env) |
| `pip install -r requirements.txt` | Install dependencies |
| `echo "No tests to run"` | Current test suite (there are **no tests**) |

**No test framework is configured.** The CI workflow explicitly skips tests with a placeholder `echo`.

## Architecture & Data Flow

```
Telegram Message
      ↓
  python-telegram-bot v13 handlers (commands/*.py)
      ↓
  yt-dlp Python API  (yt_dlp.YoutubeDL)
      ↓
  downloads/<title>.<ext>
      ↓
  upload.py decides:
      ├─ < 35 MB → send via Telegram API (reply_video / reply_audio / reply_document)
      └─ ≥ 35 MB → upload via curl.libriciel.fr with progress updates, return URL
```

- **Synchronous**: The entire bot is sync. All handlers block on I/O. Do not introduce `async`/`await` unless migrating the entire framework.
- **Subprocess**: Only `ffmpeg` (for `/music` conversion to MP3) spawns an external binary.

### Caching & Deduplication
- **URL deduplication**: `utils/file_manager.py` stores SHA-256 hashes of URLs in `downloads/hashes.txt`. Before downloading, the bot checks if the hash exists.
- **Metadata cache**: `utils/cache.py` maintains `download_temp/cache_metadata.json` with per-entry TTL:
  - Small files (≤ 5 MB) → 24 hours
  - Large files (> 5 MB) → 1 hour

### Retention Policy
- `utils/retention.py` sets file `mtime` to a future timestamp based on size/type:
  - MP3 files → long retention (default 24h)
  - Small files (< 4 MB default) → long retention
  - Large files → short retention (default 2h)
- These values are configurable via `.env` (`SMALL_FILE_SIZE_MB`, `RETENTION_SMALL_HOURS`, `RETENTION_LARGE_HOURS`).

### Startup Behavior (`main()`)
1. Loads cache from `download_temp/cache_metadata.json` (creates empty if missing)
2. Checks free disk space; triggers emergency cleanup if below `MIN_FREE_SPACE_MB` (default 500)
3. Registers all command handlers on the dispatcher
4. Sets Telegram bot command menu (`/start`, `/help`, `/download`)
5. Starts a daemon thread for periodic cleanup every `CLEANUP_INTERVAL_HOURS`
6. Begins polling

### Periodic Cleanup (`scheduled_cleanup` thread)
- Runs every `CLEANUP_INTERVAL_HOURS` (default 24h)
- Calls `clear_downloads()` which empties `downloads/` but preserves `hashes.txt`

## External Dependencies (Binaries)

| Binary | Expected Location | Used For |
|--------|-------------------|----------|
| `yt-dlp` | Python package (`yt_dlp`) | Video/audio downloading |
| `ffmpeg` | `ffmpeg/ffmpeg-7.0.2-amd64-static/ffmpeg` (local) or `/usr/local/bin/ffmpeg` (Docker) | MP3 extraction (`/music`) |

- In Docker, FFmpeg is copied from a multi-stage `ghcr.io/linuxserver/ffmpeg:latest` image.
- Locally, the bundled static build `ffmpeg/ffmpeg-7.0.2-amd64-static/ffmpeg` is used (path configurable via `.env`).

## Configuration & Secrets

- **Token source**: `.env` file, variable `BOT_TOKEN`. Auto-generated if missing by `utils/token_loader.py`.
- **No `token.txt`**: The old flat-file approach has been replaced by `python-dotenv`.
- **Environment variables loaded**:
  - `BOT_TOKEN` — Telegram bot token (required)
  - `VERSION` — Bot version string (default `V.8-7`)
  - `DEVELOPED_BY` — Author string (default `Tom V. | OverStyleFR`)
  - `FFMPEG_PATH` — Path to ffmpeg binary
  - `CLEANUP_INTERVAL_HOURS`, `MIN_FREE_SPACE_MB` — Disk/rotation tuning
  - `SMALL_FILE_SIZE_MB`, `RETENTION_SMALL_HOURS`, `RETENTION_LARGE_HOURS` — Retention tuning

## Code Patterns & Conventions

- **Language**: UI strings and comments are in **French** (e.g., "Téléchargement en cours", "Veuillez patienter..."). Maintain this for user-facing messages.
- **Logging**: Single colored logger (`utils/logger.py`):
  - `console_logger` (TelegramBot logger) writes to both `logs/YYYY-MM-DD.log` and `StreamHandler`
  - Format: `'%(asctime)s - %(levelname)s - %(message)s'`
  - Category-based ANSI colors (e.g., `[DOWNLOAD]` blue, `[MUSIC]` magenta, `[UPLOAD]` yellow)
- **Retry logic**: Commands use `max_attempts = 3` / `while attempts < max_attempts` loops.
- **Error handling**: Broad `except Exception` with logging; generally falls back to retrying or sending an error message to the user.

## Important Gotchas

1. **Old Telegram API**: If you add new handlers, use v13 semantics:
   - `CommandHandler("cmd", func)` (in v13, `pass_args=True` is implicit via `context.args`)
   - `MessageHandler(Filters.text & ~Filters.command, func)` for plain text
   - `update.message.chat_id`, `context.bot.send_video(...)` — **not** v20 kwargs.

2. **File upload limit**: The bot uses a **35 MB** ceiling (`MAX_FILE_SIZE` in `utils/upload.py`). Files above this are uploaded externally via `curl.libriciel.fr` with a PycURL PUT and a 10%-step progress callback message.

3. **Python version lock**: `python-telegram-bot==13.7` is **incompatible with Python 3.13+**. Always target Python 3.11. If running locally on a newer OS, install Python 3.11 via `pyenv` or similar.

4. **Docker setuptools fix**: The Dockerfile explicitly reinstalls `setuptools<71` after wheel install because modern setuptoolsdrops `pkg_resources`, which APScheduler 3.6.3 requires. Do not remove this line.

5. **No yt-dlp binary**: `yt-dlp` is installed as a Python package (`yt_dlp`), not a vendored binary. Both `download.py` and `music.py` import and call `yt_dlp.YoutubeDL(...)` directly.

6. **Stats command is restricted**: `commands/stats.py` allows only `AUTHORIZED_USER` (username `overstylefr`) or `AUTHORIZED_IDS` (hardcoded Telegram user ID). Unauthorized users get "Accès refusé."

7. **CI skips tests**: The GitHub Actions workflow has a placeholder `echo "No tests to run"`. Adding tests requires updating `.github/workflows/deploy.yml`.

8. **Branch-based image tags** (`ghcr.io/...`):
   - `main` → `latest` + version tags extracted from `config.py`
   - `develop` → `dev`
   - Other branches → branch name

## Docker Notes

- Multi-stage build:
  1. `ffmpeg` stage — copies binaries from `linuxserver/ffmpeg`
  2. `builder` stage — runs `pip wheel` to create wheels
  3. Final stage — installs wheels, fixes setuptools, copies FFmpeg, copies source, runs `python main.py`
- Base image: `python:3.11-slim-bullseye`
- The local `ffmpeg/` directory is copied into the image but the Dockerfile prefers the stage-copied `/usr/local/bin/ffmpeg`.

## Git & Branches

- `main` — production/stable
- `develop` — active development
- CI triggers on **both** `main` and `develop` pushes.

## When Modifying This Codebase

- The project is split into modules; add new commands to `commands/` and new utilities to `utils/`.
- Preserve French user-facing strings.
- Do not upgrade `python-telegram-bot` without rewriting all handler signatures and startup logic.
- If adding a new command, remember to `dp.add_handler(CommandHandler("cmd", func))` in `main.py` before `updater.start_polling()`.
- If you introduce `async`, you must rewrite the entire bot (handlers, dispatcher, updater → ApplicationBuilder). Prefer sync additions to avoid a full migration.
- Update `VERSION` in `config.py` (and/or `.env`) when shipping meaningful changes.
- If you touch packaging, verify the Docker build still starts without `ModuleNotFoundError: pkg_resources`.
