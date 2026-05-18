# hotbar

Floating, glass-effect toolbar pinned to a corner of the screen with quick-access capture buttons.

## what it does

Spawns a borderless top-most window with Windows acrylic blur (DWM `DwmEnableBlurBehindWindow` + `SetWindowCompositionAttribute`). Each button binds a virtual-key sequence dispatched via `SendInput`, so capscr's hotkey thread treats it identically to a real hardware keypress. Auto-hide after configurable idle delay.

## status

**v0.3.x: metadata-only.** capscr's plugin runtime arrives in v0.4. This plugin's manifest installs and shows under "installed", but the hotbar window doesn't render until then. The Win32 + SendInput logic sits in `src/lib.rs` ready for v0.4.

## config (v0.4 preview)

```toml
[hotbar]
enabled = true
position = "top"             # top | bottom | left | right
auto_hide = true
auto_hide_delay_ms = 1500

[hotbar.size]
button_size = 36
spacing = 4

[hotbar.theme]
foreground = [240, 240, 240, 255]
background = [30, 30, 30, 220]

[hotbar.glass]
enabled = true
blur_amount = 30
tint_color = [30, 30, 30, 180]

[[hotbar.buttons]]
label = "region"
glyph = "▢"
keys = ["Numpad5"]

[[hotbar.buttons]]
label = "gif"
glyph = "▶"
keys = ["Pause"]
```

## platform

Windows only — the glass effect uses Win32 DWM APIs. macOS / Linux equivalents are out of scope for v0.4.

## license

MIT — see `LICENSE` at the repo root.
