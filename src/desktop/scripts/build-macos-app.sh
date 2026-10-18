#!/usr/bin/env bash
# Build "Lumen Code.app" and a .dmg for macOS.
#
#   scripts/build-macos-app.sh [--arch universal|arm64|x86_64] [--skip-build] [--notarize]
#
# Output: dist/macos/<arch>/Lumen Code.app and dist/macos/LumenCode-<version>-macos-<arch>.dmg
#
# Signing follows the environment:
#   LUMEN_SIGN_IDENTITY   SHA-1 (or name) of a "Developer ID Application" identity in the
#                         keychain: hardened runtime, secure timestamp, camera entitlement.
#                         Unset: ad-hoc signature — runs on this Mac, but Gatekeeper refuses
#                         the same file downloaded through a browser.
#   LUMEN_NOTARY_PROFILE  with --notarize: a `xcrun notarytool store-credentials` profile.
#                         The app and the image are notarized and the tickets stapled.
#
# --skip-build repackages the binaries already in target/ (scripts/release-macos-local.sh
# builds the universal binary once, then packages each architecture from it).
set -euo pipefail

die() { echo "error: $*" >&2; exit 1; }
note() { echo "==> $*"; }

arch="universal"; skip_build=0; notarize=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --arch)       [[ $# -ge 2 ]] || die "--arch needs a value"; arch="$2"; shift 2 ;;
    --skip-build) skip_build=1; shift ;;
    --notarize)   notarize=1; shift ;;
    -h|--help)    sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//; $d'; exit 0 ;;
    *)            die "unknown argument: $1" ;;
  esac
done
[[ "$(uname -s)" == "Darwin" ]] || die "this script runs on macOS only (lipo, codesign, hdiutil)"
case "$arch" in
  universal) triples=(aarch64-apple-darwin x86_64-apple-darwin) ;;
  arm64)     triples=(aarch64-apple-darwin) ;;
  x86_64)    triples=(x86_64-apple-darwin) ;;
  *)         die "--arch must be universal, arm64 or x86_64" ;;
esac

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tools="$(cd "$root/../tools" && pwd)"
version="$(sed -n '/^\[package\]/,/^\[/ s/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$root/Cargo.toml" | head -1)"
[[ -n "$version" ]] || die "no version in Cargo.toml"
signed=0; [[ -n "${LUMEN_SIGN_IDENTITY:-}" ]] && signed=1
if (( notarize )); then
  (( signed )) || die "--notarize needs LUMEN_SIGN_IDENTITY: Apple notarizes Developer ID signatures only"
  [[ -n "${LUMEN_NOTARY_PROFILE:-}" ]] || die "--notarize needs LUMEN_NOTARY_PROFILE (xcrun notarytool store-credentials)"
fi

# ---------------------------------------------------------------- binary --
cd "$root"
if (( ! skip_build )); then
  for t in "${triples[@]}"; do
    rustup target add "$t" >/dev/null 2>&1 || true
    note "cargo build --release --target $t"
    MACOSX_DEPLOYMENT_TARGET=12.0 cargo build --release --locked --target "$t"
  done
fi
for t in "${triples[@]}"; do
  [[ -x "target/$t/release/lumen" ]] || die "no binary at target/$t/release/lumen (build without --skip-build)"
done

out="$root/dist/macos"
app="$out/$arch/Lumen Code.app"
rm -rf "${out:?}/${arch:?}"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
if (( ${#triples[@]} > 1 )); then
  slices=()
  for t in "${triples[@]}"; do slices+=("target/$t/release/lumen"); done
  lipo -create "${slices[@]}" -output "$app/Contents/MacOS/lumen"
else
  cp "target/${triples[0]}/release/lumen" "$app/Contents/MacOS/lumen"
fi
# the image must hold exactly the slices its name promises
# (no `case` inside $(...): macOS's bash 3.2 cannot parse it)
if [[ "$arch" == universal ]]; then want="arm64 x86_64"; else want="$arch"; fi
have="$(lipo -archs "$app/Contents/MacOS/lumen" | tr ' ' '\n' | sort | xargs)"
[[ "$have" == "$want" ]] || die "binary slices are '$have', expected '$want'"

# ---------------------------------------------------------------- bundle --
sed "s|@VERSION@|$version|g" "$root/packaging/macos/Info.plist.in" > "$app/Contents/Info.plist"
plutil -lint -s "$app/Contents/Info.plist" || die "Info.plist does not parse"
printf 'APPL????' > "$app/Contents/PkgInfo"
# the icon: the brand mark at every size an .icns holds
iconset="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$iconset"
for s in 16 32 128 256 512; do
  python3 "$tools/icon.py" "$s" "$iconset/icon_${s}x${s}.png"
  python3 "$tools/icon.py" "$((s * 2))" "$iconset/icon_${s}x${s}@2x.png"
done
iconutil --convert icns "$iconset" --output "$app/Contents/Resources/AppIcon.icns"

# ------------------------------------------------------------------ sign --
if (( signed )); then
  note "signing with $LUMEN_SIGN_IDENTITY (hardened runtime)"
  codesign --force --timestamp --options runtime \
    --entitlements "$root/packaging/macos/entitlements.plist" \
    --sign "$LUMEN_SIGN_IDENTITY" "$app"
else
  note "ad-hoc signature (local use only)"
  codesign --force --sign - "$app"
fi
codesign --verify --strict --verbose=1 "$app" || die "the signature does not verify"

notarize() {  # file → notarized (stapling is the caller's)
  local file="$1" out id status
  note "notarizing ${file##*/} (this takes a few minutes)"
  out="$(xcrun notarytool submit "$file" --keychain-profile "$LUMEN_NOTARY_PROFILE" --wait --output-format json)" ||
    die "notarytool submit failed for ${file##*/}: $out"
  id="$(plutil -extract id raw - <<<"$out")"
  status="$(plutil -extract status raw - <<<"$out")"
  if [[ "$status" != "Accepted" ]]; then
    xcrun notarytool log "$id" --keychain-profile "$LUMEN_NOTARY_PROFILE" >&2 || true
    die "Apple did not accept ${file##*/}: $status"
  fi
}

if (( notarize )); then
  zip="$(mktemp -d)/lumen-app.zip"
  ditto -c -k --keepParent "$app" "$zip"
  notarize "$zip"
  xcrun stapler staple "$app" >/dev/null || die "could not staple the ticket to the app"
fi

# ------------------------------------------------------------------- dmg --
dmg="$out/LumenCode-$version-macos-$arch.dmg"
rm -f "$dmg"
staging="$(mktemp -d)"
cp -R "$app" "$staging/"
ln -s /Applications "$staging/Applications"
# HFS+: an APFS image will not mount on older macOS; GitHub runners sometimes fail
# hdiutil create on the first try
for attempt in 1 2 3; do
  if hdiutil create -volname "Lumen Code" -srcfolder "$staging" -fs HFS+ -format UDZO -ov "$dmg" >/dev/null; then break; fi
  [[ $attempt -lt 3 ]] || die "hdiutil create failed three times"
  sleep 5
done
hdiutil verify -quiet "$dmg" || die "the image fails hdiutil verify"
if (( signed )); then
  codesign --force --timestamp --sign "$LUMEN_SIGN_IDENTITY" "$dmg"
fi
if (( notarize )); then
  notarize "$dmg"
  xcrun stapler staple "$dmg" >/dev/null || die "could not staple the ticket to the image"
  spctl --assess --type execute -v "$app" || die "Gatekeeper rejects the app"
  spctl --assess --type open --context context:primary-signature -v "$dmg" || die "Gatekeeper rejects the image"
fi
note "done: ${dmg#"$root"/}"
