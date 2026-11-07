#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# extract-release-notes.sh — Extrait la section anglaise du CHANGELOG
# ==============================================================================
# Usage: ./scripts/extract-release-notes.sh 0.7.0 > RELEASE_NOTES.md
#        ./scripts/extract-release-notes.sh v0.7.0

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
CHANGELOG="${ROOT_DIR}/CHANGELOG.md"

VERSION="${1:-}"
if [[ -z "${VERSION}" ]]; then
    echo "usage: $0 <version>" >&2
    exit 2
fi
VERSION="${VERSION#v}"

if [[ ! -f "${CHANGELOG}" ]]; then
    echo "error: ${CHANGELOG} not found" >&2
    exit 1
fi

NOTES="$(
    awk -v ver="${VERSION}" '
        index($0, "## [" ver "]") == 1 { found = 1; next }
        found && substr($0, 1, 4) == "## [" { exit }
        found { print }
    ' "${CHANGELOG}" | sed -e '/./,$!d' -e :a -e '/^\n*$/{$d;N;ba' -e '}'
)"

if [[ -z "${NOTES}" ]]; then
    echo "error: no release notes found for ${VERSION} in ${CHANGELOG}" >&2
    exit 1
fi

printf '# Maschine Linux %s\n\n%s\n' "${VERSION}" "${NOTES}"
