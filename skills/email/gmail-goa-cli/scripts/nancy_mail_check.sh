#!/bin/bash
# nancy_mail_check wrapper — cron needs scripts under ~/.hermes/scripts/ and
# a usable PATH + D-Bus session.
set -euo pipefail
export PATH="/home/arctic/.local/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
export DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$(id -u)/bus"
exec /home/arctic/.local/bin/nancy_mail_check
