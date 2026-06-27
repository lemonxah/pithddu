use crate::state::{BoardDef, BoardPin, BtnData, ColorRule, ModSpec, Preset, State, Zone};

pub const CATALOG: &[(&str, &str, &str)] = &[
    ("gearSpeed", "Gear + Speed", "Big gear glyph and speed"),
    ("rpmLights", "RPM Lights", "12-LED shift strip"),
    ("lapDelta", "Lap Delta", "Delta to best/optimal"),
    ("position", "Position", "Race position / field"),
    ("fuel", "Fuel", "Fuel remaining"),
    ("fuelPerLap", "Fuel per Lap", "Avg consumption"),
    ("tyres", "Tyre Temps", "4-corner tyre grid"),
    ("tyreWear", "Tyre Wear", "4-corner wear %"),
    ("lapTimes", "Lap Times", "Current + best"),
    ("lastLap", "Last Lap", "Last lap time"),
    ("sectors", "Sectors", "S1/S2/S3 splits"),
    ("brakeBias", "Brake Bias", "Front bias %"),
    ("water", "Water Temp", "Coolant temp"),
    ("oil", "Oil Temp", "Oil temp"),
    ("tcAbs", "TC / ABS", "Aid levels"),
    ("mapPosition", "Track Map", "Position dot on track"),
    ("flag", "Flag", "Current flag colour"),
    ("clock", "Session Clock", "Time remaining"),
    ("speed", "Speed only", "Speed value"),
    ("gear", "Gear only", "Gear glyph"),
    ("rpmValue", "RPM Value", "Numeric rpm"),
];

pub const ZONE_KEYS: [&str; 5] = ["topStrip", "leftRail", "center", "rightRail", "bottom"];
pub const ZONE_TITLES: [&str; 5] = ["TOP STRIP", "LEFT RAIL", "CENTER", "RIGHT RAIL", "BOTTOM"];
pub const DEG: &str = "\u{00B0}";

pub fn zone_index(k: &str) -> usize {
    ZONE_KEYS.iter().position(|&z| z == k).unwrap_or(0)
}

pub fn mod_name(ty: &str) -> &str {
    CATALOG
        .iter()
        .find(|m| m.0 == ty)
        .map(|m| m.1)
        .unwrap_or(ty)
}

pub fn default_spec(ty: &str) -> ModSpec {
    let mut m = ModSpec {
        templ: ty.to_string(),
        enabled: true,
        ..Default::default()
    };
    let stat = |m: &mut ModSpec, f: &str, l: &str, u: &str, b: &str| {
        m.kind = "stat".into();
        m.field = f.into();
        m.label = l.into();
        m.unit = u.into();
        m.base = b.into();
    };
    match ty {
        "gearSpeed" => m.kind = "gearSpeed".into(),
        "gear" => m.kind = "gear".into(),
        "rpmLights" => m.kind = "rpmStrip".into(),
        "rpmValue" => stat(&mut m, "rpm", "RPM", "", "white"),
        "speed" => stat(&mut m, "speed_kmh", "KM/H", "", "white"),
        "lapDelta" => {
            stat(&mut m, "delta_ms", "DELTA", "", "amber");
            m.rules = vec![
                ColorRule {
                    op: "<".into(),
                    v: 0,
                    color: "green".into(),
                },
                ColorRule {
                    op: ">".into(),
                    v: 0,
                    color: "red".into(),
                },
            ];
        }
        "position" => {
            m.kind = "position".into();
            m.field = "position".into();
            m.label = "POS".into();
        }
        "fuel" => stat(&mut m, "fuel_dl", "FUEL", "L", "white"),
        "fuelPerLap" => stat(&mut m, "fuel_per_lap_ml", "FUEL/LAP", "L", "white"),
        "tyres" => {
            m.kind = "tyreGrid".into();
            m.field = "tt_fl_m".into();
            m.unit = DEG.into();
        }
        "tyreWear" => {
            m.kind = "tyreGrid".into();
            m.field = "tw_fl".into();
            m.unit = "%".into();
        }
        "lapTimes" => m.kind = "lapPair".into(),
        "lastLap" => stat(&mut m, "last_lap_ms", "LAST", "", "white"),
        "sectors" => m.kind = "sectors".into(),
        "brakeBias" => stat(&mut m, "brake_bias_x10", "BIAS", "%", "white"),
        "water" => {
            stat(&mut m, "water_c", "H2O", DEG, "white");
            m.rules = vec![ColorRule {
                op: ">".into(),
                v: 105,
                color: "red".into(),
            }];
        }
        "oil" => stat(&mut m, "oil_c", "OIL", DEG, "white"),
        "tcAbs" => m.kind = "tcDual".into(),
        "mapPosition" => m.kind = "map".into(),
        "flag" => {
            m.kind = "flag".into();
            m.base = "green".into();
        }
        _ => stat(&mut m, "speed_kmh", mod_name(ty), "", "white"),
    }
    m
}

fn make_preset(uid: &mut i32, name: &str, builtin: bool, spec: &[(&str, &[&str])]) -> Preset {
    let mut p = Preset {
        name: name.into(),
        builtin,
        zones: Vec::new(),
    };
    for z in 0..5 {
        let mut zn = Zone {
            key: ZONE_KEYS[z].into(),
            title: ZONE_TITLES[z].into(),
            modules: Vec::new(),
        };
        for (key, types) in spec {
            if zn.key == *key {
                for t in *types {
                    let mut ms = default_spec(t);
                    ms.id = format!("{t}-{uid}");
                    *uid += 1;
                    zn.modules.push(ms);
                }
            }
        }
        p.zones.push(zn);
    }
    p
}

pub fn seed_presets(s: &mut State) {
    s.presets.clear();
    s.presets.push(make_preset(
        &mut s.uid,
        "Endurance",
        true,
        &[
            ("topStrip", &["rpmLights"]),
            ("leftRail", &["lapDelta", "position"]),
            ("center", &["gearSpeed"]),
            ("rightRail", &["fuel", "tyres"]),
            ("bottom", &["lapTimes"]),
        ],
    ));
    s.presets.push(make_preset(
        &mut s.uid,
        "Sprint",
        true,
        &[
            ("topStrip", &["rpmLights"]),
            ("leftRail", &["lapDelta"]),
            ("center", &["gearSpeed"]),
            ("rightRail", &["position"]),
            ("bottom", &["lapTimes"]),
        ],
    ));
    s.presets.push(make_preset(
        &mut s.uid,
        "Drift",
        true,
        &[
            ("topStrip", &["rpmLights"]),
            ("leftRail", &["position"]),
            ("center", &["gearSpeed"]),
            ("rightRail", &["speed"]),
            ("bottom", &[]),
        ],
    ));
    s.presets.push(make_preset(
        &mut s.uid,
        "Rally",
        true,
        &[
            ("topStrip", &["rpmLights"]),
            ("leftRail", &["sectors"]),
            ("center", &["gearSpeed"]),
            ("rightRail", &["position"]),
            ("bottom", &["lapTimes"]),
        ],
    ));
    s.presets.push(make_preset(
        &mut s.uid,
        "Data",
        true,
        &[
            ("topStrip", &["rpmLights"]),
            ("leftRail", &["water", "oil"]),
            ("center", &["gearSpeed"]),
            ("rightRail", &["brakeBias", "tcAbs"]),
            ("bottom", &["lapTimes"]),
        ],
    ));
    s.zones = s.presets[0].zones.clone();
    s.active_preset = 0;
}

pub fn seed_shift(s: &mut State) {
    const RAMP: [u32; 12] = [
        0x00E676, 0x00E676, 0x00E676, 0x00E676, 0xFFB300, 0xFFB300, 0xFFB300, 0xFFB300, 0xFF3B30,
        0xFF3B30, 0xFF3B30, 0xFF3B30,
    ];
    const THR: [i32; 12] = [62, 66, 70, 74, 78, 82, 85, 88, 91, 94, 97, 99];
    for gear in 1..=6 {
        for i in 0..12 {
            s.leds[gear][i].rgb = RAMP[i];
            s.leds[gear][i].threshold = THR[i];
        }
    }
}

pub const BUTTON_FIELDS: [&str; 7] = [
    "",
    "headlights",
    "wipers",
    "pit_limiter",
    "ignition",
    "tc_active",
    "abs_active",
];

#[allow(clippy::too_many_arguments)]
fn b(l: &str, t: bool, on: bool, a: &str, c: u32, sy: bool, f: &str, av: bool) -> BtnData {
    BtnData {
        label: l.into(),
        toggle: t,
        on,
        action: a.into(),
        col: c,
        sync: sy,
        field: f.into(),
        avail: av,
    }
}

pub fn seed_buttons(s: &mut State) {
    s.btn_pages.clear();
    s.btn_pages.push(vec![
        b(
            "Pit Limiter",
            true,
            false,
            "PitLimiter",
            0x00E5A0,
            true,
            "pit_limiter",
            true,
        ),
        b(
            "Headlights",
            true,
            true,
            "Headlights",
            0xFFB300,
            true,
            "headlights",
            true,
        ),
        b(
            "Wipers", true, false, "Wipers", 0x2E9DFF, true, "wipers", true,
        ),
        b("Radio", false, false, "Radio", 0x00E5A0, false, "", false),
        b("Marker", false, false, "Marker", 0x00E5A0, false, "", false),
        b(
            "Reset Lap",
            false,
            false,
            "ResetLap",
            0x00E5A0,
            false,
            "",
            false,
        ),
    ]);
    s.btn_pages.push(vec![
        b("TC+", false, false, "TCPlus", 0x00E5A0, false, "", false),
        b("TC-", false, false, "TCMinus", 0x00E5A0, false, "", false),
        b(
            "ABS",
            true,
            false,
            "ABS",
            0x2E9DFF,
            true,
            "abs_active",
            true,
        ),
        b("BB+", false, false, "BBPlus", 0x00E5A0, false, "", false),
        b("BB-", false, false, "BBMinus", 0x00E5A0, false, "", false),
        b("MAP+", false, false, "MapPlus", 0x00E5A0, false, "", false),
    ]);
    s.btn_pages.push(vec![
        b(
            "Ignition", true, false, "Ignition", 0xFFB300, false, "", false,
        ),
        b(
            "Starter", false, false, "Starter", 0x00E5A0, false, "", false,
        ),
        b(
            "Pit Speed",
            false,
            false,
            "PitSpeed",
            0x00E5A0,
            false,
            "",
            false,
        ),
        b(
            "DRS",
            true,
            false,
            "DRS",
            0x00E5A0,
            true,
            "DRSAvailable",
            false,
        ),
        b("Push2Pass", false, false, "P2P", 0x00E5A0, false, "", false),
        b(
            "Neutral", false, false, "Neutral", 0x00E5A0, false, "", false,
        ),
    ]);
}

pub const PINDEFS: &[(&str, &str)] = &[
    ("sclk", "SPI SCLK"),
    ("mosi", "SPI MOSI"),
    ("miso", "SPI MISO"),
    ("dc", "Shared DC"),
    ("disp1_cs", "Display 1 CS"),
    ("disp2_cs", "Display 2 CS"),
    ("touch1_cs", "Touch 1 CS"),
    ("touch2_cs", "Touch 2 CS"),
    ("led_din", "LED data"),
];
pub const PIN_N: usize = 9;

pub fn seed_boards(s: &mut State) {
    let xiao = BoardDef {
        name: "Seeed XIAO ESP32-S3".into(),
        id: "xiao_s3".into(),
        target: "esp32s3".into(),
        pins: [
            ("D0", 1),
            ("D1", 2),
            ("D2", 3),
            ("D3", 4),
            ("D4", 5),
            ("D5", 6),
            ("D6", 43),
            ("D7", 44),
            ("D8", 7),
            ("D9", 8),
            ("D10", 9),
        ]
        .iter()
        .map(|(l, g)| BoardPin {
            label: (*l).into(),
            gpio: *g,
        })
        .collect(),
    };
    const GP_S3: &[i32] = &[
        1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 21, 38, 39, 40, 41, 42, 43, 44,
        45, 47, 48,
    ];
    const GP_S2: &[i32] = &[
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 33, 34, 35, 36, 37, 38, 39,
        40, 41, 42, 43, 44, 45,
    ];
    let gpio_board = |nm: &str, id: &str, target: &str, gp: &[i32]| BoardDef {
        name: nm.into(),
        id: id.into(),
        target: target.into(),
        pins: gp
            .iter()
            .map(|g| BoardPin {
                label: format!("GPIO{g}"),
                gpio: *g,
            })
            .collect(),
    };
    s.boards = vec![
        xiao,
        gpio_board("ESP32-S3-DevKitC-1", "devkitc_s3", "esp32s3", GP_S3),
        gpio_board("Waveshare ESP32-S3-Zero", "zero_s3", "esp32s3", GP_S3),
        gpio_board("Generic ESP32-S3", "generic_s3", "esp32s3", GP_S3),
        gpio_board("ESP32-S2 DevKit (1 screen)", "devkit_s2", "esp32s2", GP_S2),
        gpio_board("Lolin S2 Mini (1 screen)", "s2_mini", "esp32s2", GP_S2),
    ];
}

pub const GAME_PROCS: &[(&str, &str)] = &[
    ("iracingsim", "iracing"),
    ("ac2-win64-shipping", "assettocorsacompetizione"),
    ("acc.exe", "assettocorsacompetizione"),
    ("le mans ultimate", "lmu"),
    ("ams2avx", "automobilista2"),
    ("ams2", "automobilista2"),
    ("assettocorsaevo", "assettocorsaevo"),
    ("acevo", "assettocorsaevo"),
    ("acs.exe", "assettocorsa"),
    ("f1_24", "f12024"),
    ("f1_25", "f12025"),
    ("rrre", "rrre"),
    ("projectmotorracing", "projectmotorracing"),
];

pub const SIMFIELDS: &[(&str, &str, &str)] = &[
    ("gear", "Gear", "isnull([Gear],'N')"),
    ("speed", "Speed km/h", "format(isnull([SpeedKmh],0),'0')"),
    ("rpm", "RPM", "format(isnull([Rpms],0),'0')"),
    (
        "maxrpm",
        "Max RPM",
        "format(isnull([CarSettings_MaxRPM],0),'0')",
    ),
    (
        "shiftrpm",
        "Shift RPM",
        "format(isnull([CarSettings_RPMShiftLight1],0),'0')",
    ),
    (
        "curlap",
        "Current Lap",
        "format(timespantoseconds(isnull([CurrentLapTime],secondstotimespan(0)))*1000,'0')",
    ),
    (
        "lastlap",
        "Last Lap",
        "format(timespantoseconds(isnull([LastLapTime],secondstotimespan(0)))*1000,'0')",
    ),
    (
        "bestlap",
        "Best Lap",
        "format(timespantoseconds(isnull([BestLapTime],secondstotimespan(0)))*1000,'0')",
    ),
    (
        "pblap",
        "Personal Best",
        "format(timespantoseconds(isnull([PersonalBestLapTime],secondstotimespan(0)))*1000,'0')",
    ),
    (
        "estlap",
        "Estimated Lap",
        "format(timespantoseconds(isnull([EstimatedLapTime],secondstotimespan(0)))*1000,'0')",
    ),
    (
        "delta",
        "Delta (0.1ms)",
        "format(isnull([PersistantTrackerPlugin.SessionBestLiveDeltaSeconds],0)*10000,'0')",
    ),
    ("pos", "Position", "isnull([Position],0)"),
    ("field", "Field Size", "isnull([OpponentsCount],0)+1"),
    ("lapsdone", "Current Lap #", "isnull([CurrentLap],0)"),
    ("totallaps", "Total Laps", "isnull([TotalLaps],0)"),
    ("lapsleft", "Laps Left", "isnull([RemainingLaps],0)"),
    (
        "water",
        "Water Temp",
        "format(isnull([WaterTemperature],0),'0')",
    ),
    ("oil", "Oil Temp", "format(isnull([OilTemperature],0),'0')"),
    (
        "oilp",
        "Oil Press x10",
        "format(isnull([OilPressure],0)*10,'0')",
    ),
    ("boost", "Boost", "format(isnull([TurboPercent],0),'0')"),
    ("tc", "TC Level", "isnull([TCLevel],0)"),
    ("abs", "ABS Level", "isnull([ABSLevel],0)"),
    (
        "bias",
        "Brake Bias x10",
        "format(isnull([BrakeBias],0)*10,'0')",
    ),
    ("fuel", "Fuel x10", "format(isnull([Fuel],0)*10,'0')"),
    (
        "fuelcap",
        "Tank x10",
        "format(isnull([CarSettings_FuelCapacity],0)*10,'0')",
    ),
    (
        "fuelpl",
        "Fuel/Lap x1000",
        "format(isnull([Computed.Fuel_LitersPerLap],0)*1000,'0')",
    ),
    (
        "fuellaps",
        "Fuel Laps x10",
        "format(isnull([Computed.Fuel_RemainingLaps],0)*10,'0')",
    ),
    (
        "ttfli",
        "FL temp in",
        "format(isnull([TyreTemperatureFrontLeft],0),'0')",
    ),
    (
        "ttflm",
        "FL temp mid",
        "format(isnull([TyreTemperatureFrontLeft],0),'0')",
    ),
    (
        "ttflo",
        "FL temp out",
        "format(isnull([TyreTemperatureFrontLeft],0),'0')",
    ),
    (
        "ttfri",
        "FR temp in",
        "format(isnull([TyreTemperatureFrontRight],0),'0')",
    ),
    (
        "ttfrm",
        "FR temp mid",
        "format(isnull([TyreTemperatureFrontRight],0),'0')",
    ),
    (
        "ttfro",
        "FR temp out",
        "format(isnull([TyreTemperatureFrontRight],0),'0')",
    ),
    (
        "ttrli",
        "RL temp in",
        "format(isnull([TyreTemperatureRearLeft],0),'0')",
    ),
    (
        "ttrlm",
        "RL temp mid",
        "format(isnull([TyreTemperatureRearLeft],0),'0')",
    ),
    (
        "ttrlo",
        "RL temp out",
        "format(isnull([TyreTemperatureRearLeft],0),'0')",
    ),
    (
        "ttrri",
        "RR temp in",
        "format(isnull([TyreTemperatureRearRight],0),'0')",
    ),
    (
        "ttrrm",
        "RR temp mid",
        "format(isnull([TyreTemperatureRearRight],0),'0')",
    ),
    (
        "ttrro",
        "RR temp out",
        "format(isnull([TyreTemperatureRearRight],0),'0')",
    ),
    (
        "tpfl",
        "FL pressure",
        "format(isnull([TyrePressureFrontLeft],0),'0')",
    ),
    (
        "tpfr",
        "FR pressure",
        "format(isnull([TyrePressureFrontRight],0),'0')",
    ),
    (
        "tprl",
        "RL pressure",
        "format(isnull([TyrePressureRearLeft],0),'0')",
    ),
    (
        "tprr",
        "RR pressure",
        "format(isnull([TyrePressureRearRight],0),'0')",
    ),
    (
        "twfl",
        "FL wear %",
        "format(isnull([TyreWearFrontLeft],100),'0')",
    ),
    (
        "twfr",
        "FR wear %",
        "format(isnull([TyreWearFrontRight],100),'0')",
    ),
    (
        "twrl",
        "RL wear %",
        "format(isnull([TyreWearRearLeft],100),'0')",
    ),
    (
        "twrr",
        "RR wear %",
        "format(isnull([TyreWearRearRight],100),'0')",
    ),
    (
        "btfl",
        "FL brake C",
        "format(isnull([BrakeTemperatureFrontLeft],0),'0')",
    ),
    (
        "btfr",
        "FR brake C",
        "format(isnull([BrakeTemperatureFrontRight],0),'0')",
    ),
    (
        "btrl",
        "RL brake C",
        "format(isnull([BrakeTemperatureRearLeft],0),'0')",
    ),
    (
        "btrr",
        "RR brake C",
        "format(isnull([BrakeTemperatureRearRight],0),'0')",
    ),
    ("thr", "Throttle", "format(isnull([Throttle],0),'0')"),
    ("brk", "Brake", "format(isnull([Brake],0),'0')"),
    ("clu", "Clutch", "format(isnull([Clutch],0),'0')"),
    ("steer", "Steer", "0"),
    ("tcact", "TC Active", "isnull([TCActive],0)"),
    ("absact", "ABS Active", "isnull([ABSActive],0)"),
    (
        "lights",
        "Headlights",
        "format(isnull([GameRawData.Graphics.RainLights],0),'0')",
    ),
    (
        "wipers",
        "Wipers",
        "format(isnull([GameRawData.Graphics.WiperLV],0),'0')",
    ),
    (
        "pitlim",
        "Pit Limiter",
        "format(isnull([PitLimiterOn],0),'0')",
    ),
    (
        "ign",
        "Ignition",
        "format(isnull([EngineIgnitionOn],0),'0')",
    ),
];

pub fn seed_sim(s: &mut State) {
    s.sim.clear();
    for (id, label, expr) in SIMFIELDS {
        s.sim.push(crate::state::SimRow {
            id: (*id).into(),
            label: (*label).into(),
            expr: (*expr).into(),
            enabled: true,
            builtin: true,
        });
    }
}
