#!/bin/bash
set -euo pipefail
exec bash "$(dirname "$0")/build-universal-macos.sh" "$@"
