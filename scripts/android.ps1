# Build the Android APK on Windows (the shell script `android.sh` does the
# same on Linux and macOS).
#
# Needs, once:
#   rustup target add aarch64-linux-android
#   cargo install cargo-apk
#   an Android SDK with the NDK: sdkmanager "ndk;27.0.12077973"
#     (or unpack https://dl.google.com/android/repository/android-ndk-r27-windows.zip
#      into <SDK>\ndk\27.0.12077973)
#
#   ANDROID_HOME      the SDK folder (C:\Users\<you>\Android\Sdk by default)
#   ANDROID_NDK_ROOT  the NDK folder (the newest under $ANDROID_HOME\ndk)
#
# A release APK must be signed: point CARGO_APK_RELEASE_KEYSTORE and
# CARGO_APK_RELEASE_KEYSTORE_PASSWORD at a keystore to sign with. Without
# them this script signs with the debug keystore every Android tool
# installs, which is fine for trying a build on a phone but cannot be
# published.
#
# Usage: scripts\android.ps1 [-Release] [-Target <triple>]
param(
    [switch]$Release,
    [string]$Target = "aarch64-linux-android"
)
$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

if (-not $env:ANDROID_HOME) {
    $env:ANDROID_HOME = Join-Path $env:LOCALAPPDATA "Android\Sdk"
}
if (-not (Test-Path $env:ANDROID_HOME)) {
    throw "No Android SDK: set ANDROID_HOME (looked in $env:ANDROID_HOME)"
}
if (-not $env:ANDROID_NDK_ROOT) {
    $ndk = Get-ChildItem (Join-Path $env:ANDROID_HOME "ndk") -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending | Select-Object -First 1
    if ($ndk) { $env:ANDROID_NDK_ROOT = $ndk.FullName }
}
if (-not $env:ANDROID_NDK_ROOT -or -not (Test-Path $env:ANDROID_NDK_ROOT)) {
    throw "No Android NDK: set ANDROID_NDK_ROOT or install one with sdkmanager"
}

$profile = @()
if ($Release) {
    $profile = @("--release")
    if (-not $env:CARGO_APK_RELEASE_KEYSTORE) {
        foreach ($k in @((Join-Path $env:USERPROFILE ".android\debug.keystore"),
                         (Join-Path $env:LOCALAPPDATA "Android\debug.keystore"))) {
            if (Test-Path $k) { $env:CARGO_APK_RELEASE_KEYSTORE = $k; break }
        }
    }
    if (-not $env:CARGO_APK_RELEASE_KEYSTORE -or -not (Test-Path $env:CARGO_APK_RELEASE_KEYSTORE)) {
        throw "No release keystore: set CARGO_APK_RELEASE_KEYSTORE and CARGO_APK_RELEASE_KEYSTORE_PASSWORD"
    }
    if (-not $env:CARGO_APK_RELEASE_KEYSTORE_PASSWORD) {
        $env:CARGO_APK_RELEASE_KEYSTORE_PASSWORD = "android"
    }
    if (-not $env:CARGO_APK_RELEASE_KEYSTORE_KEY_ALIAS) {
        $env:CARGO_APK_RELEASE_KEYSTORE_KEY_ALIAS = "androiddebugkey"
    }
    Write-Output "Signing with $env:CARGO_APK_RELEASE_KEYSTORE"
}

Push-Location $root
try {
    cargo apk build --manifest-path apps/tracedraw-android/Cargo.toml --lib @profile --target $Target
    $out = Join-Path $root "target\release\apk\tracedraw.apk"
    if (-not (Test-Path $out)) { $out = Join-Path $root "target\debug\apk\tracedraw.apk" }
    if (Test-Path $out) { Write-Output "APK: $out" }
} finally {
    Pop-Location
}
