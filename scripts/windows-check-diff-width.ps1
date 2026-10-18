# Fixture-specific: default dark theme, short submodule split diff in lower pane.
param([string]$Screenshot)
Add-Type -AssemblyName System.Drawing
$b = [System.Drawing.Bitmap]::FromFile($Screenshot)
try {
 $max = 0; $bestY = 0; $right = 0
 for ($y=[int]($b.Height*0.55); $y -lt [int]($b.Height*0.9); $y++) {
  $count=0; $last=0
  for ($x=25; $x -lt $b.Width-25; $x++) {
   $c=$b.GetPixel($x,$y)
   if ($c.G -gt $c.R*1.25 -and $c.G -gt $c.B*1.1 -and $c.G -lt 100) { $count++; $last=$x }
  }
  if ($count -gt $max) { $max=$count; $bestY=$y; $right=$last }
 }
 $result = [ordered]@{ width=$b.Width; row=$bestY; greenPixels=$max; rightEdge=$right; coverage=[math]::Round($right/$b.Width,3); passed=($right -gt $b.Width*0.9) }
 $result | ConvertTo-Json
 if (-not $result.passed) { exit 1 }
} finally { $b.Dispose() }
