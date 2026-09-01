#!/usr/bin/env python3
"""Merge key/value pairs into server.json on gh-pages. Never overwrite.

    python3 deploy/merge-server-json.py <server.json> KEY VALUE [KEY VALUE ...]

server.json is shared by every game: the arena's "ws" and "proto", Fire
Racer's "fire_ws" and "fire_proto", Four Kings' "kings_ws" and
"kings_proto", the multi-host "hosts" list and "mirrors" (docs/hosts.md on
feat/multi-host), and the "v" deploy stamp. Each deploy writes only its own
keys, so this script reads the file, sets exactly the pairs it was given,
bumps "v" to the current unix time (the pages cache-bust their bundles on
it), and writes everything else back untouched. Clobbering the file would
take the OTHER games offline, which is why the fire deploy's inline python
did the same merge and why this one is a file: the kings deploy runs it
inside the claude-sdk WSL distro, where a heredoc would have to cross
Git Bash -> wsl.exe -> bash quoting, and that path has hung on this
machine.

Values that look like integers (all digits, optional leading minus) are
stored as JSON numbers so "kings_proto 1" lands as `"kings_proto": 1`,
the same type deploy-pages.sh writes for "proto" and "fire_proto".
Everything else is stored as a string.

A missing or unreadable file starts from an empty object, exactly as the
inline merges did. Output is compact JSON, the format the other writers
produce, so a diff on gh-pages shows only the keys that changed.
"""

import json
import os
import re
import sys
import time


def coerce(value: str):
    return int(value) if re.fullmatch(r"-?[0-9]+", value) else value


def main(argv):
    if len(argv) < 4 or (len(argv) - 2) % 2 != 0:
        sys.stderr.write(
            "usage: merge-server-json.py <server.json> KEY VALUE [KEY VALUE ...]\n"
        )
        return 2
    path = argv[1]
    pairs = argv[2:]
    data = {}
    if os.path.exists(path):
        try:
            with open(path, encoding="utf-8") as fh:
                loaded = json.load(fh)
            if isinstance(loaded, dict):
                data = loaded
            else:
                sys.stderr.write(
                    f"merge-server-json: {path} is not a JSON object; starting empty\n"
                )
        except (OSError, ValueError) as exc:
            sys.stderr.write(
                f"merge-server-json: could not read {path} ({exc}); starting empty\n"
            )
    before = set(data)
    for key, value in zip(pairs[0::2], pairs[1::2]):
        data[key] = coerce(value)
    data["v"] = str(int(time.time()))
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(data, fh)
    kept = sorted(before - set(pairs[0::2]) - {"v"})
    print(
        "merge-server-json: set "
        + ", ".join(f"{k}={data[k]!r}" for k in pairs[0::2])
        + f", v={data['v']}; kept "
        + (", ".join(kept) if kept else "(nothing else was there)")
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
