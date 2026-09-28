#!/usr/bin/env bash
# venv-run.sh — invoke a script in a project-local venv while bypassing
# the system PYTHONPATH (which is polluted by ~/.hermes/hermes-agent/venv).
#
# Without `env -u PYTHONPATH`, Python on the desktop app inherits
# the hermes-agent venv's site-packages, which can conflict with newer
# or older bindings in the project venv (especially Python 3.11 vs 3.13
# C extensions like PIL._imaging).
#
# Usage:
#   ./scripts/venv-run.sh <project_venv_relative_to_home> <script_and_args...>
#
# Example:
#   ./scripts/venv-run.sh .hermes/scripts/hermes-times-v3/.venv/bin/python \\
#       .hermes/scripts/hermes-times-v3/render.py /tmp/manifest.json

set -euo pipefail

if [ "$#" -lt 2 ]; then
  echo "usage: $0 <project_venv_bin> <script_and_args...>" >&2
  echo "  e.g. $0 .hermes/scripts/hermes-times-v3/.venv/bin/python \\" >&2
  echo "          .hermes/scripts/hermes-times-v3/render.py /tmp/manifest.json" >&2
  exit 2
fi

VENV_BIN="$1"
shift

# Resolve to absolute path even if a tilde is given
case "$VENV_BIN" in
  ~*) VENV_BIN="$HOME${VENV_BIN#~}" ;;
esac

if [ ! -x "$VENV_BIN" ]; then
  echo "venv python not found or not executable: $VENV_BIN" >&2
  exit 1
fi

exec env -u PYTHONPATH "$VENV_BIN" "$@"
