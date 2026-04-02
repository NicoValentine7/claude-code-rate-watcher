#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_PATH="$("${ROOT_DIR}/scripts/build_app.sh" release)"
ARCHIVE_NAME="${1:-claude-code-rate-watcher-macos-app.tar.gz}"
ARCHIVE_PATH="${ROOT_DIR}/${ARCHIVE_NAME}"

rm -f "${ARCHIVE_PATH}"
tar czf "${ARCHIVE_PATH}" -C "$(dirname "${APP_PATH}")" "$(basename "${APP_PATH}")"
echo "${ARCHIVE_PATH}"
