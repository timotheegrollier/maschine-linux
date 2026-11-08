#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# build-rpm.sh — Génération de paquet RPM pour Fedora / Nobara Linux
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${ROOT_DIR}"

VERSION="$(grep -m1 '^version =' Cargo.toml | cut -d '"' -f2)"
if [ ! -f "target/release/maschine-linux" ]; then
    echo "==> Compilation du binaire release (v${VERSION})..."
    cargo build --release -p maschine-app --no-default-features || cargo build --release --locked -p maschine-app
fi

OUT_DIR="${ROOT_DIR}/dist"
mkdir -p "${OUT_DIR}"

# Méthode 1 : cargo-generate-rpm (ne nécessite ni sudo ni rpmbuild)
if command -v cargo-generate-rpm >/dev/null 2>&1; then
    echo "==> Génération RPM via cargo-generate-rpm..."
    (cd crates/maschine-app && cargo generate-rpm --target-dir ../../target)
    find target/generate-rpm -name "maschine-linux-*.rpm" -exec cp {} "${OUT_DIR}/" \;
    echo "✓ RPM créé dans ${OUT_DIR}/"
    ls -la "${OUT_DIR}"/*.rpm
    exit 0
fi

# Méthode 2 : rpmbuild
if command -v rpmbuild >/dev/null 2>&1; then
    echo "==> Génération RPM via rpmbuild..."
    RPM_TOP="${ROOT_DIR}/target/rpmbuild"
    mkdir -p "${RPM_TOP}"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

    # Archive source
    TAR_NAME="maschine-linux-${VERSION}"
    git archive --format=tar.gz --prefix="${TAR_NAME}/" HEAD -o "${RPM_TOP}/SOURCES/${TAR_NAME}.tar.gz"

    cp packaging/rpm/maschine-linux.spec "${RPM_TOP}/SPECS/"

    rpmbuild --define "_topdir ${RPM_TOP}" \
             --define "version ${VERSION}" \
             -bb "${RPM_TOP}/SPECS/maschine-linux.spec"

    find "${RPM_TOP}/RPMS" -name "*.rpm" -exec cp {} "${OUT_DIR}/" \;
    echo "✓ RPM créé dans ${OUT_DIR}/"
    ls -la "${OUT_DIR}"/*.rpm
    exit 0
fi

echo "==> Ni 'cargo-generate-rpm' ni 'rpmbuild' n'a été détecté."
echo "    Installation recommandée de cargo-generate-rpm :"
echo "    cargo install cargo-generate-rpm"
echo "    ou installation de rpmbuild :"
echo "    sudo dnf install -y rpm-build"
exit 1
