#!/usr/bin/env bash
# Builds the Release macOS app and packs it into a .dmg (kb: decisions/open-source-release).
#
#   scripts/make-dmg.sh                       # unsigned (ad-hoc) dmg, local use
#   VERSION=0.2.0 scripts/make-dmg.sh         # version for CFBundleShortVersionString
#
# Signing and notarisation happen only when their inputs are present; no credential is ever
# stored in the repository or passed on the command line:
#   SIGN_IDENTITY   a "Developer ID Application: ..." identity in a keychain (`security
#                   find-identity -v -p codesigning`). Unset: the app stays ad-hoc signed.
#   NOTARY_PROFILE  a notarytool keychain profile (`xcrun notarytool store-credentials NAME`).
#   NOTARY_KEY_PATH, NOTARY_KEY_ID, NOTARY_ISSUER
#                   an App Store Connect API key (.p8 file path, key ID, issuer ID); CI uses
#                   these (.github/workflows/release.yml), mirroring saturnus's desktop.yml.
# Notarisation needs SIGN_IDENTITY and one of the two credential sets; then the app is
# notarised and stapled, the .dmg around it signed, notarised and stapled, and both are
# checked the way Gatekeeper will.
#
# Prerequisites: `make rust` and `make project` (the Makefile's `dmg` target runs them).
# Output: dist/almondbread_<version>_aarch64.dmg
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"

VERSION="${VERSION:-$(sed -n 's/^ *MARKETING_VERSION: *"\(.*\)"$/\1/p' project.yml | head -n 1)}"
BUILD_NUMBER="${BUILD_NUMBER:-1}"
if ! [[ "$VERSION" =~ ^[0-9]+(\.[0-9]+){0,2}$ ]]; then
    echo "make-dmg: VERSION '$VERSION' is not of the form 1, 1.2 or 1.2.3" >&2
    exit 1
fi
DERIVED="$ROOT/.build/xcode-dmg"
APP="$DERIVED/Build/Products/Release/Almondbread.app"
DIST="$ROOT/dist"
DMG="$DIST/almondbread_${VERSION}_aarch64.dmg"
STAGE="$DIST/stage"

echo "make-dmg: building Almondbread $VERSION ($BUILD_NUMBER)"
rm -rf "$DERIVED/Build/Products/Release/Almondbread.app"
xcodebuild -project Almondbread.xcodeproj -scheme Almondbread -configuration Release \
    -destination 'platform=macOS,arch=arm64' -derivedDataPath "$DERIVED" -quiet \
    MARKETING_VERSION="$VERSION" CURRENT_PROJECT_VERSION="$BUILD_NUMBER" build

plist="$APP/Contents/Info.plist"
got="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$plist")"
if [ "$got" != "$VERSION" ]; then
    echo "make-dmg: the app says version '$got', expected '$VERSION'" >&2
    exit 1
fi

# Licence texts inside the bundle (the Inter font's OFL requires its licence to travel with
# the binary that embeds it; THIRD_PARTY_NOTICES.md covers the rest).
cp LICENSE AI_NOTICE THIRD_PARTY_NOTICES.md "$APP/Contents/Resources/"
cp crates/almondbread-render/src/font/OFL.txt "$APP/Contents/Resources/Inter-OFL.txt"

notary() {
    if [ -n "${NOTARY_KEY_PATH:-}" ]; then
        xcrun notarytool "$@" --key "$NOTARY_KEY_PATH" --key-id "$NOTARY_KEY_ID" \
            --issuer "$NOTARY_ISSUER"
    else
        xcrun notarytool "$@" --keychain-profile "$NOTARY_PROFILE"
    fi
}

# Submits $1 and waits; fails with the log unless Apple accepts it.
notarise() {
    local result status id
    result="$(notary submit "$1" --wait --timeout 30m --output-format json)" || true
    status="$(plutil -extract status raw -o - - <<<"$result" 2>/dev/null || true)"
    id="$(plutil -extract id raw -o - - <<<"$result" 2>/dev/null || true)"
    echo "make-dmg: notarisation of $(basename "$1"): ${status:-no status} (submission ${id:-none})"
    if [ "$status" != "Accepted" ]; then
        echo "$result" >&2
        [ -n "$id" ] && notary log "$id" >&2 || true
        exit 1
    fi
}

NOTARISE=false
if [ -n "${SIGN_IDENTITY:-}" ]; then
    if [ -n "${NOTARY_KEY_PATH:-}" ] && [ -n "${NOTARY_KEY_ID:-}" ] && [ -n "${NOTARY_ISSUER:-}" ]; then
        NOTARISE=true
    elif [ -n "${NOTARY_PROFILE:-}" ] && notary history >/dev/null 2>&1; then
        NOTARISE=true
    else
        echo "make-dmg: no notarisation credentials (NOTARY_PROFILE or NOTARY_KEY_*): signed only"
    fi
    echo "make-dmg: signing with the Developer ID identity (hardened runtime)"
    codesign --force --options runtime --timestamp --sign "$SIGN_IDENTITY" "$APP"
    codesign --verify --deep --strict --verbose=2 "$APP"
    if $NOTARISE; then
        zip="$DIST/Almondbread-notarise.zip"
        mkdir -p "$DIST"
        rm -f "$zip"
        ditto -c -k --keepParent "$APP" "$zip"
        notarise "$zip"
        rm -f "$zip"
        xcrun stapler staple "$APP"
        xcrun stapler validate "$APP"
        spctl --assess --type execute --verbose=2 "$APP"
    fi
else
    echo "make-dmg: SIGN_IDENTITY not set: ad-hoc signed, not notarised (local use only)"
    # Re-seal: the licence files were added after Xcode's ad-hoc signature.
    codesign --force --sign - "$APP"
    codesign --verify --deep --strict "$APP"
fi

rm -rf "$STAGE" "$DMG"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/Almondbread.app"
ln -s /Applications "$STAGE/Applications"
cp LICENSE "$STAGE/LICENSE.txt"
hdiutil create -quiet -volname "Almondbread $VERSION" -srcfolder "$STAGE" -fs HFS+ \
    -format UDZO -ov "$DMG"
rm -rf "$STAGE"

if [ -n "${SIGN_IDENTITY:-}" ]; then
    codesign --force --timestamp --sign "$SIGN_IDENTITY" "$DMG"
    if $NOTARISE; then
        notarise "$DMG"
        xcrun stapler staple "$DMG"
        xcrun stapler validate "$DMG"
        spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG"
    fi
fi
echo "make-dmg: $DMG"
