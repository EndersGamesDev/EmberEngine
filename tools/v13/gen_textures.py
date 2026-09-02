#!/usr/bin/env python3
"""Arena v13 "Trench City" material pictures on adler's ComfyUI (Qwen-Image).

Every picture here is a MATERIAL, not a concept: a flat, seamless, evenly lit
albedo that the client tiles over a box face or box-projects onto a generated
mesh. The renderer samples exactly one base-colour texture per mesh, so all
the "realism" a surface can carry has to be painted into this one picture —
which is why every prompt asks for soft ambient occlusion baked in.

Raw output lands in ``assets/concepts/v13/textures/`` at generation size;
``tools/v13/bake_textures.py`` composes the atlases and downscales into
``assets/textures/v13/`` — the files the game actually embeds. Keep the raw
set: it is the thing to re-bake from when a size budget changes.

    python tools/v13/gen_textures.py              # all
    python tools/v13/gen_textures.py sky cobble   # some

Shares the relay, the graph and the fetch with ``gen_views.py``; ComfyUI runs
one job at a time, so running both scripts together just interleaves them.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from gen_views import fetch, graph, post, wait  # noqa: E402

OUT = os.environ.get(
    "V13_TEXTURES",
    os.path.join(os.path.dirname(__file__), "..", "..", "assets", "concepts", "v13", "textures"),
)

TILE = (
    "Seamless tileable texture, straight-on orthographic view, photorealistic PBR "
    "albedo with soft ambient occlusion baked in, flat even diffuse lighting, no "
    "hard shadows, no perspective, no text, no watermark, edge-to-edge material only."
)

# name -> (subject, width, height)
TEXTURES = {
    # --- the cover boxes -------------------------------------------------
    "container-side": (
        "long side wall of a weathered steel shipping container, faded rust-red "
        "paint with chipped edges and rust streaks, evenly spaced vertical "
        "corrugation ribs, a few dents and scuffs, corner posts at both edges",
        1024, 1024,
    ),
    "container-doors": (
        "closed double doors at the end of a weathered steel shipping container, "
        "faded rust-red paint, four vertical locking bars with cam handles, heavy "
        "hinges, rubber door seals, corrugated door panels, chipped paint",
        1024, 1024,
    ),
    "container-roof": (
        "corrugated steel roof panel of a weathered shipping container seen from "
        "above, faded rust-red paint, horizontal corrugation ribs, pooled rust "
        "stains, dust and a few dents",
        1024, 1024,
    ),
    "crate-side": (
        "side of a rough wooden military supply crate, pale weathered pine planks "
        "with visible grain and knots, dark iron corner brackets and nail heads, "
        "a faded black stencilled arrow, dusty and scuffed",
        1024, 1024,
    ),
    "crate-top": (
        "lid of a rough wooden military supply crate seen from above, weathered "
        "pine planks with grain and knots, two iron reinforcing straps, nail heads, "
        "dust in the gaps between planks",
        1024, 1024,
    ),
    # "side of a box" drew the whole box in three-quarter view; a face picture
    # has to say it fills the frame and has no outline, or the box gets a
    # picture of a box on it.
    "ammo-side": (
        "flat straight-on close-up of the long side panel of an olive-drab steel "
        "military ammunition box filling the entire frame edge to edge, pressed "
        "steel with a horizontal stiffening rib, a spring latch and a folding wire "
        "handle seen head-on, chipped olive paint, a faded yellow stencilled "
        "calibre marking, rust at the seams, no background, no outline of the box",
        1024, 1024,
    ),
    "ammo-top": (
        "flat view from directly above of the hinged lid of an olive-drab steel "
        "military ammunition box filling the entire frame edge to edge, pressed "
        "steel, chipped olive paint, scratches, a faded stencilled marking, no "
        "background, no outline of the box",
        1024, 1024,
    ),
    "sandbag": (
        "wall of stacked military sandbags, burlap khaki hessian bags bulging and "
        "staggered like brickwork, five courses, dusty and weathered, WWI trench",
        1024, 1024,
    ),
    "trench-wall": (
        "WWI trench revetment wall, packed earth held back by weathered timber "
        "planks and vertical posts, wire and roots, mud stains, damp and dark",
        1024, 1024,
    ),
    # "underside of a tunnel ceiling" drew the whole tunnel receding to a
    # vanishing point. The roof slab is 12 m of flat underside; it needs a
    # patch seen straight up.
    "tunnel-roof": (
        "flat close-up seen from directly below of a tunnel ceiling made of "
        "rusty corrugated iron sheets bolted over heavy dark timber beams, "
        "filling the entire frame, soot and damp stains, camera pointing "
        "straight up, no perspective, no vanishing point",
        1024, 1024,
    ),
    "rubble": (
        "pile of broken red bricks, mortar chunks, splintered timber and dust, "
        "war-damaged building debris seen from above",
        1024, 1024,
    ),
    # --- the ground and the boundary ---------------------------------------
    # "seen from directly above" still drew a street receding to a vanishing
    # point, with kerbs and (from "shell splinters") seashells. The floor
    # tiles this twelve times, so it has to be a flat overhead patch.
    "cobble": (
        "flat overhead close-up of wet grey granite cobblestones filling the "
        "entire frame, small rounded setts in a fan pattern, mud and small "
        "puddles in the joints, a little scattered brick dust, camera pointing "
        "straight down, no kerb, no street edges, no vanishing point, no objects",
        1024, 1024,
    ),
    # Same lesson: "long wall" drew it receding along a pavement.
    "city-wall": (
        "flat straight-on close-up of a cream limestone ashlar block wall "
        "filling the entire frame, with a carved stone cornice and a row of "
        "turned balusters running along the top edge, bullet chips and soot "
        "stains, weathered and beautiful, no perspective, no pavement, no "
        "buildings behind",
        1024, 1024,
    ),
    # --- materials box-projected onto the generated props ------------------
    "limestone": (
        "cream honey limestone ashlar block wall, fine chisel marks, thin mortar "
        "joints, light weathering and soot streaks, old European cathedral stone",
        1024, 1024,
    ),
    "sandstone": (
        "pale golden sandstone block wall with carved floral art-nouveau relief "
        "bands, thin mortar joints, gentle weathering",
        1024, 1024,
    ),
    "bronze": (
        "oxidised bronze surface with green verdigris patina over dark brown "
        "metal, streaks and mottling, monument sculpture bronze",
        1024, 1024,
    ),
    "burlap": (
        "khaki burlap hessian sackcloth weave, coarse fibres, dusty and faded, "
        "sandbag cloth",
        1024, 1024,
    ),
    "scorched-steel": (
        "burnt and rusted car body sheet metal, scorched black paint remnants "
        "over orange rust, blistered and pitted",
        1024, 1024,
    ),
    "cast-iron": (
        "black painted cast iron with fine casting texture, chipped to grey metal "
        "at the edges, a little rust bleeding through, Victorian street furniture",
        1024, 1024,
    ),
    "granite": (
        "dark grey polished granite with black and white speckle, fine mica "
        "glints, a few chips at the edges, monument plinth stone",
        1024, 1024,
    ),
    # --- the sky -----------------------------------------------------------
    # Not a tile: a 360-degree strip for the sky cylinder. The seam is hidden
    # behind the cathedral, so it does not have to be seamless.
    "sky": (
        "ultra-wide panoramic skyline of a beautiful old European city at golden "
        "hour, seen from the middle of a grand square: cathedral spires, copper "
        "domes, mansard rooftops and chimney pots against a warm glowing sky "
        "with soft pink and gold clouds, distant hills, no foreground, no "
        "people, no text, photorealistic matte painting",
        2048, 512,
    ),
}


def main() -> int:
    only = set(sys.argv[1:])
    os.makedirs(OUT, exist_ok=True)
    jobs = [(n, s, w, h) for n, (s, w, h) in TEXTURES.items() if not only or n in only]
    print(f"{len(jobs)} textures to render -> {OUT}", flush=True)
    done = fails = 0
    t_all = time.time()
    for i, (name, subject, w, h) in enumerate(jobs):
        prefix = f"v13-tex-{name}"
        dest = os.path.join(OUT, f"{name}.png")
        if os.path.exists(dest):
            print(f"[{i + 1}/{len(jobs)}] {name} exists, skip", flush=True)
            done += 1
            continue
        prompt = f"{subject}. {TILE}" if name != "sky" else subject
        g = graph(prompt, prefix, 131_000 + i)
        g["6"]["inputs"]["width"] = w
        g["6"]["inputs"]["height"] = h
        g["5"]["inputs"]["text"] = "blurry, text, watermark, logo, people, perspective, vanishing point, frame, border, cropped"
        t0 = time.time()
        pid = post("/prompt", {"prompt": g})["prompt_id"]
        images = wait(pid)
        ok = bool(images)
        if ok:
            n = fetch(images[0], dest)
            print(f"[{i + 1}/{len(jobs)}] {name} ok ({time.time() - t0:.0f}s, {n // 1024} KB)", flush=True)
        else:
            print(f"[{i + 1}/{len(jobs)}] {name} FAIL ({time.time() - t0:.0f}s)", flush=True)
        done += ok
        fails += not ok
    print(f"DONE: {done} ok, {fails} failed, {(time.time() - t_all) / 60:.1f} min", flush=True)
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
