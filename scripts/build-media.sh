#!/bin/bash
set -euo pipefail
exec bash "$(dirname "$0")/build-media-universal.sh" "$@"
