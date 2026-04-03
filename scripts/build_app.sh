#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONFIGURATION="${1:-release}"
APP_NAME="Claude Code Rate Watcher"
APP_DIR="${ROOT_DIR}/dist/${APP_NAME}.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"
FRAMEWORKS_DIR="${CONTENTS_DIR}/Frameworks"
EXECUTABLE_NAME="ccrw"
FEED_URL="${CCRW_SPARKLE_FEED_URL:-https://github.com/NicoValentine7/claude-code-rate-watcher/releases/latest/download/appcast.xml}"
PUBLIC_KEY="${CCRW_SPARKLE_PUBLIC_KEY:-}"

cd "${ROOT_DIR}"
swift build -c "${CONFIGURATION}" >&2
BIN_DIR="$(swift build -c "${CONFIGURATION}" --show-bin-path 2>/dev/null)"
EXECUTABLE_PATH="${BIN_DIR}/${EXECUTABLE_NAME}"
FRAMEWORK_PATH="${BIN_DIR}/Sparkle.framework"

rm -rf "${APP_DIR}"
mkdir -p "${MACOS_DIR}" "${RESOURCES_DIR}" "${FRAMEWORKS_DIR}"
cp "${EXECUTABLE_PATH}" "${MACOS_DIR}/${EXECUTABLE_NAME}"

if [[ -d "${FRAMEWORK_PATH}" ]]; then
  cp -R "${FRAMEWORK_PATH}" "${FRAMEWORKS_DIR}/"
  install_name_tool -add_rpath "@executable_path/../Frameworks" "${MACOS_DIR}/${EXECUTABLE_NAME}" 2>/dev/null || true
fi

if [[ -f "${ROOT_DIR}/assets/AppIcon.icns" ]]; then
  cp "${ROOT_DIR}/assets/AppIcon.icns" "${RESOURCES_DIR}/AppIcon.icns"
fi

cat > "${CONTENTS_DIR}/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleExecutable</key>
  <string>${EXECUTABLE_NAME}</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>CFBundleIdentifier</key>
  <string>com.claude-code-rate-watcher</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.11.0</string>
  <key>CFBundleVersion</key>
  <string>0.11.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>SUPublicEDKey</key>
  <string>${PUBLIC_KEY}</string>
  <key>SUFeedURL</key>
  <string>${FEED_URL}</string>
</dict>
</plist>
PLIST

chmod 755 "${MACOS_DIR}/${EXECUTABLE_NAME}"
echo "${APP_DIR}"
