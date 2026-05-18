# capscr-plugins

Canonical registry + source-of-truth for the [capscr](https://github.com/lintowe/capscr) plugin marketplace.

`registry.json` at the repo root is what `https://rot.lt/capscr/registry.json` serves. The in-app Marketplace tab in capscr fetches that URL on demand. Plugin zips referenced by `download_url` in the registry are built from this repo and uploaded to `https://rot.lt/capscr/plugins/<id>-<version>.zip`.

## status

| layer | state |
|---|---|
| registry schema + manifest format | **stable** as of capscr 0.3.29 |
| plugin metadata (browse / install in the Marketplace tab) | **working today** |
| plugin runtime (event hooks — Plugin trait, WASM host) | **arrives in capscr 0.4** |

So today: users can `install` a plugin from the in-app marketplace, the manifest lands in `%APPDATA%/com.capscr.capscr/data/plugins/<id>/`, and it appears under "installed". The plugin's actual behaviour (image filter, sound playback, hotbar UI) lights up when capscr 0.4 ships its runtime.

The Rust source in `<plugin>/src/lib.rs` is the v0.4 implementation, gated behind a `runtime` cargo feature so the crates compile standalone today.

## what's here

```
capscr-plugins/
├── registry.json          canonical listing — what rot.lt serves
├── borders/
│   ├── plugin.toml        v0.3 manifest (in the zip)
│   ├── README.md          (in the zip)
│   ├── Cargo.toml         v0.4 build manifest (NOT in the zip)
│   └── src/lib.rs         v0.4 implementation (NOT in the zip)
├── sounds/
│   └── ...
├── hotbar/
│   └── ...
├── scripts/
│   └── build-zips.mjs     packs <id>/<plugin.toml + README.md + assets>
│                          into dist/<id>-<version>.zip, recomputes the
│                          sha256 + size_bytes in registry.json
├── dist/                  (gitignored — generated)
├── Cargo.toml             workspace for v0.4 dev builds
└── LICENSE                MIT
```

## publishing a plugin

The full contract is in the capscr repo at [`docs/marketplace.md`](https://github.com/lintowe/capscr/blob/master/docs/marketplace.md). Quick path:

1. Add a folder `<id>/` with `plugin.toml` + `README.md`.
2. Add an entry to `registry.json` (placeholder sha256 / size_bytes — script fills these in).
3. Run `node scripts/build-zips.mjs` from the repo root. It:
   - packs each plugin folder into `dist/<id>-<version>.zip`
   - excludes `src/`, `Cargo.toml`, `Cargo.lock`, `target/`, dotfiles
   - computes sha256, updates `registry.json`
4. Commit the updated `registry.json` and the new `dist/<id>-<version>.zip`.
5. The rot.lt deploy step picks up the submodule and publishes both the registry JSON and the zips under `https://rot.lt/capscr/`.

## developing a plugin (v0.4 preview)

Each plugin crate has a `runtime` feature. With it off (default), the crate compiles standalone — `cargo check --workspace` passes. With it on, the crate links against capscr's plugin host and exposes a `Plugin` impl:

```bash
# default (compiles, no runtime hookup)
cargo check --workspace

# v0.4 mode (requires capscr's plugin host being available)
cargo check --workspace --features runtime
```

The `Plugin` trait lives in `capscr::plugin` (currently stubbed; full surface lands with v0.4). The expected event types are `PreCapture / PostCapture / PostSave / PostUpload`.

## license

MIT — see [`LICENSE`](LICENSE).
