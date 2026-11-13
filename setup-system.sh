#!/usr/bin/env bash
# System-level setup for the harness environment.
# Installs system dependencies (gh, jq) requiring root or sudo privileges.
set -euo pipefail

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  echo "Usage: $(basename "$0")"
  echo
  echo "Installs system dependencies (gh, jq)."
  echo "Requires root or sudo privileges."
  exit 0
fi

SUDO=()
if [[ $EUID -ne 0 ]]; then
  if ! command -v sudo >/dev/null 2>&1; then
    echo "Error: $(basename "$0") requires root or sudo privileges, but 'sudo' was not found." >&2
    exit 1
  fi
  if ! sudo -n true 2>/dev/null && ! sudo -v; then
    echo "Error: $(basename "$0") requires sudo privileges." >&2
    exit 1
  fi
  SUDO=(sudo)
fi

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_NAME=$(basename "$REPO_ROOT")

# Ensure /home/vscode has directory traversal permissions and links to the workspace
if [[ -d /home/vscode ]]; then
  "${SUDO[@]}" chmod 755 /home/vscode
  if [[ -d "$REPO_ROOT" && ! -e "/home/vscode/$REPO_NAME" ]]; then
    "${SUDO[@]}" ln -sfn "$REPO_ROOT" "/home/vscode/$REPO_NAME"
  fi
fi
if [[ -d /home/noot ]]; then
  if [[ -d "$REPO_ROOT" && ! -e "/home/noot/$REPO_NAME" ]]; then
    "${SUDO[@]}" ln -sfn "$REPO_ROOT" "/home/noot/$REPO_NAME"
  fi
fi

# Add GitHub CLI apt repository and install system dependencies
"${SUDO[@]}" mkdir -p -m 755 /etc/apt/keyrings
out=$(mktemp)
trap 'rm -f "$out"' EXIT
wget -nv -O"$out" https://cli.github.com/packages/githubcli-archive-keyring.gpg
cat "$out" | "${SUDO[@]}" tee /etc/apt/keyrings/githubcli-archive-keyring.gpg >/dev/null
"${SUDO[@]}" chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | "${SUDO[@]}" tee /etc/apt/sources.list.d/github-cli.list >/dev/null
"${SUDO[@]}" apt update

"${SUDO[@]}" apt install gh jq -y
