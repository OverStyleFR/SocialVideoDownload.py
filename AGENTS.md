# Agent Guidelines: SocialVideoDownload.py

## Project Overview
A **Telegram bot** (single-file Python application) that downloads videos and music from social media links (YouTube, TikTok, etc.) and sends them back to users. Deployed as a Docker container to GitHub Packages.

- **Language**: Python 3.11 (target), Python 3.x (CI)
- **Primary file**: `main.py` (monolith, ~450 lines)
- **Framework**: `python-telegram-bot==12.8` — **critical**: this is the old v12 synchronous API (`Updater`, `Dispatcher`, `use_context=True`). Do NOT use modern v20+ async patterns (`Application`, `ContextTypes`, etc.); they are incompatible.
- **Deployment target**: Docker image → `ghcr.io/...` (GitHub Container Registry)

## Essential Commands

| Command | Purpose |
|---------|---------|
| `python main.py` | Run the bot locally |
| `docker build -t socialvideodownload .` | Build Docker image |
| `docker run -v $(pwd)/token.txt:/app/token.txt socialvideodownload` | Run container (needs `token.txt` mounted) |
| `pip install -r requirements.txt` | Install dependencies |
| `echo "No tests to run"` | Current test suite (there are **no tests**) |

**No test framework is configured.** The CI workflow explicitly skips tests with a placeholder `echo`.

## Architecture & Data Flow

```
Telegram Message
      ↓
  python-telegram-bot v12 handlers (main.py)
      ↓
  Subprocess: ./yt-dlp [options] -o download_temp/<md5>.mp4 <link>
      ↓
  [Video path] → Check file size (≤ 50MB) → bot.send_video()
  [Music path] → ffmpeg extract-audio → .mp3 → bot.send_audio()
```

- **Single entry point**: `main.py` contains everything — handlers, retry logic, logging, token reading, startup cleanup.
- **No modules/packages**: All logic is in one file. There is no `src/` or package structure.
- **Subprocess-heavy**: Both video (`./yt-dlp`) and audio (`ffmpeg`) pipelines spawn external binaries.
- **Synchronous**: The entire bot is sync. All handlers block on I/O (download, ffmpeg, Telegram upload). Do not introduce `async`/`await` unless you are migrating the entire framework.

### Caching
Downloaded files are cached in `download_temp/` using an **MD5 hash of the URL** as the filename (`<hash>.mp4` or `<hash>.mp3`). The bot checks for existence before re-downloading.

### Startup Behavior (`main()`)
On every start, the bot **empties** `download_temp/` (deletes all files inside) if it exists, or creates it if missing. This means the cache is not persistent across restarts.

### File-Size Guard
Telegram bot API limits: the bot hardcodes a **50 MB** ceiling (`max_size_bytes = 50 * 1024 * 1024`). Files exceeding this are rejected with a French error message.

## External Dependencies (Binaries)

| Binary | Expected Location | Used For |
|--------|-------------------|----------|
| `yt-dlp` | `./yt-dlp` (cwd) | Video/audio downloading |
| `ffmpeg` | `ffmpeg-6.1-amd64-static/ffmpeg` | Audio extraction (music command) |

- `yt-dlp` appears to be a vendored/committed binary in the repo root (not a Python package).
- `ffmpeg` is bundled as a static build directory (`ffmpeg-6.1-amd64-static/`). The music command hardcodes this relative path.
- In Docker, FFmpeg is copied from a multi-stage `ghcr.io/linuxserver/ffmpeg:latest` image into `/usr/local/bin/`. The local static path is only relevant for local runs.

## Configuration & Secrets

- **Token source**: `token.txt` (plain text file, one line, **not** an env variable). It is `.gitignore`d.
- **No `.env` library**: The project does not load environment variables. `token.txt` is read directly in `read_token()`.
- **Bot metadata**: `BOT_VERSION = "V0.7-3"` and `YOUR_NAME = "Tom V. | OverStyleFR"` are hardcoded constants near the top of `main.py`.

## Code Patterns & Conventions

- **Language**: UI strings and comments are in **French** (e.g., "Téléchargement en cours", "Veuillez patienter..."). Maintain this for user-facing messages.
- **Logging**: Two loggers:
  - Root logger → `logs/bot_YYYYMMDD_HHMMSS.log` (files)
  - `console_logger` ("console_logger") → both file and `StreamHandler`
  Format: `'%(asctime)s - %(levelname)s - %(message)s'`
- **Retry logic**: Downloads use a `while current_retry < max_retries` loop with `max_retries = 3`.
- **Error handling pattern**: Broad `except Exception` + `except urllib3.exceptions.HTTPError` + `except subprocess.CalledProcessError`. Sometimes `break` on generic errors, sometimes continue retrying.
- **Resource management**: Files are `open()`ed and manually `.close()`d. There is no `with` statement for file handles.
- **Result persistence**: `save_result_to_file()` writes yt-dlp stdout/stderr to `download_temp/download_result/result_YYYYMMDD_HHMMSS.txt`.

## Important Gotchas

1. **Old Telegram API**: If you add new handlers, use v12 semantics:
   - `CommandHandler("cmd", func, pass_args=True)` for arguments
   - `MessageHandler(Filters.text & ~Filters.command, func)` for plain text
   - `update.message.chat_id`, `context.bot.send_video(...)` — **not** `update.effective_chat.id` or `context.bot.send_video(...)` with v20 kwargs.

2. **Missing file handles**: Several code paths `open()` files but do not wrap them in `try/finally` or `with`, risking leaks on exceptions.

3. **Cleanup logic is asymmetric**:
   - `/download` attempts `os.remove(video_path)` in `finally` **only if the file does NOT exist** (`if video_path and not os.path.exists(video_path): os.remove(video_path)`). This is likely a bug — it means successful files are never cleaned up, but failed paths throw `FileNotFoundError` silently.
   - `/music` does the same inverted check.
   - `handle_text_messages` (auto-download) has **no cleanup at all**.

4. **No dependency management for yt-dlp**: The binary in the repo root may become outdated. The Dockerfile does not fetch a fresh `yt-dlp` during build; it relies on whatever is committed.

5. **50 MB limit**: Telegram’s bot API file-size cap is enforced client-side. If Telegram raises the limit, this constant must be updated manually.

6. **Branch-based image tags** (`ghcr.io/...`):
   - `main` → `latest` + SHA
   - `develop` → `dev` + SHA
   - Other branches → branch name + SHA

7. **CI skips tests**: The GitHub Actions workflow has a placeholder `echo "No tests to run"`. Adding tests requires updating `.github/workflows/deploy.yml`.

## Docker Notes

- Multi-stage build:
  1. `ffmpeg` stage — copies binaries from `linuxserver/ffmpeg`
  2. `builder` stage — runs `pip wheel` to create wheels
  3. Final stage — installs wheels, copies FFmpeg, copies source, runs `python main.py`
- Base image: `python:3.11-slim-bullseye`
- The `ffmpeg-6.1-amd64-static/` directory is copied into the image but the Dockerfile does **not** use it; it prefers the stage-copied `/usr/local/bin/ffmpeg`.

## Git & Branches

- `main` — production/stable
- `develop` — active development
- CI triggers on **both** `main` and `develop` pushes.

## When Modifying This Codebase

- Keep everything in `main.py` unless the change is large enough to justify splitting (the project has no import structure).
- Preserve French user-facing strings.
- Do not upgrade `python-telegram-bot` without rewriting all handler signatures and startup logic.
- If adding a new command, remember to `dp.add_handler(...)` in `main()` before `updater.start_polling()`.
- If you introduce `async`, you must rewrite the entire bot (handlers, dispatcher, updater → ApplicationBuilder). Prefer sync additions to avoid a full migration.
- Update `BOT_VERSION` when shipping meaningful changes.
