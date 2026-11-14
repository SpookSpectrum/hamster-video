#!/usr/bin/env python3
"""Compatibility entry point for strict Universal bundle verification."""
import json
import sys
from pathlib import Path
from macos_bundle import verify_bundle

if len(sys.argv) != 2:
    raise SystemExit('Usage: verify-universal.py /path/to/Hamster Video.app')
print(json.dumps(verify_bundle(Path(sys.argv[1]), 'universal'), indent=2))
