# Native shared-memory telemetry on Linux (ACC RPM / shift-lights without SimHub)

Some sims (Assetto Corsa, ACC, AC EVO, rFactor 2, Le Mans Ultimate, RaceRoom,
Automobilista 2) expose their **full** telemetry — including **engine RPM**, which
ACC's UDP broadcasting feed does *not* carry — only through Windows **shared
memory**, not UDP. The dashboard can read that shared memory natively on Linux
(`Telemetry UDP` page → *Active connectors* → **Shared memory (AC/ACC)**), but
there's one catch you have to solve first.

## Why a bridge is required

Under Proton/Wine the game creates its shared memory with
`CreateFileMapping(INVALID_HANDLE_VALUE, …)` — an **anonymous, pagefile-backed**
mapping. wineserver keeps it as an unlinked temp/`memfd` region inside the prefix,
so it has **no stable `/dev/shm/<name>` path** a native Linux process can open.
A small **in-prefix bridge** fixes this: it re-backs the game's mapping onto a real
`/dev/shm` file, which our reader then picks up automatically.

## Option A (recommended) — our own `pith-shm-bridge` tools

The repo ships two tiny in-prefix tools (`../pith-shm-bridge/`) so you don't need
a third-party bridge:

- **`pith-shim.exe`** — reads the sim's shared memory and sends our `$`-frame
  straight over **UDP** to the dashboard. No `/dev/shm` needed at all. Simplest.
- **`pith-shmbridge.exe`** — mirrors the shared memory to `/dev/shm` so the
  **Shared memory (Linux)** connector reads it natively.

Build + usage in `../pith-shm-bridge/README.md`. Both reuse the dashboard's
verified parsers (`pith-core`), so they're byte-identical to the device.

## Option B — simshmbridge (covers AC, ACC, AC EVO, AMS2/PCARS2, rF2, LMU, RaceRoom)

<https://github.com/Spacefreak18/simshmbridge>

1. Build it (needs a Wine dev toolchain for the `*bridge.exe`; see its README).
2. On the Linux side, run the daemon (`simd`) — it watches for a sim launching and
   creates the `/dev/shm` files.
3. Configure the game's Proton launch so the matching `*bridge.exe`
   (e.g. `acbridge.exe`) runs **inside the same prefix** as the game. simshmbridge's
   docs cover wiring this via the Steam launch options / `simd`.
4. Start ACC. You should now see the block appear:
   ```bash
   ls -l /dev/shm/ | grep -i acpmf      # → acpmf_physics, acpmf_graphics, acpmf_static
   ```

## Option C — LukasLichten shm-bridge + datalink

<https://github.com/LukasLichten/awesome-linux-simracing> (Rust; `shm-bridge` +
`datalink` wraps the Proton launch command). Same end result: `acpmf_physics`
lands in `/dev/shm`.

## What the dashboard does

Once a recognised block is in `/dev/shm`, the **Shared memory (Linux)** connector
auto-detects it, reads it at ~50 Hz, and feeds RPM / gear / speed / fuel / pedals
to the device — so **ACC shift-lights work natively, no SimHub, no plugin**. The
status row shows `Reading` when it's live. Recognised blocks:

- `acpmf_physics` → **AC / ACC**, `acevo_pmf_physics` → **AC EVO**
- `$R3E` → **RaceRoom** (engine RPM is rad/s; converted)
- `$rFactor2SMMP_Telemetry$` + `$rFactor2SMMP_Scoring$` → **rFactor 2 / LMU**
  (both buffers are read so the *player* car is matched by `mID`)

> Shift-light redline: the physics page has no max-RPM (that's in the static page),
> so the device uses its configured redline. Set a custom redline in **Shift
> Lights** if the car isn't matched from the library.

## Files / names reference

| Game | `/dev/shm` block (via bridge) | Notes |
|------|------------------------------|-------|
| Assetto Corsa / ACC | `acpmf_physics`, `acpmf_static` | RPM in physics, maxRpm in static |
| Assetto Corsa EVO | `acevo_pmf_physics` | divergent names |
| rFactor 2 | `$rFactor2SMMP_Telemetry$` | needs TheIronWolf plugin to populate |
| Le Mans Ultimate | `$rFactor2SMMP_Telemetry$` | built-in (enable plugins in-game) |
| RaceRoom | `$R3E` | RPM is rad/s |
| iRacing | — | EOS anti-cheat blocks Proton; not supported |

> AMS2 / Project CARS 2 don't need this — the dashboard already decodes their
> native **UDP** output directly.
