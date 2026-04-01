#!/usr/bin/env sh
set -eu

REPO="GodPuffin/squid"
BIN_DIR="${HOME}/.local/bin"
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}:${ARCH}" in
  Linux:x86_64)
    ASSET_NAME="squid-x86_64-unknown-linux-gnu.tar.gz"
    ;;
  Linux:arm64|Linux:aarch64)
    ASSET_NAME="squid-aarch64-unknown-linux-gnu.tar.gz"
    ;;
  Darwin:x86_64)
    ASSET_NAME="squid-x86_64-apple-darwin.tar.gz"
    ;;
  Darwin:arm64|Darwin:aarch64)
    ASSET_NAME="squid-aarch64-apple-darwin.tar.gz"
    ;;
  *)
    echo "Unsupported platform: ${OS} ${ARCH}" >&2
    exit 1
    ;;
esac

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Missing required command: $1" >&2
    exit 1
  }
}

need_cmd curl
need_cmd tar
need_cmd mktemp
need_cmd install

LATEST_JSON="$(curl -fsSL -H "User-Agent: squid-installer" "https://api.github.com/repos/${REPO}/releases/latest")"
DOWNLOAD_URL="$(printf '%s' "$LATEST_JSON" | sed -n "s|.*\"browser_download_url\": *\"\\([^\"]*${ASSET_NAME}\\)\".*|\\1|p" | head -n 1)"

if [ -z "$DOWNLOAD_URL" ]; then
  echo "Could not find release asset ${ASSET_NAME}" >&2
  exit 1
fi

mkdir -p "$BIN_DIR"
TMP_DIR="$(mktemp -d)"
ARCHIVE_PATH="${TMP_DIR}/${ASSET_NAME}"

curl -fsSL "$DOWNLOAD_URL" -o "$ARCHIVE_PATH"
tar -xzf "$ARCHIVE_PATH" -C "$TMP_DIR"
install -m 0755 "${TMP_DIR}/squid" "${BIN_DIR}/squid"

case "${SHELL:-}" in
  */zsh)
    SHELL_RC="${HOME}/.zshrc"
    ;;
  */bash)
    SHELL_RC="${HOME}/.bashrc"
    ;;
  *)
    SHELL_RC="${HOME}/.profile"
    ;;
esac

PATH_LINE='export PATH="$HOME/.local/bin:$PATH"'
if ! printf '%s' ":$PATH:" | grep -q ":${BIN_DIR}:"; then
  if [ -f "$SHELL_RC" ]; then
    if ! grep -Fq "$PATH_LINE" "$SHELL_RC"; then
      printf '\n%s\n' "$PATH_LINE" >> "$SHELL_RC"
    fi
  else
    printf '%s\n' "$PATH_LINE" > "$SHELL_RC"
  fi
  PATH_MESSAGE="Added ${BIN_DIR} to PATH in ${SHELL_RC}. Restart your shell if needed."
else
  PATH_MESSAGE="${BIN_DIR} is already on PATH."
fi

rm -rf "$TMP_DIR"

echo "Installed squid to ${BIN_DIR}/squid"
echo "$PATH_MESSAGE"
echo "Run: squid path/to/database.sqlite"
