#!/usr/bin/env bash
# Poll the picture generator's history and pull anything new, every 90 s,
# for up to the given number of minutes. Log to tools/v13/fetch-loop.log.
cd "$(dirname "$0")/../.." || exit 1
mins="${1:-60}"
end=$(( $(date +%s) + mins * 60 ))
while [ "$(date +%s)" -lt "$end" ]; do
    python tools/v13/fetch_pictures.py 2>&1 | grep -v "^  " | grep -v "already had" 
    python tools/v13/fetch_pictures.py --list >/dev/null 2>&1
    n=$(ls assets/concepts/v13/*.png assets/concepts/v13/textures/*.png 2>/dev/null | wc -l)
    echo "$(date +%H:%M:%S) on disk: $n/49"
    sleep 90
done
