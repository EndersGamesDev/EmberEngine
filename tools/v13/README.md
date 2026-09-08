# Arena v13 tools

These scripts served Arena v13, the Trench City release, by generating material and orthographic prop pictures, recovering completed ComfyUI jobs, meshing seven reviewed props, and baking bundle-sized textures.

The runbook produces retained concepts under `assets/concepts/v13`, shipping GLBs under `assets/models/v13`, and the texture set embedded from `assets/textures/v13`; the Arena renderer and authored level consume those outputs, while `fetch-loop.sh` makes an interrupted image run resumable.

The release design and budgets live in `docs/plans/arena-v13-trench-city.md`, and `docs/asset-pipeline.md` records this directory as the worked generated-map pipeline, including source provenance and retention outside the shipping bundle.
