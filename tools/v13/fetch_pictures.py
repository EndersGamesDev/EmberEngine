#!/usr/bin/env python3
"""Pull v13 pictures off the picture generator's ComfyUI history.

The picture-generator connector (Ideogram 4 on adler, ComfyUI on :8288, run
by another account) answers with a gallery URL that needs a key this repo
does not hold. It IS a ComfyUI though, and its ``/history`` carries the full
prompt of every job and ``/view`` serves the output — both readable from
adler's loopback over ssh. So: list the history, match each job's positive
prompt against the prompt tables in ``gen_textures.py`` and ``gen_views.py``,
and fetch whatever is ours and not yet on disk.

    python tools/v13/fetch_pictures.py          # fetch everything matched
    python tools/v13/fetch_pictures.py --list   # show matches, fetch nothing

Idempotent; names are derived from the prompt tables, so a regenerated
picture with the same prompt lands on the same name (delete the old file
first if you want the new one).
"""

import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
import gen_textures  # noqa: E402
import gen_views  # noqa: E402

HOST = os.environ.get("V13_PICTURE_HOST", "adler")
API = os.environ.get("V13_PICTURE_API", "http://127.0.0.1:8288")
TEX_OUT = gen_textures.OUT
VIEW_OUT = gen_views.OUT


def remote_curl(url: str) -> bytes:
    return subprocess.run(
        ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", HOST, f"curl -s -m 60 '{url}'"],
        check=True,
        capture_output=True,
    ).stdout


def key(text: str) -> str:
    """A prompt's identity: the subject, whitespace-normalised, lower-cased."""
    return " ".join(text.split()).lower()


def targets() -> dict:
    """prompt-key -> destination path, for every picture v13 wants."""
    out = {}
    for name, (subject, _w, _h) in gen_textures.TEXTURES.items():
        out[key(subject)] = os.path.join(TEX_OUT, f"{name}.png")
    for part, subject in gen_views.PARTS.items():
        for view, phrase in gen_views.VIEWS[part].items():
            prompt = f"{subject} {gen_views.FORMAT.format(view=phrase)}"
            out[key(prompt)] = os.path.join(VIEW_OUT, f"v13-{part}-{view}.png")
    return out


def positive_prompt(entry: dict) -> str:
    graph = entry.get("prompt", [None, None, {}])[2] or {}
    texts = [
        n.get("inputs", {}).get("text", "")
        for n in graph.values()
        if n.get("class_type") == "CLIPTextEncode"
    ]
    # The negative prompt is the short one.
    return max(texts, key=len) if texts else ""


def main() -> int:
    list_only = "--list" in sys.argv
    wanted = targets()
    history = json.loads(remote_curl(f"{API}/history?max_items=200"))
    fetched = skipped = 0
    seen = set()
    # Newest first, so a regenerated picture wins over the one it replaces.
    for pid, entry in reversed(list(history.items())):
        if not entry.get("status", {}).get("completed"):
            continue
        prompt = key(positive_prompt(entry))
        if len(prompt) < 40:
            # Somebody else's job, or one with no text prompt at all. An
            # empty prompt is contained in everything; matching it once
            # fetched a cartoon penguin as the container wall.
            continue
        dest = None
        for k, d in wanted.items():
            # Our subject text must appear inside the job's prompt; the
            # connector may append its own suffix but never trims ours.
            if k in prompt:
                dest = d
                break
        if dest is None:
            continue
        if dest in seen:
            continue  # an earlier (newer) entry already claimed this name
        seen.add(dest)
        images = [im for n in entry.get("outputs", {}).values() for im in n.get("images", [])]
        if not images:
            continue
        if os.path.exists(dest):
            skipped += 1
            continue
        im = images[0]
        print(f"{pid[:8]} -> {os.path.relpath(dest)}", flush=True)
        if list_only:
            continue
        data = remote_curl(
            f"{API}/view?filename={im['filename']}&subfolder={im.get('subfolder', '')}&type={im.get('type', 'output')}"
        )
        if not data.startswith(b"\x89PNG"):
            print(f"  not a PNG ({len(data)} bytes); skipped", flush=True)
            continue
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        with open(dest, "wb") as f:
            f.write(data)
        fetched += 1
        print(f"  {len(data) // 1024} KB", flush=True)
    print(f"fetched {fetched}, already had {skipped}, wanted {len(wanted)}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
