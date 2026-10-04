#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail

export BUILD_PROFILE="${BUILD_PROFILE:-release}"
export VERSION
VERSION="$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["workspace"]["package"]["version"])')"
case "${RUST_TARGET}:${PACKAGE_ARCH}:${RPM_ARCH}" in
  x86_64-unknown-linux-gnu:amd64:x86_64|aarch64-unknown-linux-gnu:arm64:aarch64) ;;
  *) echo "Inconsistent Linux target/package architecture" >&2; exit 1 ;;
esac
go install github.com/goreleaser/nfpm/v2/cmd/nfpm@v2.47.0
nfpm="$(go env GOPATH)/bin/nfpm"
mkdir -p dist
"$nfpm" package -f packaging/nfpm.yaml -p deb -t "dist/ptouch_${VERSION}_${PACKAGE_ARCH}.deb"
"$nfpm" package -f packaging/nfpm.yaml -p rpm -t "dist/ptouch-${VERSION}.${RPM_ARCH}.rpm"
test "$(dpkg-deb --field "dist/ptouch_${VERSION}_${PACKAGE_ARCH}.deb" Architecture)" = "$PACKAGE_ARCH"
test "$(rpm -qp --queryformat '%{ARCH}' "dist/ptouch-${VERSION}.${RPM_ARCH}.rpm")" = "$RPM_ARCH"
extracted="$(mktemp -d)"
trap 'rm -rf "$extracted"' EXIT
dpkg-deb --extract "dist/ptouch_${VERSION}_${PACKAGE_ARCH}.deb" "$extracted"
for binary in ptouch ptouch-gui; do
  cmp "target/${RUST_TARGET}/${BUILD_PROFILE}/${binary}" "$extracted/usr/bin/$binary"
done
