# Crop the bottom band of a PNG and double it, to look at a bar closely.
param([string]$In, [string]$Out, [int]$Height = 200)
Add-Type -AssemblyName System.Drawing
$im = [System.Drawing.Image]::FromFile($In)
$h = [Math]::Min($Height, $im.Height)
$r = New-Object System.Drawing.Rectangle 0, ($im.Height - $h), $im.Width, $h
$crop = New-Object System.Drawing.Bitmap $im.Width, $h
$g = [System.Drawing.Graphics]::FromImage($crop)
$g.DrawImage($im, (New-Object System.Drawing.Rectangle 0, 0, $im.Width, $h), $r, [System.Drawing.GraphicsUnit]::Pixel)
$big = New-Object System.Drawing.Bitmap ($im.Width * 2), ($h * 2)
$g2 = [System.Drawing.Graphics]::FromImage($big)
$g2.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
$g2.DrawImage($crop, 0, 0, ($im.Width * 2), ($h * 2))
$big.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $g2.Dispose(); $im.Dispose()
"saved $Out"
