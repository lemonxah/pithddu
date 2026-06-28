using System;
using System.Net.Sockets;
using System.Text;

namespace PithDdu.SimHubPlugin
{
    /// <summary>
    /// Sends one telemetry frame per call to the dashboard over TCP, lazily
    /// (re)connecting and swallowing transport errors so a dropped link never
    /// stalls SimHub's data thread. Reconnects are throttled to once a second.
    /// </summary>
    public sealed class Forwarder : IDisposable
    {
        private readonly object gate = new object();
        private PithSettings cfg = new PithSettings();

        private TcpClient tcp;
        private NetworkStream tcpStream;
        private int lastConnectTick = -100000;
        private bool connected;

        public string Status { get; private set; } = "idle";

        /// <summary>True while the TCP link to the dashboard is open.</summary>
        public bool Connected { get { lock (gate) { return connected; } } }

        public void Configure(PithSettings settings)
        {
            lock (gate)
            {
                cfg = settings;
                // Force a clean reconnect with the new parameters.
                CloseLocked();
                lastConnectTick = -100000;
            }
        }

        public void Send(string frame)
        {
            lock (gate)
            {
                if (!cfg.Enabled) { return; }
                if (!connected && !TryConnectLocked()) { return; }

                var bytes = Encoding.ASCII.GetBytes(frame + "\n");
                try
                {
                    tcpStream.Write(bytes, 0, bytes.Length);
                }
                catch (Exception e)
                {
                    Status = "send failed: " + e.Message;
                    CloseLocked();
                }
            }
        }

        private bool TryConnectLocked()
        {
            int now = Environment.TickCount;
            // Throttle reconnect attempts (TickCount can wrap; treat as ~1s window).
            if (unchecked(now - lastConnectTick) < 1000) { return false; }
            lastConnectTick = now;

            try
            {
                tcp = new TcpClient { NoDelay = true };
                tcp.Connect(cfg.Host, cfg.Port);
                tcpStream = tcp.GetStream();
                connected = true;
                Status = "connected (" + cfg.Host + ":" + cfg.Port + ")";
                return true;
            }
            catch (Exception e)
            {
                Status = "connecting… (" + e.Message + ")";
                CloseLocked();
                return false;
            }
        }

        private void CloseLocked()
        {
            connected = false;
            try { tcpStream?.Dispose(); } catch { }
            try { tcp?.Close(); } catch { }
            tcpStream = null;
            tcp = null;
        }

        public void Dispose()
        {
            lock (gate) { CloseLocked(); }
        }
    }
}
