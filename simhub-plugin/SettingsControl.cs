using System;
using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Threading;

namespace PithDdu.SimHubPlugin
{
    /// <summary>
    /// Minimal settings tab (built in code — no XAML) shown under SimHub's
    /// "Additional plugins → Pith DDU". Edits write straight to the plugin's
    /// settings and reconfigure the link; a timer shows the live link status.
    /// </summary>
    public class SettingsControl : UserControl
    {
        private readonly PithTelemetryPlugin plugin;
        private readonly TextBlock statusText = new TextBlock
        {
            Foreground = Brushes.Orange,
            FontSize = 14,
            FontWeight = FontWeights.SemiBold,
            Margin = new Thickness(0, 16, 0, 0),
            Text = "○ Not connected",
        };

        public SettingsControl(PithTelemetryPlugin plugin)
        {
            this.plugin = plugin;
            var s = plugin.Settings;

            var root = new StackPanel { Margin = new Thickness(16) };
            root.Children.Add(Header("Pith DDU Telemetry"));

            var enabled = new CheckBox { Content = "Enabled", IsChecked = s.Enabled, Margin = new Thickness(0, 8, 0, 8) };
            enabled.Checked += (o, e) => { s.Enabled = true; plugin.ApplySettings(); };
            enabled.Unchecked += (o, e) => { s.Enabled = false; plugin.ApplySettings(); };
            root.Children.Add(enabled);

            root.Children.Add(Labeled("Host (dashboard)", Text(s.Host, v => { s.Host = v; plugin.ApplySettings(); })));
            root.Children.Add(Labeled("Port", Text(s.Port.ToString(), v => { if (int.TryParse(v, out var p)) { s.Port = p; plugin.ApplySettings(); } })));
            root.Children.Add(Labeled("Send interval ms", Text(s.IntervalMs.ToString(), v => { if (int.TryParse(v, out var i)) { s.IntervalMs = i; plugin.ApplySettings(); } })));

            root.Children.Add(statusText);

            // Live field diagnostics — shows what each frame field resolves to, so a
            // NULL/0 pinpoints a wrong SimHub property name to fix in FrameBuilder.
            root.Children.Add(new TextBlock
            {
                Text = "Fields (NULL = wrong property name):",
                Foreground = Brushes.Gray,
                FontSize = 11,
                Margin = new Thickness(0, 14, 0, 4),
            });
            var diag = new TextBox
            {
                IsReadOnly = true,
                FontFamily = new FontFamily("Consolas, monospace"),
                FontSize = 11,
                Height = 260,
                VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
                TextWrapping = TextWrapping.NoWrap,
                Background = Brushes.Black,
                Foreground = Brushes.LightGray,
            };
            root.Children.Add(diag);

            Content = root;

            // Poll the link status once a second: green ● when connected to the
            // dashboard, amber ○ otherwise, plus a live frame counter so you can
            // confirm data is actually flowing.
            var timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
            timer.Tick += (o, e) =>
            {
                bool up = plugin.Connected;
                statusText.Foreground = up ? Brushes.LimeGreen : Brushes.Orange;
                statusText.Text = (up ? "● " : "○ ") + plugin.Status
                    + "    ·    " + plugin.FramesSent.ToString("N0") + " frames sent";
                if (!diag.IsFocused) { diag.Text = plugin.LastDiagnostics; }
            };
            timer.Start();
        }

        private static TextBlock Header(string t) =>
            new TextBlock { Text = t, FontSize = 18, FontWeight = FontWeights.Bold, Foreground = Brushes.White };

        private static TextBox Text(string value, Action<string> onChange)
        {
            var tb = new TextBox { Text = value, Width = 200, HorizontalAlignment = HorizontalAlignment.Left };
            tb.LostFocus += (o, e) => onChange(tb.Text.Trim());
            return tb;
        }

        private static FrameworkElement Labeled(string label, FrameworkElement field)
        {
            var sp = new StackPanel { Orientation = Orientation.Vertical, Margin = new Thickness(0, 6, 0, 0) };
            sp.Children.Add(new TextBlock { Text = label, Foreground = Brushes.Gray, FontSize = 11 });
            sp.Children.Add(field);
            return sp;
        }
    }
}
