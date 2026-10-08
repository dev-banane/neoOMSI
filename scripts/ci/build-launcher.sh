#!/usr/bin/env bash
# Build the launcher (github.com/neoOMSI/launcher, branch main) into the package:
# dist/<platform>/launcher, on macOS into neoOMSI.app. LAUNCHER_SRC=../launcher builds a
# local checkout as it is instead.
set -euo pipefail

platform="${1:?usage: build-launcher.sh <windows|macos|linux> <x64|arm64>}"
arch="${2:?usage: build-launcher.sh <windows|macos|linux> <x64|arm64>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"

case "$platform" in
  windows) flag=--win ;;
  macos) flag=--mac ;;
  linux) flag=--linux ;;
  *) echo "unknown platform $platform" >&2; exit 1 ;;
esac

if [ -n "${LAUNCHER_SRC:-}" ]; then
  src="$(cd "$LAUNCHER_SRC" && pwd)"
else
  src="${RUNNER_TEMP:-$root/target}/neoomsi-launcher-src"
  rm -rf "$src"
  git clone --quiet --depth 1 --branch main https://github.com/neoOMSI/launcher.git "$src"
fi

cd "$src"
pnpm_version="$(node -p "require('./package.json').packageManager.split('@')[1]")"
pnpm() { npx --yes "pnpm@$pnpm_version" "$@"; }
pnpm install --frozen-lockfile
pnpm build
out="$src/release/$platform-$arch"
rm -rf "$out"
pnpm exec electron-builder --dir "$flag" "--$arch" -c.directories.output="$out"

cd "$root"
case "$platform" in
  macos)
    app=dist/macos/neoOMSI.app
    dest="$app/Contents/Resources/launcher"
    rm -rf "$dest"
    mkdir -p "$dest"
    ditto "$(ls -d "$out"/mac*/"neoOMSI Launcher.app")" "$dest/neoOMSI Launcher.app"
    codesign --force --deep --sign - "$app"
    ;;
  *)
    dest="dist/$platform/launcher"
    rm -rf "$dest"
    cp -R "$(ls -d "$out"/"${flag#--}"-*unpacked)" "$dest"
    ;;
esac
echo "launcher $(git -C "$src" rev-parse --short HEAD 2>/dev/null || echo "(local)") -> $dest"
