#!/bin/bash
# cron-wrapper.sh — copy this to ~/.hermes/scripts/<service>.sh and edit
# the two paths below. Cron requires scripts to live under
# ~/.hermes/scripts/ and rejects absolute paths in the `cronjob` tool.
#
# Cron's PATH is bare; prepend ~/.local/bin so the monitor can find
# `hermes`, `uv`, `gh`, etc. that ship there.
set -euo pipefail

export PATH="/home/arctic/.local/bin:/usr/local/bin:/usr/bin:/bin:${PATH:-}"

# Adjust these to the actual monitor binary. --verbose is a good default
# for cron so the run log captures classifier decisions (great when
# debugging a misfire days later).
exec /home/arctic/.local/bin/<SERVICE>_monitor --verbose
