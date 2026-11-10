# Arma el .wgt de Haze para Samsung, firmado con un perfil de Tizen Studio, y (con -Tv) lo
# instala y lo abre en la tele por WiFi. Ver docs/tizen.md.
#
#   .\build.ps1                      # solo build\Haze.wgt
#   .\build.ps1 -Tv 192.168.0.139    # ademas sdb connect, instalar y abrir
#
# El perfil por defecto es el de Mythos: los certificados de Samsung (autor y distribuidor con el
# DUID de la tele) sirven para cualquier app.
param(
  [string]$Tv = "",
  [string]$SecurityProfile = "Mythos",
  [string]$TizenStudio = "$env:USERPROFILE\tizen-studio"
)
$ErrorActionPreference = "Stop"
$here = $PSScriptRoot
$tizen = Join-Path $TizenStudio "tools\ide\bin\tizen.bat"
$sdb = Join-Path $TizenStudio "tools\sdb.exe"
$appId = "HazeMusic1.Haze"

# Solo lo que va en la app: una copia, porque la firma escribe ahi.
$build = Join-Path $here "build"
if (Test-Path $build) { Remove-Item -Recurse -Force $build }
New-Item -ItemType Directory $build | Out-Null
foreach ($f in "config.xml", "index.html", "app.js", "app.css", "icon.png") { Copy-Item (Join-Path $here $f) $build }
Copy-Item -Recurse (Join-Path $here "fonts") $build

& $tizen package -t wgt -s $SecurityProfile -- $build
if ($LASTEXITCODE -ne 0) { throw "tizen package fallo (¿existe el perfil '$SecurityProfile'? docs/tizen.md)" }
$wgt = Get-ChildItem $build -Filter *.wgt | Select-Object -First 1
Write-Host "Paquete: $($wgt.FullName)"
if (-not $Tv) { return }

$serial = "${Tv}:26101"
& $sdb connect $Tv
& $tizen install -n $wgt.Name -s $serial -- $build
if ($LASTEXITCODE -ne 0) { throw "tizen install fallo" }
& $tizen run -p $appId -s $serial
