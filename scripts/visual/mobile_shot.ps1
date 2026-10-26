# Capture a TraceDraw window to a PNG, for checking the phone layout on a
# desktop. Runs the app with TRACEDRAW_MOBILE=1 in a phone-sized window,
# waits for it to draw, captures the window and closes it.
param(
    [string]$Exe = "E:\TraeCN\tracedraw\target\debug\tracedraw.exe",
    [string]$Out = "E:\TraeCN\tracedraw\target\mobile-shot.png",
    [string]$File = "",
    [int]$WaitMs = 9000,
    [string]$Size = "412x915",
    # Clicks in the window's own pixels, "x,y;x,y": what a finger would do
    # before the capture (to open a sheet, say).
    [string]$Clicks = ""
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
}
"@
[void][Win]::SetProcessDPIAware()

$env:TRACEDRAW_MOBILE = "1"
$env:TRACEDRAW_MOBILE_SIZE = $Size
$env:TRACEDRAW_CONFIG_DIR = "E:\TraeCN\tracedraw\target\mobile-config"
$env:TRACEDRAW_NO_SPLASH = "1"

$appArgs = @("--no-splash")
if ($File -ne "") { $appArgs += $File }
$p = Start-Process -FilePath $Exe -ArgumentList $appArgs -PassThru
Start-Sleep -Milliseconds $WaitMs
$p.Refresh()
if ($p.MainWindowHandle -eq 0) {
    Write-Output "no window"
    $p | Stop-Process -Force
    exit 1
}
$h = $p.MainWindowHandle
# Top left and on top of whatever else is open, so nothing of it is off
# screen and nothing covers it (SetForegroundWindow alone often fails).
$HWND_TOPMOST = [IntPtr](-1)
[void][Win]::SetWindowPos($h, $HWND_TOPMOST, 8, 8, 0, 0, 0x0001 -bor 0x0004)
[void][Win]::SetForegroundWindow($h)
Start-Sleep -Milliseconds 1200
$rect = New-Object Win+RECT
[void][Win]::GetClientRect($h, [ref]$rect)
$origin = New-Object Win+POINT
$origin.X = 0; $origin.Y = 0
[void][Win]::ClientToScreen($h, [ref]$origin)

if ($Clicks -ne "") {
    foreach ($pair in $Clicks.Split(";")) {
        $xy = $pair.Split(",")
        $x = $origin.X + [int]$xy[0]
        $y = $origin.Y + [int]$xy[1]
        [void][Win]::SetCursorPos($x, $y)
        Start-Sleep -Milliseconds 250
        [void][Win]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 90
        [void][Win]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 900
    }
    Start-Sleep -Milliseconds 600
}

$w = $rect.Right - $rect.Left
$hh = $rect.Bottom - $rect.Top
if ($w -le 0 -or $hh -le 0) { Write-Output "empty rect"; $p | Stop-Process -Force; exit 1 }
$bmp = New-Object System.Drawing.Bitmap $w, $hh
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($origin.X, $origin.Y, 0, 0, (New-Object System.Drawing.Size $w, $hh))
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output "saved $Out ($w x $hh) at $($origin.X),$($origin.Y)"
$p | Stop-Process -Force
