"""Sequential, reproducible LAN fleet artwork for UltimateLegue v2.

Only one named job is submitted per invocation. Review its image and mesh
before invoking the next champion. Raw PNGs stay under target/league-art;
the web receives a small WebP derivative. No GPU profiles are switched.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
STYLE = (
    "Full body fantasy game character, centered single figure, three quarter front view, "
    "plain gray background, hand painted 3D game model, strong silhouette. "
)
PROMPTS = {
    "swarm": (
        "(A floating spherical robot with one enormous glowing cyan eye:1.4), "
        "(four broad blade-shaped bronze fins attached around the sphere:1.2), "
        "hovering magical surveillance drone, compact ivory armored sphere, ancient geometric bronze "
        "engravings, turquoise light from its large central lens, two small shoulder drone pods, "
        "original fantasy strategy game art, painted ceramic and aged bronze materials, "
        "clean bold silhouette, entire robot visible centered, three quarter view on plain neutral "
        "gray background, high quality digital painting, soft studio lighting."
    ),
    "emberknight": STYLE + (
        "One powerful flame knight in thick volcanic black and bronze plate armor, angular enclosed "
        "ivory helmet with a fiery amber slit visor, broad massive shoulders, glowing orange ember "
        "cracks, deep red short cloak, holding one broad flame-edged greatsword pointing down beside "
        "the body, two legs and two arms, planted confident stance, heroic fire guardian."
    ),
    "hallow": STYLE + (
        "One mysterious benevolent hollow guardian, tall slender floating ivory ceremonial robe "
        "with bronze edging, faceless dark hood containing a single warm golden light, large broken "
        "circular bronze halo behind the head, holding a long staff crowned with a pale green crystal, "
        "layered cream cloth and mint green enchanted ribbons, serene healer, clear elegant silhouette."
    ),
    "bogmaw": STYLE + (
        "One huge squat swamp monster guardian, toad-like hulking body with moss green leathery skin, "
        "wide toothy mouth and glowing yellow eyes, two massive forearms and short powerful legs, "
        "ancient ivory stone plates grown into its back and shoulders, reeds and amber fungus "
        "sprouting between plates, rusted bronze hook held in one fist, friendly grotesque but formidable."
    ),
    "tessera": STYLE + (
        "One adult female clockmaker battle mage, athletic poised figure, dark plum coat with ivory "
        "and bronze segmented armor, short silver hair, amber goggles pushed onto her forehead, "
        "one compact bronze clockwork backpack, both hands in mechanical gauntlets, "
        "violet hourglass pendant on her chest, fitted boots, two arms two legs, intelligent confident face, "
        "unique elegant chronomancer silhouette."
    ),
    "lane": (
        "Seamless top down texture of ancient ivory limestone paving for a hand painted fantasy game, "
        "large irregular worn stone slabs, fine cracks, restrained bronze geometric inlay, soft moss "
        "in narrow joints, neutral desaturated warm gray, flat even diffuse light, orthographic straight "
        "down view, fills entire image, no perspective, no objects, no shadows, no text."
    ),
    "garden": (
        "Seamless top down hand painted fantasy game ground texture, rich dark jade moss and "
        "tiny dense grass blades, a few tiny pale fallen leaves and low ferns, subtle organic variation, "
        "dark desaturated emerald palette, flat even diffuse light, orthographic straight down view, "
        "fills entire image, no perspective, no objects, no shadows, no text."
    ),
    "court": (
        "Orthographic top down view of one ancient circular ivory stone ritual floor medallion, "
        "bronze concentric geometric inlay, twelve radial segments, turquoise crystal triangular "
        "details, weathered engraved stone, hand painted fantasy game environment texture, symmetrical, "
        "centered fills the square, flat even diffuse lighting, no perspective, no text, no objects."
    ),
    "obelisk": (
        "One ancient Crystalforge fantasy arena obelisk, massive tall faceted turquoise crystal "
        "held inside an ornate dark bronze and ivory stone pedestal, four strong stone buttresses, "
        "carved geometric motifs, weathered moss at the base, stylized hand painted game asset, "
        "three quarter front view, whole object centered fully visible on plain warm light gray "
        "background, studio diffuse light, strong clean silhouette, no text."
    ),
    "arena": (
        "Beautiful original fantasy strategy game arena, Crystalforge elevated ruined garden, "
        "wide ivory stone bridge lane crossing an emerald ravine, two circular ritual plazas either "
        "side of the lane, bronze geometric stone architecture and enormous luminous turquoise "
        "crystals, flowering moss and hanging vines, distant warm amber citadel, afternoon shafts "
        "of light through drifting mist, epic wide establishing shot seen from elevated three quarter "
        "camera, luxurious painterly game environment concept art, playable open space in center, "
        "rich detail on edges, harmonious teal gold palette, no text, no UI, no characters."
    ),
}
ORDER = list(PROMPTS)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("name", choices=ORDER)
    parser.add_argument("--fleet-dir", default="C:/Users/end/dev/cluster/provision/asset-forge")
    parser.add_argument("--seed", type=int)
    args = parser.parse_args()
    sys.path.insert(0, args.fleet_dir)
    import asset_workers as fleet
    from PIL import Image

    raw = ROOT / "target/league-art" / args.name
    raw.mkdir(parents=True, exist_ok=True)
    queue = fleet._get(fleet.WORKERS["images"]["base"] + "/queue")
    if queue.get("queue_running") or queue.get("queue_pending"):
        raise SystemExit("Image worker has an existing job. Wait for its owner; no parallel submission.")
    seed = args.seed if args.seed is not None else 6090600 + ORDER.index(args.name)
    job = dict(prompt=PROMPTS[args.name], seed=seed, steps=32, cfg=7,
               negative="text, watermark, logo, blurry, low quality, cropped, duplicate character, multiple views, photograph, modern gun, display stand, pedestal, decorative frame, border, picture frame, character sheet, design sheet, detached props, floating objects, accessories sheet" + (", legs, arms, hands, feet, human, humanoid" if args.name == "swarm" else ""),
               width=1344 if args.name == "arena" else 1024,
               height=768 if args.name == "arena" else 1024, local_dir=str(raw))
    started = time.time()
    print(f"Generating {args.name} on specht32, seed {seed}", flush=True)
    result = fleet.tool_image_generate(job)
    print(result, flush=True)
    pngs = sorted(raw.glob("*.png"), key=lambda p: p.stat().st_mtime)
    if not pngs or pngs[-1].stat().st_mtime < started - 1:
        raise SystemExit("No new output image was produced")
    source = pngs[-1]
    out = ROOT / "web/games/league/v2/art"
    out.mkdir(parents=True, exist_ok=True)
    dest = out / (args.name + ".webp")
    im = Image.open(source).convert("RGB")
    im.thumbnail((1344, 768) if args.name == "arena" else (768, 768))
    im.save(dest, "WEBP", quality=88, method=6)
    record = {"name": args.name, "generator": "SDXL 1.0 / LAN specht32 ComfyUI",
              **{k: v for k, v in job.items() if k != "local_dir"},
              "source": source.relative_to(ROOT).as_posix(), "output": dest.relative_to(ROOT).as_posix(),
              "sha256": hashlib.sha256(dest.read_bytes()).hexdigest(),
              "elapsed_seconds": round(time.time() - started, 2), "review": "pending"}
    manifest = ROOT / "assets/models/league/v2/art-provenance.json"
    manifest.parent.mkdir(parents=True, exist_ok=True)
    records = json.loads(manifest.read_text(encoding="utf-8")) if manifest.exists() else []
    records = [r for r in records if r["name"] != args.name] + [record]
    manifest.write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps(record), flush=True)


if __name__ == "__main__":
    main()
