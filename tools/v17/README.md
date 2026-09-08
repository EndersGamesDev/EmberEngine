# Arena v17 tools

This folder served Arena v17's expanded first-person combat presentation by adding the scutum shield and Murasama sword, with its posed fist, to the v16 operator rifle-and-hands build.

`prep_pictures.py` converts the marketplace albedos to the exact RGB sizes the renderer can use, and `build_viewmodel.py` fits and names `shield`, `sword`, and `hand_sword` nodes before writing the GLB and rig sidecar consumed by the Arena client for Q and E actions.

Original marketplace files remain outside version control; the v17 worked example in `docs/asset-pipeline.md` governs measured frames, alpha removal, node-name contracts, and sidecar provenance.
