$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$dist = Join-Path $repoRoot "dist\web-release"
$pkgStage = Join-Path $dist "__pkg_stage"
$assetsStage = Join-Path $dist "__assets_stage"

Set-Location $repoRoot

rustup target add wasm32-unknown-unknown

if (-not (Get-Command wasm-bindgen -ErrorAction SilentlyContinue)) {
    cargo install wasm-bindgen-cli --version 0.2.118
}

cargo build --release --target wasm32-unknown-unknown

New-Item -ItemType Directory -Force $dist | Out-Null
foreach ($path in @($pkgStage, $assetsStage, (Join-Path $dist "assets.zip"), (Join-Path $dist "index.html"))) {
    if (Test-Path $path) {
        Remove-Item -LiteralPath $path -Recurse -Force
    }
}
New-Item -ItemType Directory -Force $pkgStage | Out-Null

wasm-bindgen `
    "target\wasm32-unknown-unknown\release\VisionDetective.wasm" `
    --target web `
    --out-dir $pkgStage `
    --out-name visiondetective

New-Item -ItemType Directory -Force (Join-Path $assetsStage "assets") | Out-Null
foreach ($name in @("config", "fonts", "pic", "scene")) {
    Copy-Item `
        -Recurse `
        -Force `
        -Path (Join-Path $repoRoot "assets\$name") `
        -Destination (Join-Path $assetsStage "assets")
}

New-Item -ItemType Directory -Force (Join-Path $assetsStage "assets\wasm") | Out-Null
Copy-Item `
    -Force `
    -Path (Join-Path $pkgStage "visiondetective_bg.wasm") `
    -Destination (Join-Path $assetsStage "assets\wasm\visiondetective_bg.wasm")

Compress-Archive `
    -Path (Join-Path $assetsStage "assets") `
    -DestinationPath (Join-Path $dist "assets.zip") `
    -CompressionLevel Optimal

$js = Get-Content (Join-Path $pkgStage "visiondetective.js") -Raw
$js = $js -replace '/\* @ts-self-types="\.\/visiondetective\.d\.ts" \*/\r?\n', ''
$js = $js -replace 'export \{ initSync, __wbg_init as default \};\s*$', ''

$htmlPrefix = @'
<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>VisionDetective</title>
  <style>
    html,
    body {
      width: 100%;
      height: 100%;
      margin: 0;
      overflow: hidden;
      background: #101114;
    }

    canvas {
      display: block;
      width: 100vw !important;
      height: 100vh !important;
      outline: none;
    }

    #loading {
      position: fixed;
      inset: 0;
      display: grid;
      place-items: center;
      color: #e8e2d4;
      font: 16px/1.5 system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      background: #101114;
    }
  </style>
</head>
<body>
  <div id="loading">Loading VisionDetective...</div>
  <script type="module">
'@

$htmlSuffix = @'

    const loading = document.getElementById("loading");

    try {
      await __wbg_init("./assets/wasm/visiondetective_bg.wasm");
      loading?.remove();
    } catch (error) {
      console.error(error);
      if (loading) {
        loading.textContent = "VisionDetective failed to load. Unzip assets.zip beside index.html and serve the folder over HTTP.";
      }
    }
  </script>
</body>
</html>
'@

Set-Content `
    -LiteralPath (Join-Path $dist "index.html") `
    -Value ($htmlPrefix + $js + $htmlSuffix) `
    -Encoding UTF8

Remove-Item -LiteralPath $pkgStage -Recurse -Force
Remove-Item -LiteralPath $assetsStage -Recurse -Force

Get-ChildItem $dist -Force
