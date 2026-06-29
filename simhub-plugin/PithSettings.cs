namespace PithDdu.SimHubPlugin
{
    /// <summary>Persisted plugin settings (saved via SimHub's common-settings store).</summary>
    public class PithSettings
    {
        public bool Enabled { get; set; } = true;

        // UDP transport — the dashboard's UDP telemetry server listens here.
        // Match this port to the dashboard's "Telemetry UDP" page.
        public string Host { get; set; } = "127.0.0.1";
        public int Port { get; set; } = 28909;

        /// <summary>Minimum gap between frames in ms (16 ≈ 60 Hz). Keeps the link from flooding.</summary>
        public int IntervalMs { get; set; } = 16;
    }
}
