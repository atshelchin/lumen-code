#!/usr/bin/env bash
# Build, sign, notarize and upload the macOS packages of a desktop release — on THIS Mac,
# so the signing key never leaves it (nothing that can sign is stored on GitHub).
#
#   scripts/release-macos-local.sh v0.1.0            # build and verify, upload nothing
#   scripts/release-macos-local.sh v0.1.0 --upload   # ...then attach them to the release
#
# The tag is the one .github/workflows/release.yml created at the head of release/v0.1.0;
# CI built the Windows and Linux packages from that commit. This script adds the third:
# arm64, x86_64 and universal .dmg images from the same commit, signed with the Developer
# ID and notarized, plus SHA256SUMS-macos.
#
# Before compiling it checks, and says in words when it cannot go on, that:
#   * the checkout IS the tag, with no local changes under src/;
#   * the keychain holds a "Developer ID Application" identity of the team (the most
#     recently issued is taken; --identity SHA1 picks one);
#   * the notary credentials work (--notary-profile NAME; tried by default: lumen-notary,
#     then vela-notary — same Apple team);
#   * with --upload: the release exists and has no macOS images yet (--replace to overwrite).
#
# One-time setup, if no profile exists yet:
#   xcrun notarytool store-credentials lumen-notary --apple-id YOU@example.com --team-id F9W689P9NE
# It asks for an app-specific password (account.apple.com > Sign-In and Security).
set -euo pipefail

team="${LUMEN_TEAM_ID:-F9W689P9NE}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_root="$(git -C "$root" rev-parse --show-toplevel)"

die() { echo "error: $*" >&2; exit 1; }
note() { echo "==> $*"; }

tag=""; upload=0; replace=0; identity=""; notary=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --upload)         upload=1; shift ;;
    --replace)        replace=1; shift ;;
    --identity)       [[ $# -ge 2 ]] || die "--identity needs a SHA-1"; identity="$2"; shift 2 ;;
    --notary-profile) [[ $# -ge 2 ]] || die "--notary-profile needs a name"; notary="$2"; shift 2 ;;
    -h|--help)        sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//; $d'; exit 0 ;;
    v[0-9]*)          tag="$1"; shift ;;
    *)                die "unknown argument: $1 (expected the release's tag, e.g. v0.1.0)" ;;
  esac
done
[[ -n "$tag" ]] || die "which release? e.g. scripts/release-macos-local.sh v0.1.0"
[[ "$(uname -s)" == "Darwin" ]] || die "this script runs on macOS only"

# ------------------------------------------------------------ the source --
note "checking that this checkout is $tag"
tag_commit="$(git -C "$repo_root" rev-parse -q --verify "refs/tags/$tag^{commit}" 2>/dev/null)" ||
  die "no such tag here: $tag (git fetch --tags?)"
[[ "$(git -C "$repo_root" rev-parse HEAD)" == "$tag_commit" ]] ||
  die "HEAD is not $tag. The macOS images must come from the same commit as the other
       packages:  git checkout $tag"
[[ -z "$(git -C "$repo_root" status --porcelain -- src)" ]] ||
  die "src/ has local changes; a release is built from the tag, exactly"
version="$(sed -n '/^\[package\]/,/^\[/ s/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$root/Cargo.toml" | head -1)"
[[ "v$version" == "$tag" ]] || die "src/desktop/Cargo.toml says $version but the tag is $tag"

# ---------------------------------------------------------- the identity --
valid="$(security find-identity -v -p codesigning | awk -v t="($team)" '/Developer ID Application/ && index($0, t) {print $2}')"
[[ -n "$valid" ]] || die "no 'Developer ID Application' identity of team $team in the keychain.
       An 'Apple Development' or 'Apple Distribution' certificate cannot sign a download."
if [[ -n "$identity" ]]; then
  grep -qx "$identity" <<<"$valid" || die "$identity is not a valid Developer ID Application identity of team $team"
else
  # the most recently issued certificate: renewed ones (and ones issued after the team
  # was renamed) sit beside the older ones, all valid at once
  best=0
  while read -r sha; do
    start="$(security find-certificate -a -Z -p | awk -v h="$sha" '/^SHA-1 hash:/ {on = ($3 == h); next} on {print} on && /END CERTIFICATE/ {exit}' |
      openssl x509 -noout -startdate 2>/dev/null | sed 's/^notBefore=//')"
    [[ -n "$start" ]] || continue
    epoch="$(date -j -f "%b %e %T %Y %Z" "$start" +%s 2>/dev/null || echo 0)"
    if (( epoch > best )); then best="$epoch"; identity="$sha"; fi
  done <<<"$valid"
  [[ -n "$identity" ]] || die "could not read the issue date of any Developer ID certificate"
fi
signer="$(security find-identity -v -p codesigning | awk -v h="$identity" '$2 == h' | sed -E 's/^[^"]*"([^"]*)".*/\1/' | head -1)"
note "signing as: $signer ($identity)"

# ------------------------------------------------------------ the notary --
if [[ -z "$notary" ]]; then
  for candidate in lumen-notary vela-notary; do
    if xcrun notarytool history --keychain-profile "$candidate" >/dev/null 2>&1; then notary="$candidate"; break; fi
  done
  [[ -n "$notary" ]] || die "no working notary credentials (tried lumen-notary, vela-notary). Once, on this Mac:
         xcrun notarytool store-credentials lumen-notary --apple-id YOU@example.com --team-id $team"
else
  xcrun notarytool history --keychain-profile "$notary" >/dev/null 2>&1 || die "the notary profile '$notary' does not work"
fi
note "notary credentials: $notary"

# ----------------------------------------------------------- the release --
if (( upload )); then
  command -v gh >/dev/null 2>&1 || die "--upload needs the GitHub CLI (gh)"
  existing="$(cd "$repo_root" && gh release view "$tag" --json assets -q '.assets[].name' 2>/dev/null)" ||
    die "there is no release for $tag yet. release.yml creates it when release/$tag is pushed;
       wait for that run, then run this again."
  if grep -qE '\.dmg$|^SHA256SUMS-macos$' <<<"$existing" && (( ! replace )); then
    die "$tag already has macOS images. Replacing published files changes their checksums
       under people who downloaded them; pass --replace if you mean it."
  fi
fi

# ------------------------------------------------------------- the build --
export LUMEN_SIGN_IDENTITY="$identity" LUMEN_NOTARY_PROFILE="$notary"
rm -rf "$root/dist/macos"
"$root/scripts/build-macos-app.sh" --arch universal --notarize
"$root/scripts/build-macos-app.sh" --arch arm64 --skip-build --notarize
"$root/scripts/build-macos-app.sh" --arch x86_64 --skip-build --notarize

cd "$root/dist/macos"
shasum -a 256 ./*.dmg | sed 's|\./||' > SHA256SUMS-macos
cat SHA256SUMS-macos

if (( upload )); then
  note "attaching to $tag"
  clobber=(); (( replace )) && clobber=(--clobber)
  (cd "$repo_root" && gh release upload "$tag" "$root"/dist/macos/*.dmg "$root/dist/macos/SHA256SUMS-macos" ${clobber[@]+"${clobber[@]}"})
  note "done: $(cd "$repo_root" && gh release view "$tag" --json url -q .url)"
else
  note "built and verified; nothing uploaded (add --upload)"
fi
