# capscr-plugins

Canonical registry + source-of-truth for the [capscr](https://github.com/lintowe/capscr) plugin marketplace.

`registry.json` at the repo root is what `https://rot.lt/capscr/registry.json` serves. The in-app Marketplace tab fetches it on demand. Plugin zips referenced by `download_url` are built from this repo and uploaded to `https://rot.lt/capscr/plugins/<id>-<version>.zip`.

## the runtime that shipped

capscr 0.4 shipped a **WASM plugin runtime** (not the native `Plugin` trait the earliest drafts here assumed). A plugin is a `cdylib` compiled to `wasm32-unknown-unknown` that exports a small C ABI; the host loads `plugin.wasm`, calls hook exports, and grants capability-gated host imports. The full ABI is in the capscr repo at [`docs/plugin-runtime.md`](https://github.com/lintowe/capscr/blob/master/docs/plugin-runtime.md). In short:

- export `capscr_alloc(size: i32) -> i32` (host writes hook payloads there) and `memory`
- export hooks: `capscr_on_capture_saved(ptr,len)`, `capscr_on_upload_success(ptr,len)`, and/or `capscr_on_capture(ptr,len) -> i64` (image-blob, capscr 0.5+)
- import what you need under module `capscr`: `log`, `clipboard_write_text`, `notify`, `fetch`
- declare matching `[capabilities]` in `plugin.toml`; the host enforces them

`plugin.toml` uses the sectioned schema (`[plugin]` / `[runtime]` / `[hooks]` / `[capabilities]`), not the flat metadata-only form.

## plugins here

| id | what | hook(s) | capability | min capscr |
|----|------|---------|-----------|-----------|
| `copy-file-path` | copy saved path to clipboard | on_capture_saved | clipboard:write | 0.4.0 |
| `capture-logger` | log save/upload events | on_capture_saved, on_upload_success | none | 0.4.0 |
| `desktop-toast` | notify with URL on upload | on_upload_success | notifications:show | 0.4.0 |
| `grayscale` | grayscale every capture | on_capture | image:read,modify | 0.5.0 |
| `borders` | solid border around captures | on_capture | image:read,modify | 0.5.0 |
| `webhook-notify` | POST uploaded link to a webhook | on_upload_success | fetch | 0.5.0 |
| `downscale` | shrink captures past a max dimension | on_capture | image:read,modify | 0.5.0 |
| `sounds` | event sounds | — | (needs an audio host import) | pending |
| `hotbar` | floating toolbar | — | (needs a UI host surface) | pending |

`sounds` and `hotbar` remain native-trait reference code: the WASM sandbox has no audio output or UI/window surface, so they can't run as WASM plugins until those host capabilities exist. They ship metadata-only for now.

## what's here

```
capscr-plugins/
├── registry.json          canonical listing — what rot.lt serves
├── <id>/
│   ├── plugin.toml        sectioned manifest (in the zip)
│   ├── README.md          (in the zip)
│   ├── Cargo.toml         cdylib build manifest (NOT in the zip)
│   └── src/lib.rs         the plugin (NOT in the zip)
├── scripts/build-zips.mjs builds wasm + packs dist/<id>-<version>.zip, updates registry.json
├── dist/                  generated zips (tracked; served via rot.lt)
├── Cargo.toml             workspace
└── LICENSE                MIT
```

## building + publishing

```bash
# one-time: the wasm target
rustup target add wasm32-unknown-unknown

# build every wasm plugin, pack the zips (incl. plugin.wasm), refresh sha256/size
node scripts/build-zips.mjs
```

Then commit the updated `registry.json` + `dist/*.zip` and push. This repo is the
**canonical source of truth**, but it is *not* consumed directly by the live site
— there is no submodule. The website (rot.lt, a separate SvelteKit repo) serves
the marketplace and must be updated to match each publish:

- the zips are **vendored** (copied) into `rot.lt/static/capscr/plugins/`, and
- `https://rot.lt/capscr/registry.json` is **generated** from `rot.lt/src/lib/data/plugins.ts`
  (id/version/download_url/sha256/size_bytes mirrored from this repo's `registry.json`).

So a push here does **not** auto-publish. After pushing, hand off to whoever
maintains rot.lt (e.g. "publish capscr-plugins @ \<sha\>") to copy the new zips,
update `plugins.ts`, and deploy.

New `sha256`/`size_bytes` start empty in `registry.json`; `build-zips.mjs` fills them from the actual zip bytes.

## writing a plugin

Use the dependency-free plugins here as templates — `grayscale` is the simplest per-pixel image filter, `copy-file-path` the simplest event forwarder. Each plugin is a `cdylib` gated with `#![cfg(target_arch = "wasm32")]` so a host `cargo build` of the workspace stays green. See [`docs/plugin-runtime.md`](https://github.com/lintowe/capscr/blob/master/docs/plugin-runtime.md) for the wire format and the worked Rust example.

## license

MIT — see [`LICENSE`](LICENSE).
