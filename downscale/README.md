# Downscale

Shrinks any capture whose longest side exceeds a configured limit, before it's
saved, copied, or uploaded — handy for keeping upload sizes down or fitting a
host's dimension cap. Captures already within the limit pass through untouched.

- **Hook:** `on_capture` (image-blob API)
- **Capability:** `image = ["read", "modify"]`
- **Requires:** capscr 0.5.0+

## config

Create `%APPDATA%\com.capscr.capscr\data\plugins\downscale\config.toml`:

```toml
max_dimension = 1920   # longest side, in px; default 1920 if unset
```

Captures with `max(width, height) > max_dimension` are box-averaged down by an
integer factor so the longest side fits; smaller captures are left as-is.

A compact showcase of `config_get` + the image API in one plugin. Dependency-free
(hand-rolled box-average downscale) — a template for any resize/resample filter.
