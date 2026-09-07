#!/usr/bin/env bash
# Zegion cross-platform installer (Linux / macOS).
# Usage: curl -fsSL https://raw.githubusercontent.com/zakirkun/zegion/main/install.sh | bash
set -euo pipefail

REPO="zakirkun/zegion"
INSTALL_DIR="${ZEGION_INSTALL_DIR:-$HOME/.local/bin}"

detect_target() {
  local os arch
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"
  case "$os" in
    linux)  os="linux" ;;
    darwin) os="macos" ;;
    *) echo "Unsupported OS: $os" >&2; exit 1 ;;
  esac
  case "$arch" in
    x86_64|amd64)  arch="x86_64" ;;
    aarch64|arm64) arch="aarch64" ;;
    *) echo "Unsupported arch: $arch" >&2; exit 1 ;;
  esac
  echo "${os}-${arch}"
}

main() {
  local target version url tmp
  target="$(detect_target)"

  version="${ZEGION_VERSION:-}"
  if [ -z "$version" ]; then
    version="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)"
  fi
  if [ -z "$version" ]; then
    echo "Could not determine latest release." >&2
    exit 1
  fi

  echo "Installing Zegion ${version} for ${target} -> ${INSTALL_DIR}"
  url="https://github.com/${REPO}/releases/download/${version}/zegion-${target}.tar.gz"

  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  curl -fsSL "$url" -o "$tmp/zegion.tar.gz"
  tar -xzf "$tmp/zegion.tar.gz" -C "$tmp"

  mkdir -p "$INSTALL_DIR"
  install -m 0755 "$tmp/zegion" "$INSTALL_DIR/zegion"

  echo "Installed: $INSTALL_DIR/zegion"
  if ! echo ":$PATH:" | grep -q ":$INSTALL_DIR:"; then
    echo "NOTE: add $INSTALL_DIR to your PATH."
  fi
  echo "Run 'zegion onboard' to configure, then 'zegion run'."
}

main "$@"
