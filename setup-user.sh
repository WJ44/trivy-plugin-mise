#!/usr/bin/env bash
# User-level setup for the harness environment.
# Installs Antigravity, mise, dev tools, and configures user dotfiles and agent settings.
# Must be run as a non-root dev user (e.g. 'vscode').
set -euo pipefail

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  echo "Usage: $(basename "$0")"
  echo
  echo "Installs user tools (Antigravity, mise, dev tools) and configures"
  echo "user dotfiles, git credentials, agent settings, and hooks."
  echo "Must be run as a non-root dev user (e.g. 'vscode')."
  exit 0
fi

if [[ $EUID -eq 0 ]]; then
  echo "Error: $(basename "$0") must not be run as root." >&2
  exit 1
fi

CURRENT_USER=$(id -un)
if [[ "$CURRENT_USER" == "noot" ]]; then
  echo "Error: $(basename "$0") must not be run as admin user 'noot'. Run as standard dev user (e.g. 'vscode')." >&2
  exit 1
fi

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_NAME=$(basename "$REPO_ROOT")

# Ensure user home directory has traversal permissions and workspace link
chmod 755 "$HOME"
if [[ -d "$REPO_ROOT" && ! -e "$HOME/$REPO_NAME" ]]; then
  ln -sfn "$REPO_ROOT" "$HOME/$REPO_NAME"
fi

# Antigravity CLI installation
curl -fsSL https://antigravity.google/cli/install.sh | bash

# put shim/ on PATH container-wide so gh and git credentials resolve to the bot
SHIM_LINE="export PATH=\"$REPO_ROOT/shim:\$PATH\""
grep -qxF "$SHIM_LINE" ~/.profile 2>/dev/null || echo "$SHIM_LINE" >>~/.profile
grep -qxF "$SHIM_LINE" ~/.bashrc 2>/dev/null || echo "$SHIM_LINE" >>~/.bashrc
export PATH="$REPO_ROOT/shim:$PATH"

# configure git identity and credential helper to act as the agent bot
SOCKET="/var/run/huddle/agent-token.sock"
if [[ ! -S "$SOCKET" ]] || ! token_json=$(curl -fsS --max-time 5 --unix-socket "$SOCKET" http://localhost/token); then
  echo "Error: agent token socket not reachable at $SOCKET." >&2
  echo "       Start host/agent-token-server.py on the host before running $(basename "$0")." >&2
  exit 1
fi

bot_login=$(jq -er '.login' <<<"$token_json")
bot_email=$(jq -er '.email' <<<"$token_json")
git config --global user.name "$bot_login"
git config --global user.email "$bot_email"
git config --global commit.gpgsign false
git config --global --unset-all credential.https://github.com.helper 2>/dev/null || true
git config --global --add credential.https://github.com.helper ""
git config --global --add credential.https://github.com.helper "!gh auth git-credential"

# configure claude settings
CLAUDE_SETTINGS_DIR="$REPO_ROOT/.claude"
CLAUDE_SETTINGS_FILE="$CLAUDE_SETTINGS_DIR/settings.local.json"

mkdir -p "$REPO_ROOT/memory" "$CLAUDE_SETTINGS_DIR"
if [ ! -f "$CLAUDE_SETTINGS_FILE" ]; then
  echo "{}" >"$CLAUDE_SETTINGS_FILE"
fi

tmp=$(mktemp)
jq \
  --arg dir "$REPO_ROOT/memory" \
  '
  ."$schema" = "https://json.schemastore.org/claude-code-settings.json" |
  .autoMemoryEnabled = true |
  .autoMemoryDirectory = $dir
  ' "$CLAUDE_SETTINGS_FILE" >"$tmp" && mv "$tmp" "$CLAUDE_SETTINGS_FILE"

# configure antigravity (agy) settings
AGY_SETTINGS_DIR="$HOME/.gemini/antigravity-cli"
AGY_SETTINGS_FILE="$AGY_SETTINGS_DIR/settings.json"

mkdir -p "$AGY_SETTINGS_DIR"
if [ ! -f "$AGY_SETTINGS_FILE" ]; then
  echo "{}" >"$AGY_SETTINGS_FILE"
fi

tmp=$(mktemp)
jq \
  --arg ws "$REPO_ROOT" \
  '
  .toolPermission = "always-proceed" |
  .trustedWorkspaces = ((.trustedWorkspaces // []) + [$ws] | unique)
  ' "$AGY_SETTINGS_FILE" >"$tmp" && mv "$tmp" "$AGY_SETTINGS_FILE"

# dev tools: mise installs the versions pinned in mise.toml and mise.lock
# (see agent-context/harness/formatting-and-linting.md)
MISE_VERSION=v2026.10.25
ARCH=$(uname -m)
case "$ARCH" in
x86_64) MISE_ARCH="linux-x64" ;;
aarch64 | arm64) MISE_ARCH="linux-arm64" ;;
*)
  echo "$(basename "$0"): unsupported architecture: $ARCH" >&2
  exit 1
  ;;
esac
MISE_ASSET="mise-$MISE_VERSION-$MISE_ARCH"
MISE_URL="https://github.com/jdx/mise/releases/download/$MISE_VERSION"
BIN_DIR="$HOME/.local/bin"

mkdir -p "$BIN_DIR"
tmp=$(mktemp -d)
# The attestation proves the binary was built by mise's own release workflow; a
# checksum file from the same release would only prove the download is intact.
curl -fsSL -o "$tmp/$MISE_ASSET" "$MISE_URL/$MISE_ASSET" &&
  gh attestation verify "$tmp/$MISE_ASSET" --repo jdx/mise >/dev/null &&
  install -m 755 "$tmp/$MISE_ASSET" "$BIN_DIR/mise" || exit 1

# Shims put the tools on PATH for login and interactive shells.
# shellcheck disable=SC2016
SHIMS_LINE='export PATH="$HOME/.local/share/mise/shims:$PATH"'
grep -qxF "$SHIMS_LINE" ~/.profile 2>/dev/null || echo "$SHIMS_LINE" >>~/.profile
grep -qxF "$SHIMS_LINE" ~/.bashrc 2>/dev/null || echo "$SHIMS_LINE" >>~/.bashrc
export PATH="$HOME/.local/share/mise/shims:$BIN_DIR:$PATH"

cd "$REPO_ROOT" || exit 1
mise trust mise.toml
mise install
lefthook install
