using System;
using System.Net.Sockets;
using System.Text;

namespace PithDdu.SimHubPlugin
{
    /// <summary>
    /// Sends one telemetry frame per call to the dashboard over UDP, one datagram
    /// per frame. UDP is connectionless and fire-and-forget, so a missing or
    /// restarted dashboard never stalls SimHub's data thread — the datagrams are
    /// simply dropped until it's listening again. The socket is created lazily and
    /// re-created (with the new endpoint) whenever the settings change.
    /// </summary>
    public sealed class Forwarder : IDisposable
    {
        private readonly object gate = new object();
        private PithSettings cfg = new PithSettings();

        private UdpClient udp;
        private bool ready;

        public string Status { get; private set; } = "idle";

        /// <summary>True while a UDP socket is open and enabled. (UDP is
        /// connectionless, so this is "ready to send", not a live link.)</summary>
        public bool Connected { get { lock (gate) { return ready; } } }

        public void Configure(PithSettings settings)
        {
            lock (gate)
            {
                cfg = settings;
                // Re-open against the new endpoint on the next send.
                CloseLocked();
            }
        }

        public void Send(string frame)
        {
            lock (gate)
            {
                if (!cfg.Enabled) { return; }
                if (!ready && !TryOpenLocked()) { return; }

                var bytes = Encoding.ASCII.GetBytes(frame);
                try
                {
                    udp.Send(bytes, bytes.Length);
                }
                catch (Exception e)
                {
                    Status = "send failed: " + e.Message;
                    CloseLocked();
                }
            }
        }

        private bool TryOpenLocked()
        {
            try
            {
                udp = new UdpClient();
                // "Connect" on a UDP socket only fixes the default destination; no
                // handshake happens, so this never blocks or fails on a dead peer.
                udp.Connect(cfg.Host, cfg.Port);
                ready = true;
                Status = "sending → " + cfg.Host + ":" + cfg.Port + " (UDP)";
                return true;
            }
            catch (Exception e)
            {
                Status = "open failed: " + e.Message;
                CloseLocked();
                return false;
            }
        }

        private void CloseLocked()
        {
            ready = false;
            try { udp?.Close(); } catch { }
            udp = null;
        }

        public void Dispose()
        {
            lock (gate) { CloseLocked(); }
        }
    }
}
