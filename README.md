# pithddu

The **Pith DDU** sim-racing dashboard — a single all-Rust monorepo for the device
firmware, the desktop companion app, and the shared crates between them.

```
pithddu/
├─ dashboard/   Desktop companion app (Rust + Slint). Configure shift lights, touch
│               buttons, the race-screen layout and per-car data; build/flash firmware;
│               mirror live telemetry. → binary `pith-dashboard`
├─ firmware/    ESP32-S3 (XIAO S3) firmware (Rust + esp-idf, embedded-graphics + mipidsi).
│               → binary `pithddu`. Its own esp toolchain + Xtensa target.
├─ pith-core/   Shared, host-testable pure logic: telemetry parse, wire formatting,
│               field registry (codegen from firmware/main/field_registry.json). no_std.
└─ pith-ui/     Shared runtime-interpreted UI engine: a UiDoc (postcard blob) is loaded
                and rendered at runtime via embedded-graphics — no recompile to change
                screens. Renders identically on the device panels and in the desktop
                preview. no_std.
```

## Workspaces

The host crates (`dashboard`, `pith-core`, `pith-ui`) form one Cargo workspace at the
repo root. The **firmware is a separate sub-workspace** — it needs the `esp` Rust
toolchain and the `xtensa-esp32s3-espidf` target (`firmware/.cargo/config.toml`,
`firmware/rust-toolchain.toml`), so it is **excluded** from the root workspace and
path-depends on the shared crates (`../pith-core`).

```sh
# Host side (dashboard + shared crates) — stable toolchain
cargo build --release -p pith-dashboard
cargo test  -p pith-core
cargo run   -p pith-dashboard --example ui_preview   # live pith-ui device preview

# Firmware — esp toolchain (source ~/export-esp.sh first)
cd firmware && cargo build --release
```

The single source of truth for bindable telemetry fields is
`firmware/main/field_registry.json`; both `pith-core` and the dashboard generate their
field registries from it at build time (`build.rs`).

## Releases

Independent release streams from this one repo, via tag prefixes:

- `dashboard-v*` → desktop app release (Linux tarball + `.deb`, Windows zip)
- `firmware-v*`  → firmware app image (`pithddu-<board>.bin`)

## History

This monorepo was started fresh (no history) by folding together the all-Rust
`pithddu-dashboard` and `pithddu-firmware` projects. Those repos retain their own
history and prior releases.
