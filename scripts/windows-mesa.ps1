# SPDX-License-Identifier: MIT
# CI-only software OpenGL. This DLL is not included in release assets.
param(
    [Parameter(Mandatory)][ValidateSet('aarch64-pc-windows-msvc', 'x86_64-pc-windows-msvc')][string]$Target,
    [Parameter(Mandatory)][ValidateSet('debug', 'release')][string]$Profile
)
$ErrorActionPreference = 'Stop'
$version = '26.2.4'
if ($Target -eq 'aarch64-pc-windows-msvc') {
    $arch = 'arm64'
    $digest = '171e0cc3d48a2d435f7ebda28159d800bed7c4b2bda3ac7f9cc708b56f5499a1'
} else {
    $arch = 'x64'
    $digest = '6eff97dc5b33c8017a67eb617a1b010341b1569863caa8638b508983645daeec'
}
$archive = Join-Path $env:RUNNER_TEMP "mesa-llvmpipe-$arch-$version.7z"
Invoke-WebRequest "https://github.com/mmozeiko/build-mesa/releases/download/$version/mesa-llvmpipe-$arch-$version.7z" -OutFile $archive
if ((Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $digest) {
    throw 'Mesa archive checksum mismatch'
}
$destination = Join-Path $env:RUNNER_TEMP "mesa-$arch"
7z x $archive "-o$destination" -y opengl32.dll
if ($LASTEXITCODE -ne 0) { throw 'Mesa extraction failed' }
Copy-Item (Join-Path $destination 'opengl32.dll') "target/$Target/$Profile/opengl32.dll"
