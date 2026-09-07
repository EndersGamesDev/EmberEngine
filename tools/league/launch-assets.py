"""One outstanding LAN generator job per worker, with durable resume records.

Submit only to an idle worker. A client failure never automatically resubmits.
Raw outputs remain in target; reviewed web assets are selected separately.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
BASES = {"image": "http://192.168.178.187:8188", "video": "http://192.168.178.188:8189", "audio": "http://192.168.178.200:8193"}


def request(url, payload=None, binary=False):
    data = None if payload is None else json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as response:
        result = response.read()
    return result if binary else json.loads(result)


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8", newline="\n")


def validate_video(record, ledger):
    """A worker's done status does not prove it produced usable pixels."""
    sys.path.insert(0, str(ROOT / "target/league-launch/pydeps"))
    import imageio_ffmpeg
    import numpy as np
    import subprocess
    result = subprocess.run([imageio_ffmpeg.get_ffmpeg_exe(), "-v", "error", "-i", record["output"],
                             "-vf", "scale=80:48", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
                            check=True, capture_output=True)
    frames = np.frombuffer(result.stdout, np.uint8).reshape(-1, 48, 80, 3)
    deviation = frames.std(axis=(1, 2)).mean(axis=1) if len(frames) else np.array([])
    invalid = int(np.count_nonzero(deviation < 2))
    expected = record["spec"].get("num_frames")
    movement = float(np.abs(np.diff(frames.astype(np.float32), axis=0)).mean()) if len(frames) > 1 else 0
    valid = bool(len(frames) > 1 and invalid == 0 and movement > .01 and (expected is None or len(frames) == expected))
    record["pixel_validation"] = {"passed": valid, "frames": len(frames), "flat_or_black_frames": invalid,
                                  "temporalMeanAbsoluteDifference": movement,
                                  "review": "Visual review still required" if valid else "REJECTED; never promote this clip"}
    save(ledger, record)
    if not valid:
        raise SystemExit("Rejected video pixels; inspect " + str(ledger))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("kind", choices=BASES)
    parser.add_argument("name")
    parser.add_argument("spec", type=Path)
    parser.add_argument("--poll-seconds", type=int, default=20)
    args = parser.parse_args()
    if not args.name.replace("-", "").replace("_", "").isalnum():
        raise SystemExit("Use a simple job name")
    spec = json.loads(args.spec.read_text(encoding="utf-8"))
    folder = ROOT / "target/league-launch/assets" / args.name
    ledger = folder / "job.json"
    base = BASES[args.kind]
    if ledger.exists():
        record = json.loads(ledger.read_text(encoding="utf-8"))
        if record["kind"] != args.kind or record["spec"] != spec:
            raise SystemExit("Existing job has a different specification; use a new name")
        if record.get("output"):
            if args.kind == "video":
                validate_video(record, ledger)
            print(json.dumps(record), flush=True)
            return
        if not record.get("id"):
            raise SystemExit("Submission was uncertain. Inspect the worker history; do not resubmit")
    else:
        if args.kind == "image":
            queue = request(base + "/queue")
            if queue.get("queue_running") or queue.get("queue_pending"):
                raise SystemExit("Image worker is occupied; wait for its owner")
            sys.path.insert(0, "C:/Users/end/dev/cluster/provision/asset-forge")
            import asset_workers
            payload = {"client_id": "ultimate-legue-v3-launch", "prompt": asset_workers._sdxl_workflow(
                spec["prompt"], spec.get("negative", "text, watermark, blurry"),
                spec["width"], spec["height"], spec["steps"], spec["cfg"], spec["seed"],
                "ultimate-legue-v3/" + args.name)}
            endpoint = "/prompt"
        else:
            health = request(base + "/health")
            jobs = request(base + "/jobs")
            rows = jobs if isinstance(jobs, list) else jobs.get("jobs", [])
            if isinstance(rows, dict):
                rows = list(rows.values())
            if any((row.get("state") or row.get("status")) in ("queued", "running") for row in rows):
                raise SystemExit("Worker already has queued/running jobs; wait for its owner")
            if health.get("queue_size", 0) or health.get("queue_len", 0) or health.get("busy", False):
                raise SystemExit("Worker reports occupied")
            payload, endpoint = spec, "/jobs"
        record = {"kind": args.kind, "name": args.name, "spec": spec, "submitted": time.time(), "status": "submitting"}
        save(ledger, record)
        result = request(base + endpoint, payload)
        record["id"] = result.get("prompt_id") if args.kind == "image" else result.get("id")
        record["submission"] = result
        save(ledger, record)
        if not record["id"]:
            raise SystemExit("Worker did not return a job ID; inspect saved response")
        print("SUBMITTED " + json.dumps({"kind": args.kind, "id": record["id"], "name": args.name}), flush=True)
    job_id = record["id"]
    while True:
        if args.kind == "image":
            result = request(base + "/history/" + job_id).get(job_id, {})
            state = result.get("status", {}).get("status_str", "running")
            done = bool(result.get("outputs")) and state != "error"
        else:
            result = request(base + "/jobs/" + job_id)
            state = result.get("state") or result.get("status")
            done = state in ("done", "completed")
        record.update(status=state, last_checked=time.time(), result=result)
        save(ledger, record)
        if done:
            if args.kind == "image":
                outputs = [im for node in result["outputs"].values() for im in node.get("images", [])]
                if len(outputs) != 1:
                    raise SystemExit("Expected exactly one generated image")
                from urllib.parse import urlencode
                asset = outputs[0]
                url = base + "/view?" + urlencode({"filename": asset["filename"], "subfolder": asset.get("subfolder", ""), "type": asset.get("type", "output")})
                suffix = ".png"
            else:
                suffix = ".mp4" if args.kind == "video" else ".wav"
                url = base + "/jobs/" + job_id + ("/video.mp4" if args.kind == "video" else "/audio.wav")
            data = request(url, binary=True)
            output = folder / (args.name + suffix)
            output.write_bytes(data)
            record.update(output=str(output), sha256=hashlib.sha256(data).hexdigest(), elapsed_seconds=round(time.time() - record["submitted"], 3))
            if isinstance(result.get('started'), (int, float)) and isinstance(result.get('finished'), (int, float)):
                record['worker_elapsed_seconds'] = round(result['finished'] - result['started'], 3)
            save(ledger, record)
            if args.kind == "video":
                validate_video(record, ledger)
            print("COMPLETED " + json.dumps({k: record[k] for k in ("id", "output", "sha256", "elapsed_seconds")}), flush=True)
            return
        if state in ("error", "failed"):
            raise SystemExit("Worker failed; inspect " + str(ledger))
        time.sleep(args.poll_seconds)


if __name__ == "__main__":
    main()
