#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NUMBERS_MODULE=complex exec bash "$repo_root/scripts/run-commons-numbers-1.3-contracts.sh" "$@"
