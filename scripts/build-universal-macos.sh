#!/bin/bash
# Compatibility entry point: stage a Universal app using the shared builder.
# Signing and DMG packaging are a separate release step.
set -euo pipefail
tools_dir="${1:?Pass a Universal Resources/tools directory}"
shift
exec bash "$(dirname "$0")/build-macos.sh" --arch universal --tools "$tools_dir" "$@"
