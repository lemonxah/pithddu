using System;
using System.Globalization;
using System.Text;
using GameReaderCommon;
using SimHub.Plugins;

namespace PithDdu.SimHubPlugin
{
    /// <summary>
    /// Builds one line of the Pith "$"-frame from SimHub's normalized telemetry.
    ///
    /// Field ORDER + property names mirror the verified Custom-Serial template the
    /// dashboard generates (dashboard `catalog.rs::SIMFIELDS`) — that's the source
    /// of truth, proven across games. Everything is read by SimHub property path
    /// via <see cref="PluginManager.GetPropertyValue"/> so it matches exactly:
    /// `DataCorePlugin.GameData.*` for game data, plus a few namespaced props
    /// (`DataCorePlugin.Computed.*`, `DataCorePlugin.GameRawData.*`,
    /// `PersistantTrackerPlugin.*`).
    ///
    /// The frame is short-frame tolerant (the device keeps defaults for omitted
    /// trailing fields), so new fields may only ever be APPENDED, never reordered.
    /// Units inline: x10 = tenths, dl = decilitres, ml = millilitres, lap times in
    /// ms, delta in 0.1 ms units.
    /// </summary>
    public static class FrameBuilder
    {
        private const string G = "DataCorePlugin.GameData.";

        public static string Build(PluginManager pm, GameData data)
        {
            var sb = new StringBuilder(320);

            // 0: gear (N / R / 1..9; device also accepts 0 = neutral).
            sb.Append('$');
            sb.Append(GearChar(Str(pm, G + "Gear")));

            // 1..4: core.
            A(sb, R(Num(pm, G + "SpeedKmh")));
            A(sb, R(Num(pm, G + "Rpms")));
            A(sb, R(Num(pm, G + "CarSettings_MaxRPM")));
            A(sb, R(Num(pm, G + "CarSettings_RPMShiftLight1")));

            // 5..10: timing (ms); delta signed 0.1 ms units.
            A(sb, R(Num(pm, G + "CurrentLapTime")));      // TimeSpan → ms
            A(sb, R(Num(pm, G + "LastLapTime")));
            A(sb, R(Num(pm, G + "BestLapTime")));
            A(sb, R(Num(pm, G + "PersonalBestLapTime")));
            A(sb, R(Num(pm, G + "EstimatedLapTime")));
            A(sb, R(Num(pm, "PersistantTrackerPlugin.SessionBestLiveDeltaSeconds") * 10000.0));

            // 11..15: race.
            A(sb, R(Num(pm, G + "Position")));
            A(sb, R(Num(pm, G + "OpponentsCount")) + 1);
            A(sb, R(Num(pm, G + "CurrentLap")));
            A(sb, R(Num(pm, G + "TotalLaps")));
            A(sb, R(Num(pm, G + "RemainingLaps")));

            // 16..22: engine / car.
            A(sb, R(Num(pm, G + "WaterTemperature")));
            A(sb, R(Num(pm, G + "OilTemperature")));
            A(sb, R(Num(pm, G + "OilPressure") * 10.0));
            A(sb, R(Num(pm, G + "TurboPercent")));
            A(sb, R(Num(pm, G + "TCLevel")));
            A(sb, R(Num(pm, G + "ABSLevel")));
            A(sb, R(Num(pm, G + "BrakeBias") * 10.0));

            // 23..26: fuel.
            A(sb, R(Num(pm, G + "Fuel") * 10.0));
            A(sb, R(Num(pm, G + "CarSettings_FuelCapacity") * 10.0));
            // ml/lap: SimHub's computed average first, then the game's own per-lap
            // figure (varies by title). Check the diagnostics panel to see which one
            // actually holds your value and reorder/extend this list to match.
            A(sb, R(NumFirst(pm,
                "DataCorePlugin.Computed.Fuel_LitersPerLap",
                "DataCorePlugin.GameRawData.Graphics.fuelXLap",
                "DataCorePlugin.GameRawData.Graphics.FuelXLap",
                "DataCorePlugin.GameRawData.Physics.fuelXLap") * 1000.0));
            A(sb, R(NumFirst(pm,
                "DataCorePlugin.Computed.Fuel_RemainingLaps",
                "DataCorePlugin.GameRawData.Graphics.fuelEstimatedLaps") * 10.0));

            // 27..38: tyre temps °C, inner/mid/outer (one value per tyre → all three).
            int ttFL = R(Num(pm, G + "TyreTemperatureFrontLeft"));
            int ttFR = R(Num(pm, G + "TyreTemperatureFrontRight"));
            int ttRL = R(Num(pm, G + "TyreTemperatureRearLeft"));
            int ttRR = R(Num(pm, G + "TyreTemperatureRearRight"));
            A(sb, ttFL); A(sb, ttFL); A(sb, ttFL);
            A(sb, ttFR); A(sb, ttFR); A(sb, ttFR);
            A(sb, ttRL); A(sb, ttRL); A(sb, ttRL);
            A(sb, ttRR); A(sb, ttRR); A(sb, ttRR);

            // 39..42: tyre pressures.
            A(sb, R(Num(pm, G + "TyrePressureFrontLeft")));
            A(sb, R(Num(pm, G + "TyrePressureFrontRight")));
            A(sb, R(Num(pm, G + "TyrePressureRearLeft")));
            A(sb, R(Num(pm, G + "TyrePressureRearRight")));

            // 43..46: tyre wear % (default 100 = full when a game doesn't report it).
            A(sb, R(Num(pm, G + "TyreWearFrontLeft", 100)));
            A(sb, R(Num(pm, G + "TyreWearFrontRight", 100)));
            A(sb, R(Num(pm, G + "TyreWearRearLeft", 100)));
            A(sb, R(Num(pm, G + "TyreWearRearRight", 100)));

            // 47..50: brake temps °C.
            A(sb, R(Num(pm, G + "BrakeTemperatureFrontLeft")));
            A(sb, R(Num(pm, G + "BrakeTemperatureFrontRight")));
            A(sb, R(Num(pm, G + "BrakeTemperatureRearLeft")));
            A(sb, R(Num(pm, G + "BrakeTemperatureRearRight")));

            // 51..54: inputs (0..100; steer unmapped, like the verified template).
            A(sb, R(Num(pm, G + "Throttle")));
            A(sb, R(Num(pm, G + "Brake")));
            A(sb, R(Num(pm, G + "Clutch")));
            A(sb, 0);

            // 55..60: aids + states.
            A(sb, R(Num(pm, G + "TCActive")));
            A(sb, R(Num(pm, G + "ABSActive")));
            A(sb, R(Num(pm, "DataCorePlugin.GameRawData.Graphics.RainLights")));  // headlights (ACC)
            A(sb, R(Num(pm, "DataCorePlugin.GameRawData.Graphics.WiperLV")));      // wipers (ACC)
            A(sb, R(Num(pm, G + "PitLimiterOn")));
            A(sb, R(Num(pm, G + "EngineIgnitionOn")));                            // ignition

            // 61: flag code (0 none,1 green,2 yellow,3 blue,4 white,5 checkered,6 black).
            A(sb, FlagCode(pm));

            // 62: lap progress 0..1000 (= 0..100.0%).
            A(sb, R(Num(pm, "DataCorePlugin.GameData.TrackPositionPercent") * 1000.0));

            // 63..64: world position for the self-learned map (best-effort; 0 → no map).
            A(sb, R(Num(pm, G + "CarCoordinateX")));
            A(sb, R(Num(pm, G + "CarCoordinateZ")));

            // 65..70: sector + best-sector times (ms; unmapped → 0).
            A(sb, 0); A(sb, 0); A(sb, 0);
            A(sb, 0); A(sb, 0); A(sb, 0);

            return sb.ToString();
        }

        /// <summary>Car model string sent as @CM (matches the template's [CarModel]).</summary>
        public static string CarModel(PluginManager pm) => Str(pm, G + "CarModel");

        /// <summary>Track name sent as @MAP (used to reset the self-learned map).</summary>
        public static string TrackName(PluginManager pm)
        {
            string t = Str(pm, G + "TrackName");
            return t.Length > 0 ? t : Str(pm, G + "TrackCode");
        }

        // ---- diagnostics ----

        /// Fields shown in the settings panel — NULL means the property name is
        /// wrong for this game (nothing to fix when it shows a value).
        public static readonly string[] DiagProps =
        {
            G + "Gear", G + "SpeedKmh", G + "Rpms", G + "CarSettings_MaxRPM",
            G + "CarSettings_RPMShiftLight1", G + "CurrentLapTime", G + "Position",
            G + "CurrentLap", G + "TotalLaps", G + "RemainingLaps",
            "PersistantTrackerPlugin.SessionBestLiveDeltaSeconds",
            G + "WaterTemperature", G + "OilTemperature", G + "OilPressure",
            G + "TurboPercent", G + "TCLevel", G + "ABSLevel", G + "BrakeBias",
            G + "Fuel", G + "CarSettings_FuelCapacity",
            // fuel-per-lap candidates — find which one holds your value:
            "DataCorePlugin.Computed.Fuel_LitersPerLap",
            "DataCorePlugin.Computed.Fuel_RemainingLaps",
            "DataCorePlugin.GameRawData.Graphics.fuelXLap",
            "DataCorePlugin.GameRawData.Graphics.FuelXLap",
            "DataCorePlugin.GameRawData.Physics.fuelXLap",
            "DataCorePlugin.GameRawData.Graphics.fuelEstimatedLaps",
            "DataCorePlugin.GameData.Fuel",
            "DataCorePlugin.GameData.MaxFuel",
            G + "TyreTemperatureFrontLeft", G + "TyrePressureFrontLeft", G + "TyreWearFrontLeft",
            G + "Throttle", G + "Brake", G + "Clutch",
            G + "TCActive", G + "ABSActive",
            "DataCorePlugin.GameRawData.Graphics.RainLights",
            "DataCorePlugin.GameRawData.Graphics.WiperLV",
            G + "PitLimiterOn", G + "EngineIgnitionOn",
            G + "TrackPositionPercent", G + "CarCoordinateX", G + "CarCoordinateZ",
            G + "CarModel", G + "TrackName",
        };

        public static string Diagnose(PluginManager pm, GameData data)
        {
            var sb = new StringBuilder(2048);
            sb.AppendLine("frame: " + Build(pm, data));
            sb.AppendLine();
            sb.AppendLine("@CM CarModel  = " + ShowProp(pm, G + "CarModel"));
            sb.AppendLine("@MAP TrackName = " + ShowProp(pm, G + "TrackName"));
            sb.AppendLine();
            sb.AppendLine("fields (NULL = wrong property for this game):");
            foreach (var n in DiagProps)
            {
                string label = n.Replace("DataCorePlugin.GameData.", "").Replace("DataCorePlugin.", "");
                sb.AppendLine("  " + label.PadRight(34) + " = " + ShowProp(pm, n));
            }
            return sb.ToString();
        }

        private static string ShowProp(PluginManager pm, string name)
        {
            try
            {
                object o = pm.GetPropertyValue(name);
                if (o == null) return "NULL";
                if (o is string s) return s.Length == 0 ? "(empty)" : "\"" + s + "\"";
                return Convert.ToString(o, CultureInfo.InvariantCulture);
            }
            catch (Exception e) { return "ERR(" + e.GetType().Name + ")"; }
        }

        // ---- helpers ----

        private static void A(StringBuilder sb, int v) { sb.Append(';'); sb.Append(v.ToString(CultureInfo.InvariantCulture)); }

        private static int R(double v) => double.IsNaN(v) || double.IsInfinity(v) ? 0 : (int)Math.Round(v);

        /// Read a SimHub property as a number. TimeSpan → total ms, bool → 0/1.
        /// Returns <paramref name="def"/> when the property is missing/non-numeric.
        private static double Num(PluginManager pm, string path, double def = 0)
        {
            try
            {
                object o = pm.GetPropertyValue(path);
                if (o == null) return def;
                if (o is TimeSpan ts) return ts.TotalMilliseconds;
                if (o is bool b) return b ? 1 : 0;
                return Convert.ToDouble(o, CultureInfo.InvariantCulture);
            }
            catch { return def; }
        }

        /// First non-zero of several candidate properties (for values whose source
        /// property differs by game, e.g. fuel-per-lap).
        private static double NumFirst(PluginManager pm, params string[] paths)
        {
            foreach (var p in paths)
            {
                double v = Num(pm, p);
                if (v != 0) return v;
            }
            return 0;
        }

        private static string Str(PluginManager pm, string path)
        {
            try { object o = pm.GetPropertyValue(path); return o == null ? "" : o.ToString(); }
            catch { return ""; }
        }

        private static char GearChar(string g)
        {
            if (string.IsNullOrEmpty(g)) return 'N';
            char c = g[0];
            return c == 'P' ? 'N' : c;
        }

        private static int FlagCode(PluginManager pm)
        {
            if (Num(pm, G + "Flag_Black") > 0) return 6;
            if (Num(pm, G + "Flag_Checkered") > 0) return 5;
            if (Num(pm, G + "Flag_White") > 0) return 4;
            if (Num(pm, G + "Flag_Blue") > 0) return 3;
            if (Num(pm, G + "Flag_Yellow") > 0) return 2;
            if (Num(pm, G + "Flag_Green") > 0) return 1;
            return 0;
        }
    }
}
