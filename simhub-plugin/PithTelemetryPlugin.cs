using System;
using System.Windows.Controls;
using System.Windows.Media;
using GameReaderCommon;
using SimHub.Plugins;

namespace PithDdu.SimHubPlugin
{
    /// <summary>
    /// SimHub plugin that streams the Pith "$"-frame to the Pith DDU dashboard
    /// over UDP (one datagram per frame). The dashboard's UDP telemetry server
    /// parses the frame with the same code the firmware uses, so there is nothing
    /// new to decode downstream.
    /// </summary>
    [PluginName("Pith DDU Telemetry")]
    [PluginDescription("Forwards normalized telemetry to the Pith DDU dashboard / device.")]
    [PluginAuthor("pith")]
    public class PithTelemetryPlugin : IPlugin, IDataPlugin, IWPFSettingsV2
    {
        public PluginManager PluginManager { get; set; }
        public PithSettings Settings { get; private set; }

        private readonly Forwarder forwarder = new Forwarder();
        private int lastSentTick;
        private int lastIdentityTick;

        public string LeftMenuTitle => "Pith DDU";
        public ImageSource PictureIcon => null;

        public void Init(PluginManager pluginManager)
        {
            PluginManager = pluginManager;
            Settings = this.ReadCommonSettings("GeneralSettings", () => new PithSettings());
            forwarder.Configure(Settings);
            System.Diagnostics.Debug.WriteLine("[PithDDU] plugin initialised");
        }

        /// <summary>Apply edited settings: persist + reconnect with the new transport.</summary>
        public void ApplySettings()
        {
            this.SaveCommonSettings("GeneralSettings", Settings);
            forwarder.Configure(Settings);
        }

        public string Status => forwarder.Status;
        public bool Connected => forwarder.Connected;
        public long FramesSent { get; private set; }
        /// Live field dump (refreshed ~2 Hz) for the settings diagnostics panel.
        public string LastDiagnostics { get; private set; } = "(waiting for a running game…)";
        private int lastDiagTick;

        public void DataUpdate(PluginManager pluginManager, ref GameData data)
        {
            if (Settings == null || !Settings.Enabled) return;
            if (data?.NewData == null || !data.GameRunning) return;

            int now = Environment.TickCount;

            // ALWAYS (re)send the car + track on a ~1 s heartbeat — the dashboard
            // dedups and only forwards to the device on a real change. This way a
            // dashboard restart or device reboot re-applies the car/track without the
            // user having to switch cars to trigger it.
            if (unchecked(now - lastIdentityTick) >= 1000)
            {
                lastIdentityTick = now;
                string car = FrameBuilder.CarModel(pluginManager);
                if (car.Length > 0) forwarder.Send("@CM" + car);
                string track = FrameBuilder.TrackName(pluginManager);
                if (track.Length > 0) forwarder.Send("@MAP" + track);
            }

            // Throttle the high-rate frame to the configured cadence (SimHub can
            // fire faster than we want to push over the link).
            int interval = Settings.IntervalMs < 4 ? 4 : Settings.IntervalMs;
            if (unchecked(now - lastSentTick) < interval) return;
            lastSentTick = now;

            // Refresh the diagnostics dump ~2 Hz (cheap; for the settings panel).
            if (unchecked(now - lastDiagTick) >= 500)
            {
                lastDiagTick = now;
                try { LastDiagnostics = FrameBuilder.Diagnose(pluginManager, data); } catch { }
            }

            try
            {
                forwarder.Send(FrameBuilder.Build(pluginManager, data));
                FramesSent++;
            }
            catch (Exception e)
            {
                System.Diagnostics.Debug.WriteLine("[PithDDU] frame build/send failed: " + e.Message);
            }
        }

        public void End(PluginManager pluginManager)
        {
            this.SaveCommonSettings("GeneralSettings", Settings);
            forwarder.Dispose();
        }

        public Control GetWPFSettingsControl(PluginManager pluginManager) => new SettingsControl(this);
    }
}
