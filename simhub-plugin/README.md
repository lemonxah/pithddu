# Pith DDU — SimHub plugin

Streams SimHub's normalized telemetry to the **Pith DDU dashboard** over TCP. It
emits the exact same `$`-frame the firmware already parses
(`pith_core::simhub::parse_line`), so nothing downstream needs to change.

```
SimHub ──[this plugin]── "$g;speed;rpm;…\n" ──► dashboard (TCP :28909) ──► device
```

## Why a plugin instead of "Custom Serial"

SimHub already normalizes 100+ games (incl. Windows-only titles like iRacing)
into one property model. This plugin reads that model directly and forwards it —
no NCalc template to hand-maintain, and it can feed the GUI (which then feeds the
device) rather than wiring the game straight to the device.

## Build

Requires the .NET SDK on Windows (or `msbuild`), and a SimHub install for the
two referenced DLLs (`GameReaderCommon.dll`, `SimHub.Plugins.dll`).

```powershell
# from this folder
dotnet build -c Release -p:SimHubPath="C:\Program Files (x86)\SimHub"
```

On a successful build the DLL (`PithDdu.SimHubPlugin.dll`) is **auto-copied into
the SimHub folder** (the `InstallToSimHub` target). Pass `-p:NoInstall=true` to
skip that. If SimHub is installed in the default location you can omit
`SimHubPath`.

> This is a C#/.NET Framework 4.8 project, separate from the Rust workspace
> (which ignores it).

### Building on Linux for a Wine SimHub

You don't need Windows. The `Microsoft.NETFramework.ReferenceAssemblies` package
(already referenced) provides the net48 reference assemblies cross-platform, so
the Linux `dotnet` SDK builds it directly:

```bash
# Point at the SimHub install inside your Wine/Proton prefix and build.
SH="$HOME/.wine/drive_c/Program Files (x86)/SimHub"   # adjust to your prefix
dotnet build -c Release -p:SimHubPath="$SH"
```

`GameReaderCommon.dll` and `SimHub.Plugins.dll` are read from that folder, and
the built DLL is copied back into it — then start SimHub under Wine and enable
the plugin. (SimHub is itself a WPF app that runs under Wine, so the settings
tab renders there too. If WPF ever misbehaves under your prefix, the plugin can
be made headless — config via a JSON file next to the DLL — just ask.)

> If a first build reports the SimHub types as missing (`GameReaderCommon could
> not be found`), it's a stale `obj/` from before the reference-assemblies
> package was restored — `rm -rf obj bin` and build again.

Verified: builds clean on Linux (`dotnet` 10 SDK) against a Wine SimHub prefix.

## Install & enable

1. Build (above) or copy `PithDdu.SimHubPlugin.dll` into the SimHub folder.
2. Start SimHub → it will ask to enable the new plugin → **Yes**.
3. Open **Additional plugins → Pith DDU** and set:
   - **Enabled**
   - **Host/Port** (default `127.0.0.1:28909` — the dashboard's listener)
   - **Send interval** (ms; `16` ≈ 60 Hz)
   - The **Link** line shows the live connection status.

The dashboard must be running (it owns the listener on `:28909`) for the link to
connect.

## Field mapping

The frame order is a hard contract with `parse_line` — **only ever append new
fields at the end, never reorder/insert.** Fields fall into two groups:

- **Typed, off `data.NewData`** (reliable): gear, speed, rpm, maxRpm, lap times,
  position, laps, water/oil temp, oil pressure, brake bias, fuel/cap, tyre
  temps/pressures/wear, throttle/brake/clutch.
- **By-name with a 0 fallback** (`FrameBuilder.Get`, refine per game using
  SimHub's *Available properties* browser): shift-light RPM, est lap, delta,
  opponents count, boost, TC/ABS level + active, brake temps, steer, headlights,
  wipers, pit limiter, ignition, flags, track %, car X/Z, sectors.

Units are noted inline in `FrameBuilder.cs` (x10 = tenths, dl = decilitres,
ml = millilitres, delta = 0.1 ms units). Loosely-mapped fields (boost, sectors,
car position, steer) are safe to leave at 0 — the device just shows them empty.

## Next (dashboard side)

The dashboard needs a small TCP listener on `:28909` that feeds each received
line into the existing telemetry path (`apply_telemetry`) and relays it to the
device. That's the Phase-2 change — see the repo's SimHub-plugin plan.
