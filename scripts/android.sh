#!/bin/sh
# Build the Android APK.
#
# Needs, once:
#   rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
#   cargo install cargo-apk
#   an Android SDK with the NDK (sdkmanager "ndk;27.0.12077973")
#
#   ANDROID_HOME     the SDK folder
#   ANDROID_NDK_ROOT the NDK folder (found under $ANDROID_HOME/ndk when unset)
#
# A release APK must be signed, and cargo-apk refuses to pack one without a
# key. Point CARGO_APK_RELEASE_KEYSTORE and
# CARGO_APK_RELEASE_KEYSTORE_PASSWORD at a keystore to sign with; without
# them this script uses the debug keystore every Android tool installs,
# which is fine for trying a build on a phone but cannot be published.
#
# Usage: scripts/android.sh [--release] [--target <triple>]
set -e

cd "$(dirname "$0")/.."

if [ -z "$ANDROID_NDK_ROOT" ]; then
    ANDROID_NDK_ROOT=$(ls -d "${ANDROID_HOME:-$HOME/Android/Sdk}"/ndk/* 2>/dev/null | sort -V | tail -1)
    export ANDROID_NDK_ROOT
fi
if [ -z "$ANDROID_NDK_ROOT" ] || [ ! -d "$ANDROID_NDK_ROOT" ]; then
    echo "No Android NDK: set ANDROID_NDK_ROOT or install one with sdkmanager" >&2
    exit 1
fi

profile=""
target=""
while [ $# -gt 0 ]; do
    case "$1" in
        --release) profile="--release" ;;
        --target) shift; target="--target $1" ;;
        *) echo "unknown option $1" >&2; exit 2 ;;
    esac
    shift
done

if [ -n "$profile" ]; then
    if [ -z "$CARGO_APK_RELEASE_KEYSTORE" ]; then
        for k in "$HOME/.android/debug.keystore" "$LOCALAPPDATA/Android/debug.keystore"; do
            if [ -f "$k" ]; then
                CARGO_APK_RELEASE_KEYSTORE="$k"
                break
            fi
        done
    fi
    if [ -z "$CARGO_APK_RELEASE_KEYSTORE" ] || [ ! -f "$CARGO_APK_RELEASE_KEYSTORE" ]; then
        echo "No release keystore: set CARGO_APK_RELEASE_KEYSTORE and CARGO_APK_RELEASE_KEYSTORE_PASSWORD" >&2
        exit 1
    fi
    CARGO_APK_RELEASE_KEYSTORE_PASSWORD=${CARGO_APK_RELEASE_KEYSTORE_PASSWORD:-android}
    export CARGO_APK_RELEASE_KEYSTORE CARGO_APK_RELEASE_KEYSTORE_PASSWORD
    echo "Signing with $CARGO_APK_RELEASE_KEYSTORE"
fi

# shellcheck disable=SC2086
cargo apk build --manifest-path apps/tracedraw-android/Cargo.toml --lib $profile $target
