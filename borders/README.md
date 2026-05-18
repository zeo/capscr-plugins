# borders

Adds borders, drop shadows, and rounded corners to captures.

## what it does

Runs on the `PostCapture` event. Takes the captured RGBA image, applies the configured border treatment, and replaces the image in the pipeline so downstream actions (save, clipboard, upload) see the bordered version.

## status

**v0.3.x: metadata-only.** capscr's plugin runtime arrives in v0.4. This plugin's manifest installs and shows under "installed", but the border rendering doesn't run yet. The full algorithm sits in `src/lib.rs` ready for v0.4.

## config (v0.4 preview)

```toml
[borders]
enabled = true
style = "solid"          # solid | double | dashed | dotted | groove | ridge | inset | outset
size = 3
color = [60, 60, 60, 255]   # RGBA, 0-255
corner_radius = 0           # in px; 0 = sharp corners
padding = 0
background_color = []       # optional; empty = inherit
only_modes = []             # optional; ["region", "window", "fullscreen"]

[borders.shadow]            # optional block
offset_x = 4
offset_y = 4
blur_radius = 12
color = [0, 0, 0, 96]
```

## license

MIT — see `LICENSE` at the repo root.
