#!/usr/bin/env sh
set -eu

REPO="${AGENTIM_REPO:-markbang/agentim}"
INSTALL_DIR="${AGENTIM_INSTALL_DIR:-${HOME}/.local/bin}"
VERSION="${AGENTIM_VERSION:-latest}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: required command not found: $1" >&2
    exit 1
  fi
}

need_cmd curl
need_cmd tar
need_cmd uname
need_cmd mktemp

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux) platform_os="linux" ;;
  Darwin) platform_os="macos" ;;
  *)
    echo "error: unsupported OS: $os" >&2
    exit 1
    ;;
esac

case "$arch" in
  x86_64|amd64) platform_arch="x86_64" ;;
  arm64|aarch64) platform_arch="aarch64" ;;
  *)
    echo "error: unsupported architecture: $arch" >&2
    exit 1
    ;;
esac

if [ "$VERSION" = "latest" ]; then
  VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' \
    | head -n 1)"
  if [ -z "$VERSION" ]; then
    echo "error: failed to resolve latest release tag from GitHub" >&2
    exit 1
  fi
fi

asset="agentim-${platform_os}-${platform_arch}.tar.gz"
checksum_asset="${asset}.sha256"
download_base="https://github.com/${REPO}/releases/download/${VERSION}"

tmpdir="$(mktemp -d)"
archive_path="${tmpdir}/${asset}"
checksum_path="${tmpdir}/${checksum_asset}"

cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT INT TERM

echo "==> Downloading ${asset} (${VERSION})"
curl -fsSL "${download_base}/${asset}" -o "$archive_path"
curl -fsSL "${download_base}/${checksum_asset}" -o "$checksum_path"

if command -v shasum >/dev/null 2>&1; then
  (
    cd "$tmpdir"
    shasum -a 256 -c "$checksum_asset"
  )
elif command -v sha256sum >/dev/null 2>&1; then
  (
    cd "$tmpdir"
    sha256sum -c "$checksum_asset"
  )
else
  echo "warning: no shasum/sha256sum found; skipping checksum verification" >&2
fi

mkdir -p "$INSTALL_DIR"
tar -xzf "$archive_path" -C "$tmpdir"

binary_path="$(find "$tmpdir" -type f -name agentim | head -n 1)"
if [ -z "$binary_path" ]; then
  echo "error: downloaded archive does not contain agentim binary" >&2
  exit 1
fi

install -m 0755 "$binary_path" "${INSTALL_DIR}/agentim"

echo "==> Installed agentim to ${INSTALL_DIR}/agentim"
echo
echo "Next steps:"
echo "  1. Make sure ${INSTALL_DIR} is on your PATH"
echo "  2. Log into Codex locally if needed"
echo "  3. Start AgentIM:"
echo "     agentim --telegram-token YOUR_TELEGRAM_BOT_TOKEN"
