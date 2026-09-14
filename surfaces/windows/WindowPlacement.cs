using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text.Json;

namespace Arut.Windows;

// Window placement is a local desktop preference, independent of chat state.
// Keep Win32 workspace coordinates paired with SetWindowPlacement, which also
// brings a saved window back on screen when its monitor is disconnected.
internal static class WindowPlacement
{
    private static readonly string SettingsPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "Arut", "window.json");

    internal sealed record Settings(int Left, int Top, int Right, int Bottom, bool Maximized);

    [StructLayout(LayoutKind.Sequential)]
    private struct Placement
    {
        public int Length;
        public int Flags;
        public int ShowCommand;
        public int MinX, MinY, MaxX, MaxY;
        public int Left, Top, Right, Bottom;
    }

    [DllImport("user32.dll", ExactSpelling = true, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetWindowPlacement(nint window, ref Placement placement);

    [DllImport("user32.dll", ExactSpelling = true, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool SetWindowPlacement(nint window, in Placement placement);

    public static void Restore(nint window)
    {
        try
        {
            if (!File.Exists(SettingsPath))
                return;
            var saved = JsonSerializer.Deserialize(File.ReadAllText(SettingsPath), WindowSettingsJson.Default.Settings);
            if (saved is null || (long)saved.Right - saved.Left <= 0 || (long)saved.Bottom - saved.Top <= 0)
                return;
            var placement = new Placement
            {
                Length = Marshal.SizeOf<Placement>(),
                ShowCommand = saved.Maximized ? 3 : 1,
                MinX = -1,
                MinY = -1,
                Left = saved.Left,
                Top = saved.Top,
                Right = saved.Right,
                Bottom = saved.Bottom,
            };
            if (!SetWindowPlacement(window, in placement))
                Trace.TraceWarning("Could not restore window placement: {0}", Marshal.GetLastWin32Error());
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or JsonException)
        {
            Trace.TraceWarning("Could not read window placement: {0}", error.Message);
        }
    }

    public static void Save(nint window)
    {
        var placement = new Placement { Length = Marshal.SizeOf<Placement>() };
        if (!GetWindowPlacement(window, ref placement))
        {
            Trace.TraceWarning("Could not read window placement: {0}", Marshal.GetLastWin32Error());
            return;
        }
        var saved = new Settings(placement.Left, placement.Top, placement.Right, placement.Bottom,
            placement.ShowCommand == 3 || (placement.ShowCommand == 2 && (placement.Flags & 2) != 0));
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(SettingsPath)!);
            File.WriteAllText(SettingsPath, JsonSerializer.Serialize(saved, WindowSettingsJson.Default.Settings));
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            // A preference write must not prevent the session from closing.
            Trace.TraceWarning("Could not save window placement: {0}", error.Message);
        }
    }
}

[System.Text.Json.Serialization.JsonSerializable(typeof(WindowPlacement.Settings))]
internal partial class WindowSettingsJson : System.Text.Json.Serialization.JsonSerializerContext;
