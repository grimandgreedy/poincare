#!/usr/bin/env bash
set -euo pipefail

profile="${1:-release}"
target_triple="${2:-x86_64-unknown-linux-gnu}"
case "$profile" in
  debug|release) ;;
  *)
    echo "Usage: $0 [debug|release] [target-triple]" >&2
    exit 1
    ;;
esac

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
target_dir="$repo_root/target/$target_triple"
binary_path="$target_dir/$profile/poincare"
version="$(grep '^version = ' "$repo_root/crates/poincare-app/Cargo.toml" | head -1 | sed 's/version = "\(.*\)"/\1/')"
appdir="$target_dir/$profile/Poincare.AppDir"
linuxdeploy="${LINUXDEPLOY:-$repo_root/target/linuxdeploy-x86_64.AppImage}"

if [[ "$target_triple" != "x86_64-unknown-linux-gnu" ]]; then
  echo "AppImage packaging currently supports x86_64-unknown-linux-gnu only." >&2
  exit 1
fi

if [[ ! -x "$binary_path" ]]; then
  echo "Missing binary: $binary_path" >&2
  exit 1
fi

if [[ ! -x "$linuxdeploy" ]]; then
  echo "Missing linuxdeploy executable: $linuxdeploy" >&2
  exit 1
fi

rm -rf "$appdir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/applications" "$appdir/usr/share/icons/hicolor/512x512/apps"

cp "$binary_path" "$appdir/usr/bin/poincare"
cp "$repo_root/crates/poincare-app/assets/icon_512.png" "$appdir/usr/share/icons/hicolor/512x512/apps/poincare.png"

cat >"$appdir/usr/share/applications/poincare.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Poincare
Comment=Interactive 3D mathematical graphing application
Exec=poincare
Icon=poincare
Terminal=false
Categories=Education;Math;
DESKTOP

(
  cd "$target_dir/$profile"
  VERSION="$version" "$linuxdeploy" \
    --appdir "$appdir" \
    --desktop-file "$appdir/usr/share/applications/poincare.desktop" \
    --icon-file "$appdir/usr/share/icons/hicolor/512x512/apps/poincare.png" \
    --executable "$appdir/usr/bin/poincare" \
    --output appimage
)

generated="$(find "$target_dir/$profile" -maxdepth 1 -type f -name '*.AppImage' | head -1)"
if [[ -z "$generated" ]]; then
  echo "linuxdeploy did not create an AppImage" >&2
  exit 1
fi

output="$target_dir/$profile/Poincare-$version-linux-x86_64.AppImage"
mv "$generated" "$output"
chmod +x "$output"
echo "Created $output"
