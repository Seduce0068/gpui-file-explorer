<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# NOTICE — GPUI File Explorer prototype

This project (`gpui-file-explorer`, license **AGPL-3.0-or-later**, see
[LICENSE](LICENSE)) is a prototype built on top of the following third-party
components. They remain under their own licenses; the AGPL applies to this
project's source code.

## Runtime stack

| Component | Version | License | Upstream |
|---|---|---|---|
| `gpui-kit` (facade crate) | 0.9.1 | Apache-2.0 | https://github.com/longbridge/gpui-kit |
| `gpui-component` | 0.9.1 | Apache-2.0 | https://github.com/longbridge/gpui-kit |
| `gpui-kit-assets` | 0.9.1 | Apache-2.0 | https://github.com/longbridge/gpui-kit |
| `gpui-pre` (package name; lib name `gpui`) | 0.3.12 | Apache-2.0 | https://github.com/zed-industries/zed (snapshot `zed@d2a4fb6`) |
| `gpui-pre-platform` | 0.3.12 | Apache-2.0 | https://github.com/zed-industries/zed |
| `gpui-base`, `gpui-pre-macros`, `gpui-pre-scheduler`, `gpui-pre-linux`, … | 0.3.12 / 0.9.1 | Apache-2.0 | same projects as above |

`gpui-kit` re-exports GPUI (`gpui-pre`), GPUI Component and the bundled asset
crate as a single dependency. `gpui-pre` is a packaging of a Zed GPUI snapshot
under the Apache-2.0 license; this prototype does not modify those crates.

## Icons

The icon glyphs rendered by the UI come from the **Lucide** icon set, which is
bundled into `gpui-kit-assets` (`assets/icons/*.svg`) and included in the
binary through `gpui_kit::assets::AllAssets`:

- Lucide — **ISC License** — https://lucide.dev / https://github.com/lucide-icons/lucide
- `gpui-kit-assets` bundles the SVGs and is Apache-2.0.

No OS-native file icons are extracted or redistributed.

## Direct dependencies of this prototype

| Crate | Version | License | Purpose |
|---|---|---|---|
| `gpui-kit` | 0.9.1 | Apache-2.0 | UI framework (GPUI + components + assets) |
| `alphanumeric-sort` | 1.5.8 | MIT | natural (human) name sort |
| `notify` | 8.2.2 | CC0-1.0 | filesystem notification backend |
| `notify-debouncer-full` | 0.7.0 | MIT OR Apache-2.0 | debouncing watcher events |
| `trash` | 5.3.0 | MIT | move files to the system trash (freedesktop trash spec) |
| `serde` | 1.0.x | MIT OR Apache-2.0 | settings (de)serialization |
| `serde_json` | 1.0.x | MIT OR Apache-2.0 | JSON settings file |
| `dirs` | 6.0.0 | MIT OR Apache-2.0 | XDG / platform config & standard directories |
| `async-channel` | 2.5.0 | Apache-2.0 OR MIT | watcher thread → UI task signalling |
| `chrono` | 0.4.x | MIT OR Apache-2.0 | local-time formatting of mtimes |
| `tracing` | 0.1.x | MIT | structured event macros (already transitive via gpui) |
| `tracing-subscriber` | 0.3.x | MIT OR Apache-2.0 | level/timestamp formatting, `RUST_LOG` filtering |

## Transitive dependencies

A GPUI application pulls in a large transitive dependency graph (wgpu, cosmic
text, accesskit, rust-embed, wayland/x11 protocol crates, …). At the time of
writing (`Cargo.lock`) those are licensed permissively (MIT / Apache-2.0 /
BSD / ISC / CC0 / Zlib). For a complete per-crate inventory, run:

```bash
cargo metadata --format-version 1 --manifest-path app/Cargo.toml \
  | jq -r '.packages[] | "\(.name) \(.version) \(.license)"' | sort
```

(Replace `jq` with any JSON reader if it is not installed.)
