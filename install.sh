#!/usr/bin/env bash
set -euo pipefail

REPO="${AGENTIM_INSTALL_REPO:-markbang/agentim}"
INSTALL_DIR="${AGENTIM_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${AGENTIM_VERSION:-latest}"
BASE_URL="https://github.com/${REPO}/releases"

log() {
  printf '%s\n' "$*"
}

fail() {
  printf 'Error: %s\n' "$*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

resolve_latest_tag() {
  local latest_url
  latest_url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "${BASE_URL}/latest" || true)"
  latest_url="${latest_url%%\?*}"
  if [[ -n "$latest_url" && "$latest_url" != "${BASE_URL}/latest" ]]; then
    basename "$latest_url"
    return 0
  fi

  curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
    | head -n 1
}

detect_asset_name() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux) os="linux" ;;
    Darwin) os="macos" ;;
    *)
      fail "unsupported operating system: ${os}. Use the release archives manually."
      ;;
  esac

  case "$arch" in
    x86_64|amd64) arch="x86_64" ;;
    arm64|aarch64) arch="aarch64" ;;
    *)
      fail "unsupported CPU architecture: ${arch}. Use the release archives manually."
      ;;
  esac

  case "${os}/${arch}" in
    linux/x86_64) printf '%s\n' "agentim-linux-x86_64.tar.gz" ;;
    macos/x86_64) printf '%s\n' "agentim-macos-x86_64.tar.gz" ;;
    macos/aarch64) printf '%s\n' "agentim-macos-aarch64.tar.gz" ;;
    linux/aarch64)
      fail "Linux aarch64 release archive is not published yet. Build from source for now."
      ;;
    *)
      fail "no release archive published for ${os}/${arch}"
      ;;
  esac
}

verify_checksum() {
  local archive_path checksum_path expected actual
  archive_path="$1"
  checksum_path="$2"

  if command -v sha256sum >/dev/null 2>&1; then
    (
      cd "$(dirname "$archive_path")"
      sha256sum -c "$(basename "$checksum_path")"
    )
    return 0
  fi

  expected="$(awk '{print $1}' "$checksum_path")"
  if command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "$archive_path" | awk '{print $1}')"
  elif command -v openssl >/dev/null 2>&1; then
    actual="$(openssl dgst -sha256 "$archive_path" | awk '{print $NF}')"
  else
    fail "no SHA256 verifier found (need sha256sum, shasum, or openssl)"
  fi

  [[ "$actual" == "$expected" ]] || fail "checksum verification failed"
}

install_binary() {
  local binary_path target_path
  binary_path="$1"
  target_path="${INSTALL_DIR}/agentim"

  mkdir -p "$INSTALL_DIR"
  if command -v install >/dev/null 2>&1; then
    install -m 755 "$binary_path" "$target_path"
  else
    cp "$binary_path" "$target_path"
    chmod 755 "$target_path"
  fi
}

main() {
  local tag archive_name checksum_name download_base tmpdir archive_path checksum_path binary_path

  require_cmd curl
  require_cmd tar
  require_cmd mktemp

  tag="$VERSION"
  if [[ "$tag" == "latest" ]]; then
    tag="$(resolve_latest_tag)"
    [[ -n "$tag" ]] || fail "failed to resolve the latest release tag"
  fi

  archive_name="$(detect_asset_name)"
  checksum_name="${archive_name}.sha256"
  download_base="${BASE_URL}/download/${tag}"
  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' EXIT

  archive_path="${tmpdir}/${archive_name}"
  checksum_path="${tmpdir}/${checksum_name}"

  log "Installing AgentIM ${tag}"
  log "Archive: ${archive_name}"
  log "Install dir: ${INSTALL_DIR}"

  curl -fsSL --retry 3 --proto '=https' --tlsv1.2 -o "$archive_path" "${download_base}/${archive_name}"
  curl -fsSL --retry 3 --proto '=https' --tlsv1.2 -o "$checksum_path" "${download_base}/${checksum_name}"

  verify_checksum "$archive_path" "$checksum_path"
  tar -xzf "$archive_path" -C "$tmpdir"

  binary_path="$(find "$tmpdir" -type f -name agentim -print -quit)"
  [[ -n "$binary_path" ]] || fail "agentim binary not found in extracted archive"

  install_binary "$binary_path"

  log "Installed: ${INSTALL_DIR}/agentim"
  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
      log "Add this to your shell profile if needed:"
      log "  export PATH=\"${INSTALL_DIR}:\$PATH\""
      ;;
  esac
}

main "$@"
