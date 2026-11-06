#!/bin/sh
# Faux yt-dlp des tests e2e : id = dernier segment de l'URL (dernier argument).
for a in "$@"; do last="$a"; done
id="${last##*/}"
echo "$id" >> "$(dirname "$0")/calls.txt"
case "$id" in
  slow) sleep 5 ;;
  fail) echo "ERROR: extracteur en panne" >&2; exit 1 ;;
esac
echo data > "$id.mp4"
printf '{"id":"%s","title":"titre %s","duration":5,"filepath":"%s/%s.mp4"}\n' "$id" "$id" "$(pwd)" "$id"
