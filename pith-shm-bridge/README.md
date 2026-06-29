# pith-shm-bridge — native shared-memory telemetry on Linux/Proton

Two tiny tools that run **inside a sim's Proton/Wine prefix** and surface its
shared-memory telemetry (engine RPM, gear, speed, fuel, tyres…) to the Pith DDU
dashboard — **no SimHub, no game plugin**. This is how you get **ACC / AC EVO /
rFactor 2 / LMU / RaceRoom** data (including ACC's RPM → shift-lights, which its
UDP broadcasting feed does *not* carry) on Linux.

Why a helper is needed: under Proton the game's shared memory is anonymous and
not visible to native Linux. *Something* in the prefix has to read it. Pick one:

- **`pith-shim.exe`** *(recommended, simplest)* — reads the sim's shared memory
  and sends our `$`-frame over **UDP** to the dashboard's Telemetry-UDP server.
  Nothing else to configure; reuses the same pipeline as every other game.
- **`pith-shmbridge.exe`** — copies the sim's shared memory to a real
  `/dev/shm` file (via Wine's `Z:\dev\shm`) so the dashboard's **native
  `/dev/shm` reader** picks it up. Use this if you prefer the dashboard to read
  `/dev/shm` directly (or to share the data with other Linux tools).

Both reuse the dashboard's verified struct parsers + `$`-frame serializer
(`pith-core`), so they stay byte-identical to what the device already understands.

## Build (on Linux, cross-compiled to Windows)

```bash
rustup target add x86_64-pc-windows-gnu      # one-time (needs mingw-w64)
cd pith-shm-bridge
cargo build --release --target x86_64-pc-windows-gnu
# → target/x86_64-pc-windows-gnu/release/pith-shim.exe
#   target/x86_64-pc-windows-gnu/release/pith-shmbridge.exe
```

## Run it inside the game's prefix

The `.exe` must run in the **same Proton container** as the game (same wineserver)
so it can see the shared memory. A *separate* launch into the prefix (plain
`protontricks-launch` while the game runs) gets its own container and **won't**
see the memory — so use one of these:

### Steam + Proton — `pith-shim-run` (no extra tools)

This wrapper injects the exe into the game's own container via Proton's launcher
service. One-time setup:
```bash
mkdir -p ~/pith
cp target/x86_64-pc-windows-gnu/release/pith-shim.exe ~/pith/
cp target/x86_64-pc-windows-gnu/release/pith-shmbridge.exe ~/pith/   # optional
chmod +x pith-shim-run
```
Steam → the game → **Properties → Launch Options**:
```
/full/path/to/pith-shm-bridge/pith-shim-run %command%
```
Launch the game; check `~/pith/pith-shim-run.log`. Override defaults inline, e.g.
`PITH_PORT=5005 /path/pith-shim-run %command%`, or `PITH_BRIDGE=1 …` to run the
bridge instead of the shim. (Needs `steam-runtime-launch-client` from the Steam
runtime on PATH; set `PITH_LAUNCH_CLIENT=/full/path` if it isn't.)

**Running extra companion exes in the same prefix** (e.g. `lmuFFB` with Le Mans
Ultimate): list them in `PITH_EXTRA` (`;`-separated exe paths, +optional args) —
they're injected into the game's container alongside the shim, every launch:
```
PITH_EXTRA="$HOME/lmu/lmuFFB.exe" /full/path/pith-shm-bridge/pith-shim-run %command%
```
This is just the generic mechanism — the launcher service can run *any* exe in the
container. To fire one manually while the game runs:
```
steam-runtime-launch-client --bus-name=com.steampowered.App<APPID> -- wine /path/to/lmuFFB.exe
```

### Lutris / Heroic (plain Wine, no container)

No pressure-vessel, so just run it in the same prefix (a pre-launch script works):
```bash
WINEPREFIX="/path/to/game/prefix" wine ~/pith/pith-shim.exe 127.0.0.1 28909
```

### Steam Tinker Launch

Also works (it does the same launcher-service injection for you) — add the exe as
a background/secondary program.

`pith-shim.exe [host] [port]` — defaults `127.0.0.1 28909` (match the dashboard's
**Telemetry UDP** page). On the dashboard you'll see the source show e.g.
`AC/ACC` / `rF2/LMU` and the device gets full telemetry.

`pith-shmbridge.exe` takes no args — start it in the prefix, then enable
**Shared memory (Linux)** on the dashboard's Telemetry UDP page.

## Supported games

| Game | Shared-memory block read |
|------|--------------------------|
| Assetto Corsa / ACC | `acpmf_physics` |
| Assetto Corsa EVO | `acevo_pmf_physics` |
| rFactor 2 / Le Mans Ultimate | `$rFactor2SMMP_Telemetry$` + `$rFactor2SMMP_Scoring$` |
| RaceRoom | `$R3E` |

> Anti-cheat: these read the games' **official telemetry-output** shared memory
> (the same interface SimHub/CrewChief use) — read-only, no injection, no game
> modification. Not a cheat. See the repo's `dashboard/SHARED_MEMORY.md`.
