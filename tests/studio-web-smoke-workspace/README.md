# studio-web smoke fixture

This directory is the workspace root that the studio-web browser smoke suite
points `websocket-host` at. Files live here purely to give the smoke tests a
deterministic tree to walk.

## Contents

- `README.md` — this file, covered by the Phase 6 `markdown_view` case.
- `screenshot.png` — 1x1 placeholder image, covered by `image_view`.
- `model.stl` — minimal stl mesh used by the S2 preview assertion.
- `examples/cube.scad` — scad source used by `scad_split_view`.
- `examples/notes.txt` — sanity fixture for the directory tree test.
