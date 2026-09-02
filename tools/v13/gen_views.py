#!/usr/bin/env python3
"""Arena v13 "Trench City" concept views on adler's ComfyUI (Qwen-Image fp8).

Same shape as the Fire Racer runbook this was copied from, with two changes
worth knowing about:

* It talks to ComfyUI through the local relay (``http://127.0.0.1:9188`` ->
  adler:8188), so it runs from the workstation with no ssh in the loop.
* It PULLS every finished image back over ComfyUI's ``/view`` endpoint into
  ``assets/concepts/v13/`` instead of leaving it in adler's ``~/comfy/out``.
  The earlier runbooks left the views on adler and copied by hand; that step
  is where a set gets lost.

These four views per prop are the input to Hunyuan3D-2mv
(``C:\\hy3d\\gen3d_mv.py``, driven by ``tools/v13/mesh-props.ps1``), so what
matters is a clean silhouette on a plain light-gray background: one subject,
centred, no shadow, no ground plane.

    python tools/v13/gen_views.py            # every prop, every view
    python tools/v13/gen_views.py cathedral  # one prop

~32 s per 1024x1024 view on the 4090. Finished views are skipped, so a rerun
resumes.
"""

import glob
import json
import os
import sys
import time
import urllib.parse
import urllib.request

API = os.environ.get("COMFY_API", "http://127.0.0.1:9188")
OUT = os.environ.get(
    "V13_VIEWS", os.path.join(os.path.dirname(__file__), "..", "..", "assets", "concepts", "v13")
)

FORMAT = (
    "AAA game asset reference, photorealistic PBR materials, orthographic {view}, "
    "single object perfectly centered, plain light gray studio background, even "
    "diffuse lighting, no shadows, no ground plane, no text, no watermark."
)
V4 = {
    "front": "front view",
    "back": "back view seen directly from behind",
    "left": "left side view in full profile",
    "right": "right side view in full profile",
}

# Decor props: scenery the client draws around and inside the arena. None
# of these is a collision volume — the sim's cover is boxes — so their
# silhouettes only have to read well from eye height.
PARTS = {
    # The skyline hero, seen over every container from anywhere in the map.
    "cathedral": (
        "Isolated grand gothic cathedral, pale honey limestone, twin tall pointed "
        "spires with ornate tracery, a large rose window above a triple-arched "
        "portal, flying buttresses along the nave, slate roof, weathered but "
        "beautiful, standalone architectural model, no terrain, no surrounding "
        "buildings, cathedral only floating."
    ),
    # Repeated around the ring: the beautiful city the fight is set in.
    "facade-a": (
        "Isolated elegant Parisian Haussmann-style apartment block, five storeys, "
        "cream limestone facade, wrought-iron balconies on every floor, tall "
        "shuttered windows, a grey zinc mansard roof with dormers and chimney "
        "stacks, arched ground-floor shopfronts, a few windows shattered and "
        "sandbags stacked at the doorway, standalone architectural model, no "
        "terrain, no neighbouring buildings, building only floating."
    ),
    "facade-b": (
        "Isolated ornate art-nouveau corner building, six storeys, pale sandstone "
        "with sculpted floral reliefs, a rounded corner tower topped by a green "
        "copper dome, tall arched windows with iron balconies, a ground-floor "
        "cafe with a striped awning torn by shrapnel, standalone architectural "
        "model, no terrain, no neighbouring buildings, building only floating."
    ),
    # The square's centrepiece: cover you fight around, drawn as a monument.
    "statue": (
        "Isolated bronze equestrian statue of a general on a rearing horse, "
        "green oxidised patina, standing on a tall rectangular carved grey "
        "granite plinth with a bronze wreath relief, monumental city-square "
        "sculpture, standalone prop, no terrain, statue and plinth only floating."
    ),
    # Trench-line dressing: scaled into the sim's sandbag boxes.
    "sandbags": (
        "Isolated straight wall of stacked military sandbags, four bags long and "
        "five courses high, burlap khaki hessian bags bulging and staggered like "
        "brickwork, dusty and weathered, WWI trench fortification segment, "
        "standalone prop, no terrain, no ground, sandbag wall only floating."
    ),
    # Street dressing between the trench lines.
    "wreck": (
        "Isolated burnt-out 1930s vintage sedan car wreck, rusted bare metal "
        "body, no glass, sagging on flat tyres, doors dented, scorched black "
        "paint remnants, abandoned in a war-torn city street, standalone prop, "
        "no terrain, no ground, car wreck only floating."
    ),
    "lamp": (
        "Isolated ornate cast-iron Victorian street lamp post, black fluted "
        "column on a stepped base, curled decorative bracket, single glass "
        "lantern head with a small crown, five metres tall, standalone prop, "
        "no terrain, no ground, lamp post only floating."
    ),
}

VIEWS = {p: dict(V4) for p in PARTS}
# Long props read as two different objects from front and back unless the
# axis is named; this is the same fix the Fire Racer car needed.
VIEWS["wreck"] = {
    "front": "front view head on, looking straight at the radiator grille",
    "back": "rear view looking straight at the tail of the car",
    "left": "side view in full profile with the bonnet pointing left",
    "right": "side view in full profile with the bonnet pointing right",
}
VIEWS["sandbags"] = {
    "front": "front view, the long face of the wall seen straight on",
    "back": "back view, the long face of the wall seen straight on from behind",
    "left": "left end view in full profile, the short end of the wall",
    "right": "right end view in full profile, the short end of the wall",
}


def graph(prompt: str, prefix: str, seed: int) -> dict:
    return {
        "1": {"class_type": "UNETLoader", "inputs": {"unet_name": "qwen_image_fp8_e4m3fn.safetensors", "weight_dtype": "default"}},
        "2": {"class_type": "CLIPLoader", "inputs": {"clip_name": "qwen_2.5_vl_7b_fp8_scaled.safetensors", "type": "qwen_image", "device": "default"}},
        "3": {"class_type": "VAELoader", "inputs": {"vae_name": "qwen_image_vae.safetensors"}},
        "10": {"class_type": "ModelSamplingAuraFlow", "inputs": {"shift": 3.1, "model": ["1", 0]}},
        "4": {"class_type": "CLIPTextEncode", "inputs": {"text": prompt, "clip": ["2", 0]}},
        "5": {"class_type": "CLIPTextEncode", "inputs": {"text": "blurry, cropped, text, watermark, people, soldiers, road, ground shadow, multiple objects, terrain", "clip": ["2", 0]}},
        "6": {"class_type": "EmptySD3LatentImage", "inputs": {"width": 1024, "height": 1024, "batch_size": 1}},
        "7": {"class_type": "KSampler", "inputs": {"model": ["10", 0], "positive": ["4", 0], "negative": ["5", 0], "latent_image": ["6", 0], "seed": seed, "steps": 20, "cfg": 2.5, "sampler_name": "euler", "scheduler": "simple", "denoise": 1.0}},
        "8": {"class_type": "VAEDecode", "inputs": {"samples": ["7", 0], "vae": ["3", 0]}},
        "9": {"class_type": "SaveImage", "inputs": {"images": ["8", 0], "filename_prefix": prefix}},
    }


def post(path: str, payload: dict) -> dict:
    req = urllib.request.Request(API + path, json.dumps(payload).encode(), {"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.loads(r.read())


def wait(pid: str, timeout_s: int = 900):
    """Block until the prompt completes; return its output image descriptors."""
    t0 = time.time()
    while time.time() - t0 < timeout_s:
        try:
            with urllib.request.urlopen(f"{API}/history/{pid}", timeout=30) as r:
                h = json.loads(r.read())
        except Exception:
            time.sleep(5)
            continue
        if pid in h:
            st = h[pid].get("status", {})
            if st.get("completed"):
                images = []
                for node in h[pid].get("outputs", {}).values():
                    images.extend(node.get("images", []))
                return images
            if st.get("status_str") == "error":
                print(f"  ERROR: {json.dumps(st)[:400]}", flush=True)
                return None
        time.sleep(4)
    print("  TIMEOUT", flush=True)
    return None


def fetch(image: dict, dest: str) -> int:
    q = urllib.parse.urlencode({
        "filename": image["filename"],
        "subfolder": image.get("subfolder", ""),
        "type": image.get("type", "output"),
    })
    with urllib.request.urlopen(f"{API}/view?{q}", timeout=120) as r:
        data = r.read()
    with open(dest, "wb") as f:
        f.write(data)
    return len(data)


def main() -> int:
    only = set(sys.argv[1:])
    os.makedirs(OUT, exist_ok=True)
    jobs = [
        (p, v, phr)
        for p in PARTS
        if not only or p in only
        for v, phr in VIEWS[p].items()
    ]
    print(f"{len(jobs)} views to render -> {OUT}", flush=True)
    done = fails = 0
    t_all = time.time()
    for i, (part, view, phrase) in enumerate(jobs):
        prefix = f"v13-{part}-{view}"
        dest = os.path.join(OUT, f"{prefix}.png")
        if os.path.exists(dest):
            print(f"[{i + 1}/{len(jobs)}] {prefix} exists, skip", flush=True)
            done += 1
            continue
        prompt = f"{PARTS[part]} {FORMAT.format(view=phrase)}"
        t0 = time.time()
        pid = post("/prompt", {"prompt": graph(prompt, prefix, 130_000 + i)})["prompt_id"]
        images = wait(pid)
        ok = bool(images)
        if ok:
            n = fetch(images[0], dest)
            print(f"[{i + 1}/{len(jobs)}] {prefix} ok ({time.time() - t0:.0f}s, {n // 1024} KB)", flush=True)
        else:
            print(f"[{i + 1}/{len(jobs)}] {prefix} FAIL ({time.time() - t0:.0f}s)", flush=True)
        done += ok
        fails += not ok
    print(f"DONE: {done} ok, {fails} failed, {(time.time() - t_all) / 60:.1f} min", flush=True)
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
