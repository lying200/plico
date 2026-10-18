# CopyFromScreen may return a black bitmap when the interactive desktop cannot
# be captured. Such an image cannot prove that an application rendered correctly.
function Assert-CaptureVisible([System.Drawing.Bitmap]$Bitmap) {
    if ($Bitmap.Width -lt 1 -or $Bitmap.Height -lt 1) {
        throw 'Capture has no pixels; UI verification is invalid'
    }
    $stepX = [Math]::Max(1, [int][Math]::Floor($Bitmap.Width / 32))
    $stepY = [Math]::Max(1, [int][Math]::Floor($Bitmap.Height / 32))
    for ($y = 0; $y -lt $Bitmap.Height; $y += $stepY) {
        for ($x = 0; $x -lt $Bitmap.Width; $x += $stepX) {
            $pixel = $Bitmap.GetPixel($x, $y)
            if ($pixel.R -ne 0 -or $pixel.G -ne 0 -or $pixel.B -ne 0) { return }
        }
    }
    throw 'Capture samples are entirely black; desktop visibility is unverified. Do not count this run as UI or performance acceptance.'
}
