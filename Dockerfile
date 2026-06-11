
# --- Build Stage ---
FROM python:3.11-slim-bullseye as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends build-essential && \
    rm -rf /var/lib/apt/lists/*

# Install Python dependencies
COPY requirements.txt .
RUN pip wheel --no-cache-dir --wheel-dir /app/wheels -r requirements.txt


# --- Final Stage ---
FROM python:3.11-slim-bullseye

WORKDIR /app

# Copy dependencies from builder
COPY --from=builder /app/wheels /wheels
COPY --from=builder /usr/local/bin/ffmpeg /usr/local/bin/ffmpeg
COPY --from=builder /usr/local/bin/ffprobe /usr/local/bin/ffprobe

# Install Python dependencies
RUN pip install --no-cache /wheels/*

# Copy application code
COPY . .

# Set the command to run the application
CMD ["python", "main.py"]
