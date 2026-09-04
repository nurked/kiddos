#!/bin/sh
# release.sh: the macOS build, a universal (Apple silicon + Intel) app,
# Developer ID-signed, notarized and stapled when the identity and the
# notarytool profile are on this Mac; ad-hoc signed otherwise.
#
#   tools/release.sh            -> dist/KidDOS-macos.zip
#
# One-time setup (same as the other apps on this Mac):
#   - "Developer ID Application" cert in the keychain (Xcode -> Accounts)
#   - xcrun notarytool store-credentials <profile> --apple-id ... \
#       --team-id <TEAM> --password <app-specific password>
# The identity is found in the keychain; the profile name is read from
# NOTARY_PROFILE or the gitignored file tools/.notary-profile.
# SKIP_NOTARIZE=1 signs but does not submit.
#
# Linux and Windows: tools/release-docker.sh.
set -e
cd "$(dirname "$0")/.."
VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
rustup target add aarch64-apple-darwin x86_64-apple-darwin > /dev/null 2>&1 || true
cargo build --release -p kiddos --target aarch64-apple-darwin
cargo build --release -p kiddos --target x86_64-apple-darwin
rm -rf dist/macos && mkdir -p dist/macos
APP=dist/macos/KidDOS.app
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create target/aarch64-apple-darwin/release/kiddos target/x86_64-apple-darwin/release/kiddos -output "$APP/Contents/MacOS/kiddos"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>KidDOS</string>
  <key>CFBundleDisplayName</key><string>KidDOS</string>
  <key>CFBundleIdentifier</key><string>org.kiddos.app</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleExecutable</key><string>kiddos</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.education</string>
</dict>
</plist>
PLIST
cp docs/PARENTS.md dist/macos/READ-ME-FIRST-PARENTS.md
ZIP=dist/KidDOS-macos.zip
IDENTITY="${SIGN_IDENTITY:-$(security find-identity -v -p codesigning 2>/dev/null | grep -o '"Developer ID Application: [^"]*"' | head -1 | tr -d '"')}"
# the notarytool keychain profile name lives outside git: tools/.notary-profile
PROFILE="${NOTARY_PROFILE:-$(cat tools/.notary-profile 2>/dev/null || true)}"
if [ -n "$IDENTITY" ]; then
  echo ">>> signing with: $IDENTITY"
  codesign --force --deep --options runtime --timestamp \
    --entitlements tools/macos-entitlements.plist --sign "$IDENTITY" "$APP"
  codesign --verify --deep --strict "$APP"
  ditto -c -k --keepParent "$APP" "$ZIP"
  if [ -z "${SKIP_NOTARIZE:-}" ] && [ -z "$PROFILE" ]; then
    echo ">>> no notarytool profile (NOTARY_PROFILE or tools/.notary-profile): signed but not notarized"
  elif [ -z "${SKIP_NOTARIZE:-}" ]; then
    echo ">>> notarizing with profile $PROFILE (usually 1-5 min)"
    xcrun notarytool submit "$ZIP" --keychain-profile "$PROFILE" --wait
    echo ">>> stapling"
    xcrun stapler staple "$APP"
    spctl --assess --type execute -vv "$APP"
  fi
else
  echo ">>> no Developer ID identity found: ad-hoc signature (right-click > Open on first run)"
  codesign --force --deep --sign - "$APP" 2>/dev/null || true
fi
( cd dist/macos && rm -f "../$(basename "$ZIP")" && zip -q -r -y "../$(basename "$ZIP")" KidDOS.app READ-ME-FIRST-PARENTS.md )
echo "$ZIP ($(du -h "$ZIP" | cut -f1)) v$VERSION universal ($(lipo -archs "$APP/Contents/MacOS/kiddos"))"
