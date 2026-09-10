#!/usr/bin/env bash
set -euo pipefail

if [[ -e "${V06B_TRANSPORT_DISABLED:?}" ]]; then
    exit 1
fi

exec /usr/libexec/at-spi-bus-launcher
