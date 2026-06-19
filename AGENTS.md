# Agent Guidelines: SocialVideoDownload.py

## Project Overview
A **Telegram bot** (modular Python application) that downloads videos and music from social media links (YouTube, TikTok, etc.) and sends them back to users. Deployed as a Docker container to GitHub Container Registry.

- **Language**: Python 3.11 (target)
- **Architecture**: Modular (commands/, utils/ packages) with entry point `main.py`
- **Framework**: `python-telegram-bot==13.7` — **critical**: this is the old v12 synchronous API (`Updater`, `Dispatcher`, `use_context=True`). Do NOT use modern v20+ async patterns; they are incompatible.
- **Deployment**: Docker image → `ghcr.io/OverStyleFR/SocialVideoDownload.py`
- **CI/CD**: GitHub Actions with multi-arch (`linux/amd64`, `linux/arm64`)

## Essential Commands

| Command | Purpose |
|---------|---------|
| `python main.py` | Run the bot locally (requires `.env`) |
| `bash setup.sh` | One-time local setup (venv + pip install + .env) |
| `docker compose up -d` | Run the Docker container locally |
| `docker build -t socialvideodownload .` | Build Docker image |
| `docker run -v $(pwd)/.env:/app/.env socialvideodownload` | Run container |
| `pip install -r requirements.txt` | Install dependencies |
| `echo "No tests to run"` | Current test suite (there are **no tests**) |

**No test framework is configured.** The CI workflow explicitly skips tests with a placeholder `echo`.

## Project Structure

```
.
├── main.py                    # Entry point
├── config.py                  # Configuration from .env
├── commands/
│   ├── start.py               # /start handler
│   ├── help.py                # /help handler
│   ├── download.py            # /download handler
│   ├── music.py               # /music handler
│   ├── stats.py               # /stats handler
│   ├── auto_download.py       # Auto-download via text messages
│   └── upload.py              # Upload helper (now in utils/)
├── utils/
│   ├── cache.py               # Cache hit tracking
│   ├── disk_manager.py        # Downloads cleanup (retention + emergency)
│   ├── file_manager.py        # Hash-based dedup (sha256 of URL)
│   ├── logger.py              # Console + file logging
│   ├── progress_file.py       # Progress-aware file wrapper for uploads
│   ├── retention.py           # File retention via mtime
│   ├── token_loader.py        # .env token loading
│   ├── curl_uploader.py       # External upload via curl.libriciel.fr
│   └── upload.py              # Telegram file upload with progress
├── imghdr.py                  # Compatibility shim (removed from stdlib 3.13+)
├── Dockerfile                 # Multi-stage: bookworm base
├── docker-compose.yml         # Local dev container
├── setup.sh                   # Standalone setup script
├── .dockerignore              # Excludes ffmpeg/, .kilo/, *.md, etc.
├── .env.example               # Environment template
├── .gitignore
├── requirements.txt
└── AGENTS.md
```

## Architecture & Data Flow

```
Telegram Message
      ↓
  python-telegram-bot v12 handlers (commands/*.py)
      ↓
  yt-dlp (Python package, not subprocess) → downloads/<title>.<ext>
      ↓
  [Video] → check retention → bot.send_video()
  [Music] → ffmpeg extract-audio → .mp3 → retention → bot.send_audio()
```

- **Modules**: Logic is split into `commands/` (handlers) and `utils/` (infrastructure).
- **yt-dlp**: Used as a **Python package** (`import yt_dlp`), not a subprocess binary.
- **Synchronous**: The entire bot is sync. All handlers block on I/O. Do not introduce `async`/`await` unless migrating the entire framework.

### Caching & Dedup
- **Hash-based**: URL → SHA-256 → stored in `downloads/hashes.txt`. Before downloading, `is_already_downloaded()` checks if the hash exists.
- **File check**: Even if the hash exists, the bot verifies the file still exists on disk (yt-dlp's `prepare_filename`). If missing, it re-downloads and the hash line persists (harmless, duplicates are per-session only).

### Retention Policy
- Files get their **mtime set to `now + retention`** after download via `set_retention()`.
- Small files (< `SMALL_FILE_SIZE_MB`) and mp3s: retention = `RETENTION_SMALL_HOURS` (default 24h).
- Large files: retention = `RETENTION_LARGE_HOURS` (default 2h).
- `cleanup_by_retention()` removes files whose mtime < now (expired retention).
- `check_and_clean_if_needed()` tries retention first, then full clear if still low on space.

### Startup Behavior (`main()`)
1. `clear_downloads()` — deletes everything in `downloads/` (fresh start).
2. `load_cache()` — loads cache tracking from disk.
3. Background thread: `scheduled_cleanup()` runs `cleanup_by_retention()` every `CLEANUP_INTERVAL_HOURS`.

### File-Size Guard
Telegram bot API limits: the bot hardcodes a **35 MB** ceiling (`MAX_FILE_SIZE = 35 * 1024 * 1024` in `utils/upload.py`). Files exceeding this are uploaded externally via `curl.libriciel.fr`.

## External Dependencies

| Dependency | Type | Used For |
|------------|------|----------|
| `yt-dlp` | Python package (pip) | Video/audio downloading |
| `ffmpeg` | System binary | Audio extraction (music command) |

- **FFmpeg** is resolved via `config.FFMPEG_PATH`:
  - Default: `ffmpeg/ffmpeg-7.0.2-amd64-static/ffmpeg` (local dev)
  - If the configured path doesn't exist: falls back to `"ffmpeg"` (system PATH)
  - In Docker: copied from `ghcr.io/linuxserver/ffmpeg:latest` into `/usr/local/bin/`
- **yt-dlp** is a Python dependency (not a vendored binary).

## Configuration & Secrets (`.env`)

| Variable | Default | Description |
|----------|---------|-------------|
| `BOT_TOKEN` | *(required)* | Telegram bot token |
| `VERSION` | `V9.3.1` | Bot version (used for Docker tags, /stats, /help) — source de vérité : `.env` / `.env.example` |
| `DEVELOPED_BY` | `Tom V. \| OverStyleFR` | Author credit |
| `FFMPEG_PATH` | see above | Path to ffmpeg binary |
| `MIN_FREE_SPACE_MB` | `500` | Min free space before emergency cleanup |
| `CLEANUP_INTERVAL_HOURS` | `24` | Interval between scheduled cleanups |
| `SMALL_FILE_SIZE_MB` | `4` | Threshold for small/large file retention |
| `RETENTION_SMALL_HOURS` | `24` | Retention for small files + mp3 |
| `RETENTION_LARGE_HOURS` | `2` | Retention for large files |

- **Token source**: `.env` file (read via `python-dotenv`). If missing, `token_loader.py` creates a template and exits.
- **`token.txt`** is deprecated (was used by the old egg-pterodactyl setup). Now `.env` is the sole config source.
- **Version** : la source de vérité est `.env` (via `VERSION=`). `.env.example` est le template commité. La CI lit `VERSION` depuis `.env.example` pour les tags Docker. `config.py` n'a plus de fallback hardcodé — si `.env` manque, la version affichée est `"unknown"`.

## Code Patterns & Conventions

- **Language**: UI strings and comments are in **French** (e.g., "Téléchargement en cours"). Maintain this for user-facing messages.
- **Logging**: Single `console_logger` ("TelegramBot") with colored console output + daily file logs in `logs/`.
- **Retry logic**: Downloads use a `while attempts < max_attempts` loop with `max_attempts = 3`.
- **Error handling**: Broad `except Exception` with logging. Some paths use `try/except` inside retry loops.

## Important Gotchas

1. **Old Telegram API**: Use v12 semantics:
   - `CommandHandler("cmd", func, pass_args=True)` for arguments
   - `MessageHandler(Filters.text & ~Filters.command, func)` for plain text
   - `update.message.chat_id`, `context.bot.send_video(...)` — **not** v20 patterns.

2. **`python-telegram-bot==13.7` + Python ≥3.13**: The vendored urllib3 in PTB breaks on Python ≥3.13. A local `imghdr.py` shim is committed for Python 3.13+ stdlib changes. `urllib3<2` is pinned in `requirements.txt` to avoid removal of `urllib3.contrib.appengine`.

3. **CI validate job**: Runs on Python 3.11 (target version). Using newer Python (3.13+) will fail due to PTB compatibility issues.

4. **Cleanup on startup**: `clear_downloads()` wipes `downloads/` entirely (including `hashes.txt`). This means the hash cache is not persistent across restarts.

5. **`egg-socialvideodownload.json`**: **Deleted** and deprecated. Was used for Pterodactyl/Pelican panel integration with `token.txt`. The bot now uses `.env` exclusively.

6. **Branch-based image tags** (`ghcr.io/...`):
   - `main` → `latest` + VERSION tag
   - `develop` → `dev`

7. **CI skips tests**: The workflow's "Run tests" is a placeholder. Adding tests requires updating `.github/workflows/deploy.yml`.

8. **Version**: Stored in `config.py` as `VERSION = os.getenv("VERSION", "unknown")`. The CI reads it via `grep .env.example` to tag Docker images.

## Docker Notes

- **Multi-stage build**:
  1. `ffmpeg` stage — copies binaries from `linuxserver/ffmpeg:latest`
  2. `builder` stage — `pip wheel` on `python:3.11-slim-bookworm`
  3. Final stage — installs wheels (excluding setuptools — kept from base image), copies FFmpeg, `COPY . .`, runs `python main.py`
- **Base image**: `python:3.11-slim-bookworm`
- **`.dockerignore`** excludes `ffmpeg/`, `.kilo/`, `*.md`, `.env`, etc. to minimize image size.
- **`docker-compose.yml`** mounts `.env`, `downloads/`, and `logs/` as volumes.

## Git & Branches

- `main` — production/stable
- `develop` — active development
- CI triggers on **both** `main` and `develop` pushes.

## When Modifying This Codebase

- **Preserve French** user-facing strings.
- **Do not upgrade** `python-telegram-bot` without rewriting all handler signatures (v12 → v20 is a full rewrite).
- If adding a new command, create the handler in `commands/`, add `dp.add_handler(...)` in `main()` before `updater.start_polling()`.
- If you introduce `async`, you must rewrite the entire bot (handlers, dispatcher, updater → ApplicationBuilder). Prefer sync additions.
- **Update `VERSION`** in `.env` / `config.py` default when shipping meaningful changes.
- **New dependencies**: If a dependency requires a Python feature removed in 3.13+ (like `imghdr`), provide a compatibility shim and commit it (do NOT gitignore).
- **`urllib3` pin**: Keep `urllib3<2` pinned — PTB v13.7 uses `urllib3.contrib.appengine` which was removed in urllib3 2.x.
