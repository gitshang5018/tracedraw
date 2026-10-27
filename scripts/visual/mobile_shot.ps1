# Capture a TraceDraw window to a PNG without touching the desktop: the
# pointer and the keyboard stay the user's, and the picture is asked of the
# window itself with PrintWindow. The sheet to look at is named with
# -Sheet, which the app opens at start (TRACEDRAW_MOBILE_SHEET), so a
# screenshot needs no click at all; -Clicks is there for the rare case a
# click is needed (the window takes them as messages, without the pointer).
param(
    [string]$Exe = "",
    [string]$Out = "",
    [string]$File = "",
    [int]$WaitMs = 15000,
    [string]$Size = "412x915",
    # Clicks in the window's own pixels, "x,y;x,y".
    [string]$Clicks = "",
    # A sheet to open at start, so a screenshot needs no click.
    [string]$Sheet = "",
    # A toolbox group whose floating tools to show there.
    [string]$Tools = "",
    # Bring the window to the front for the capture when PrintWindow comes
    # back empty (a graphics window that keeps its picture to itself).
    [switch]$ScreenCopy
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Bg {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
}
"@
[void][Bg]::SetProcessDPIAware()

# The folder the repository is in, from this script's own place.
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
if ($Exe -eq "") { $Exe = Join-Path $root "target\debug\tracedraw.exe" }
if ($Out -eq "") { $Out = Join-Path $root "target\mobile-shot.png" }

$env:TRACEDRAW_MOBILE = "1"
$env:TRACEDRAW_MOBILE_SIZE = $Size
$env:TRACEDRAW_CONFIG_DIR = (Join-Path $root "target\mobile-config")
$env:TRACEDRAW_NO_SPLASH = "1"
if ($Sheet -ne "") { $env:TRACEDRAW_MOBILE_SHEET = $Sheet } else { Remove-Item Env:\TRACEDRAW_MOBILE_SHEET -ErrorAction SilentlyContinue }
if ($Tools -ne "") { $env:TRACEDRAW_MOBILE_TOOLS = $Tools } else { Remove-Item Env:\TRACEDRAW_MOBILE_TOOLS -ErrorAction SilentlyContinue }

$appArgs = @("--no-splash")
if ($File -ne "") { $appArgs += ('"' + $File + '"') }
$p = Start-Process -FilePath $Exe -ArgumentList $appArgs -PassThru
Start-Sleep -Milliseconds $WaitMs
$p.Refresh()
$h = $p.MainWindowHandle
if ($h -eq 0) {
    Write-Output "no window"
    $p | Stop-Process -Force
    exit 1
}

# The window is put on top without being activated: the pointer, the
# keyboard and the focused window stay the user's, but the app draws and
# takes the clicks posted to it (an occluded window is not asked to).
$HWND_TOPMOST = [IntPtr](-1)
$SWP_NOSIZE = 0x0001; $SWP_NOMOVE = 0x0002; $SWP_NOACTIVATE = 0x0010
[void][Bg]::SetWindowPos($h, $HWND_TOPMOST, 0, 0, 0, 0, ($SWP_NOSIZE -bor $SWP_NOMOVE -bor $SWP_NOACTIVATE))
Start-Sleep -Milliseconds 800

# Clicks posted to the window: a move first, so it knows where the finger
# is, then the press and the release.
if ($Clicks -ne "") {
    $WM_MOUSEMOVE = 0x0200; $WM_LBUTTONDOWN = 0x0201; $WM_LBUTTONUP = 0x0202
    $MK_LBUTTON = 0x0001
    foreach ($pair in $Clicks.Split(";")) {
        $xy = $pair.Split(",")
        $x = [int]$xy[0]; $y = [int]$xy[1]
        $lp = [IntPtr](($y -shl 16) -bor ($x -band 0xFFFF))
        $a = [Bg]::SendMessage($h, $WM_MOUSEMOVE, [IntPtr]0, $lp)
        Start-Sleep -Milliseconds 150
        $b = [Bg]::SendMessage($h, $WM_LBUTTONDOWN, [IntPtr]$MK_LBUTTON, $lp)
        Start-Sleep -Milliseconds 90
        $c = [Bg]::SendMessage($h, $WM_LBUTTONUP, [IntPtr]0, $lp)
        Write-Output ("click {0},{1} -> move={2} down={3} up={4}" -f $x, $y, $a, $b, $c)
        Start-Sleep -Milliseconds 900
    }
    Start-Sleep -Milliseconds 600
}

$rect = New-Object Bg+RECT
[void][Bg]::GetClientRect($h, [ref]$rect)
$w = $rect.Right - $rect.Left
$hh = $rect.Bottom - $rect.Top
if ($w -le 0 -or $hh -le 0) { Write-Output "empty rect"; $p | Stop-Process -Force; exit 1 }

$bmp = New-Object System.Drawing.Bitmap $w, $hh
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
$ok = [Bg]::PrintWindow($h, $hdc, 3)
$g.ReleaseHdc($hdc)

# A graphics window may hand back an empty picture: count the colours to
# see whether anything was drawn.
$sample = @{}
for ($y = 4; $y -lt $hh; $y += 23) {
    for ($x = 4; $x -lt $w; $x += 23) {
        $c = $bmp.GetPixel($x, $y).ToArgb()
        $sample[$c] = 1
    }
}
$colours = $sample.Count
if (-not $ok -or $colours -le 2) {
    Write-Output "PrintWindow gave $colours colour(s): using the screen copy"
    $g.Dispose(); $bmp.Dispose()
    # The window is raised without being activated, so the pointer and the
    # keyboard stay where the user left them.
    [void][Bg]::SetWindowPos($h, $HWND_TOPMOST, 0, 0, 0, 0, ($SWP_NOSIZE -bor $SWP_NOMOVE -bor $SWP_NOACTIVATE))
    Start-Sleep -Milliseconds 1200
    $origin = New-Object Bg+POINT
    $origin.X = 0; $origin.Y = 0
    [void][Bg]::ClientToScreen($h, [ref]$origin)
    $bmp = New-Object System.Drawing.Bitmap $w, $hh
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($origin.X, $origin.Y, 0, 0, (New-Object System.Drawing.Size $w, $hh))
}

$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output "saved $Out ($w x $hh)"
$p | Stop-Process -Force
