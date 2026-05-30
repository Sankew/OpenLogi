# OpenLogi

> **Options+ ? Try OpenLogi.**

A lightweight, local-first, open-source companion for Logitech HID++ peripherals.
No telemetry. No cloud. No account. No auto-update checker. Plain TOML config.

![status: pre-alpha](https://img.shields.io/badge/status-pre--alpha-orange)
![rust: stable](https://img.shields.io/badge/rust-stable-blue)
<!-- ![ci](https://github.com/AprilNEA/OpenLogi/actions/workflows/ci.yml/badge.svg) -->

---

## What it is

OpenLogi talks to Logitech HID++ mice, keyboards, and trackballs without
running the official Logi Options+ application. v0.0.1 is a probe-only CLI:
plug in a Logi Bolt receiver, run `openlogi`, see what's paired and how full
the batteries are.

## What it is not

- **Not a daemon.** Today it's a one-shot CLI; a background process will come
  when there's something useful for it to do (event injection, profile auto-switch).
- **Not a GUI.** No tray icon, no settings window. The config is a TOML file you
  edit. A graphical front-end may exist one day; it will not live in this binary.
- **Not a network app.** It will never make outbound HTTP, never check for
  updates, never report telemetry. Update by `cargo install` or `git pull`.
- **Not a drop-in for Options+.** Many features (gesture button + swipes, custom
  per-app profiles, DPI cycle, scroll inversion) are on the roadmap but not yet
  implemented. See [Status](#status).
- **Not affiliated with Logitech.** "Logitech", "MX Master", and "Options+" are
  trademarks of Logitech International S.A.

## Status

| Capability | v0.0.1 |
|---|---|
| Discover Logi Bolt receivers (CLI + GUI) | ✅ |
| List paired devices (slot, codename, kind, online state, wpid) | ✅ |
| Battery percentage / level / charging status | ✅ (online devices) |
| GPUI desktop window (static device list) | ✅ |
| Direct-Bluetooth devices (no receiver) | ❌ |
| Unifying receivers | ❌ (not yet in `hidpp 0.2`) |
| SmartShift toggle | 🚧 v0.0.2 |
| DPI control | 🚧 needs upstream `hidpp` feature `0x2201` |
| Button remapping | 🚧 needs upstream `hidpp` feature `0x1B04` |
| Per-app profile switching | 🚧 needs above + foreground-app detector |

## Install

Prerequisites: a recent stable Rust toolchain (Edition 2024, MSRV 1.85).

```sh
git clone https://github.com/AprilNEA/OpenLogi
cd OpenLogi
cargo run --release -- list
```

Or build and put the binary somewhere on `PATH`:

```sh
cargo build --release
cp target/release/openlogi ~/.local/bin/
```

The GUI binary (`openlogi-gui`) opens a desktop window with the same device list:

```sh
cargo run -p openlogi-gui --release
```

GUI builds need Apple's full Xcode toolchain (Xcode 16+ with the optional Metal
Toolchain component) on macOS. CLI builds need only stable Rust.

To build a macOS DMG locally, install `create-dmg`; the packaging script will
install `cargo-bundle` if needed:

```sh
brew install create-dmg
bash scripts/package-macos.sh
```

The default DMG is lean: device renders are fetched on demand at first launch.
Set `OPENLOGI_BUNDLE_ASSETS=1` before packaging to bundle every render for an
offline DMG.

### macOS

Quit **Logi Options+** before running `openlogi` — the two applications fight
over HID++ access and only one can talk to a given receiver at a time.

### Linux

You'll need read access to `/dev/hidraw*`. The shipped scripts and udev rules
will land alongside the first Linux release.

### Windows

Not tested in v0.0.1. The HID transport (`async-hid`) is cross-platform; bug
reports welcome.

## Configuration

Config lives at the platform-standard application support path:

- macOS: `~/Library/Application Support/dev.OpenLogi.openlogi/config.toml`
- Linux: `$XDG_CONFIG_HOME/openlogi/config.toml`
- Windows: `%APPDATA%\OpenLogi\openlogi\config\config.toml`

v0.0.1 doesn't read anything from it; the file isn't created until v0.0.2.

## Project layout

```
crates/
  openlogi-core/   serializable types, config, paths — no HID, no async
  openlogi-hid/    hidpp + async-hid glue: enumerate(), inventory types
  openlogi-cli/    the `openlogi` binary
  openlogi-gui/    the `openlogi-gui` binary — GPUI + gpui-component
```

## Developing on devenv (macOS)

This repo's `devenv.nix` sets up a Nix-based dev shell with sccache, the stable
Rust toolchain, and the env overrides GPUI needs. The first time you `cd` into
the repo after pulling a change to `devenv.nix`, **reload direnv** so the new
env vars (`DEVELOPER_DIR`, `SDKROOT`, the PATH filter that strips Nix's
`xcbuild` xcrun stub) take effect:

```sh
direnv reload    # or: exit your shell and `cd` back in
```

Without that, GPUI's `gpui_macos` build script can't find Apple's `metal`
shader compiler, and link errors about missing `_write` / `_sysconf` /
`_waitpid` symbols show up because the Nix `apple-sdk-14.4` stub doesn't
expose `libSystem` the way Apple's real linker wants.

The devenv DMG shortcut mirrors the GitHub Actions packaging workflow:

```sh
devenv tasks run openlogi:dmg
```

## Acknowledgments

- **[`hidpp`](https://crates.io/crates/hidpp)** by [@lus](https://github.com/lus)
  implements the HID++ protocol in Rust. OpenLogi is a consumer; this project
  would not be a weekend's work without it.
- **[Solaar](https://github.com/pwr-Solaar/Solaar)** is the de-facto reference
  for Logitech HID++ on Linux and a source of device-specific knowledge that
  Logitech does not publish.
- **[external-reference](https://github.com/external-reference)** by Tom Badash informed the
  feature scoping and the per-OS hook design we'll need for v0.0.3+.

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. The dual license is standard in the Rust ecosystem (see Tokio,
Serde, etc.): Apache contributes a patent grant, MIT maximises downstream
compatibility, and `MIT OR Apache-2.0` is recognised by every tool that audits
licenses.
