#!/bin/sh
# Faux ffmpeg : volumedetect audible ; écrit le dernier argument ; « badmp3 » écrit un partiel puis échoue.
for a in "$@"; do last="$a"; done
case "$*" in *volumedetect*) echo "max_volume: -5.0 dB" >&2; exit 0 ;; esac
echo x > "$last"
case "$*" in *badmp3*) echo "conversion impossible" >&2; exit 1 ;; esac
exit 0
