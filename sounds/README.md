# sounds

Plays audio cues on capture lifecycle events — pre-capture, post-capture, post-save, post-upload.

## what it does

Subscribes to capscr's `PreCapture` / `PostCapture` / `PostSave` / `PostUpload` events and plays the configured WAV per event. Uses `rodio` for output on a dedicated thread so playback never blocks the capture pipeline.

## status

**v0.3.x: metadata-only.** capscr's plugin runtime arrives in v0.4. This plugin's manifest installs and shows under "installed", but no audio is played until then. The playback logic sits in `src/lib.rs` ready for v0.4.

Note: capscr 0.3.x already plays a built-in `screenshot.wav` / `upload.wav` via Win32 `PlaySoundW` (see `src/sound.rs` in the capscr repo). This plugin replaces that path with per-event, per-mode, user-supplied sounds.

## config (v0.4 preview)

```toml
[sounds]
enabled = true
volume = 1.0           # 0.0 to 1.0

[sounds.sounds.pre_capture]
path = "..."           # absolute or relative to plugin dir

[sounds.sounds.post_capture]
path = "..."
volume = 0.8           # per-event override
only_modes = ["region", "window"]   # optional filter

[sounds.sounds.post_save]
path = "..."

[sounds.sounds.post_upload]
path = "..."
```

## license

MIT — see `LICENSE` at the repo root.
