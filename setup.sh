#!/usr/bin/env bash
# Bootstrap dispatcher for the harness environment.
#
# Delegates to setup-system.sh or setup-user.sh depending on whether
# system packages (root/sudo) or user dev tools (non-root) are being configured.
set -euo pipefail

usage() {
  cat <<EOF
Usage: $(basename "$0") <--system|--user>

Options:
  --system, system    Install system dependencies (gh, jq). Requires root or sudo.
  --user, user        Install user tools, agent settings, and hooks. Must run as non-root.
  -h, --help          Show this help message.
EOF
}

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

case "${1:-}" in
--system | system)
  shift
  exec "$REPO_ROOT/setup-system.sh" "$@"
  ;;
--user | user)
  shift
  exec "$REPO_ROOT/setup-user.sh" "$@"
  ;;
-h | --help)
  usage
  exit 0
  ;;
"")
  echo "Error: Missing required argument." >&2
  usage >&2
  exit 1
  ;;
*)
  echo "Error: Unrecognized option '$1'." >&2
  usage >&2
  exit 1
  ;;
esac
