# --- FFmpeg Stage ---
FROM ghcr.io/linuxserver/ffmpeg:latest AS ffmpeg

# --- Build Stage ---
FROM python:3.11-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update && \
    apt-get install -y --no-install-recommends build-essential && \
    rm -rf /var/lib/apt/lists/*

COPY requirements.txt .
RUN pip wheel --no-cache-dir --wheel-dir /app/wheels -r requirements.txt


# --- Final Stage ---
FROM python:3.11-slim-bookworm

WORKDIR /app

COPY --from=builder /app/wheels /wheels
COPY --from=ffmpeg /usr/local/bin/ffmpeg /usr/local/bin/ffmpeg
COPY --from=ffmpeg /usr/local/bin/ffprobe /usr/local/bin/ffprobe
RUN chmod +x /usr/local/bin/ffmpeg /usr/local/bin/ffprobe

RUN pip install --no-cache $(ls /wheels/*.whl | grep -v setuptools) && rm -rf /wheels

COPY . .

CMD ["python", "main.py"]
