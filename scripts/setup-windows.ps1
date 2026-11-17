$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$plainRoot = Join-Path $repo 'native/plain-ecosystem'

foreach ($tool in @('cargo', 'rustc', 'clang', 'llvm-config', 'node')) {
  if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
    throw "Missing required tool '$tool'. Install Rust, LLVM 18 (including clang), and Node.js 20+ first."
  }
}
$llvmVersion = (& llvm-config --version).Trim()
if (-not $llvmVersion.StartsWith('18.')) {
  throw "Plain currently requires LLVM 18.x; found $llvmVersion. Install LLVM 18 and set LLVM_CONFIG_PATH / LLVM_SYS_180_PREFIX."
}

$llvmPrefix = (& llvm-config --prefix).Trim()
$env:LLVM_SYS_180_PREFIX = $llvmPrefix
$env:LLVM_CONFIG_PATH = (Get-Command llvm-config).Source
$env:LLVM_LINK_LLVM_DYLIB = '1'
$liveIdeOrigin = 'https://plain-ide-kappa.vercel.app'
$allowedOrigins = @($env:PLAIN_IDE_ORIGINS -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
if ($allowedOrigins -notcontains $liveIdeOrigin) {
  $allowedOrigins += $liveIdeOrigin
}
$env:PLAIN_IDE_ORIGINS = $allowedOrigins -join ','
Push-Location $plainRoot
try {
  cargo build --release --workspace
  if ($LASTEXITCODE -ne 0) { throw 'Plain compiler build failed.' }
} finally { Pop-Location }
$env:PLAIN_COMPILER_PATH = Join-Path $plainRoot 'target/release/plain.exe'
Write-Host 'Plain native compiler built successfully.' -ForegroundColor Green
Write-Host 'Starting local bridge. Copy the connection token into Plain IDE.' -ForegroundColor Cyan
node (Join-Path $repo 'native/bridge/server.mjs')
