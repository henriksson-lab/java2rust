#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export RNG_ORACLE_FOCUS_SET="$repo_root/scripts/focus-sets/commons-rng-1.0-sampling-collections.txt"
export RNG_ORACLE_DRIVER="$repo_root/tests/corpus/rng/SamplingCollectionsOracle.java"
export RNG_ORACLE_OUT="${RNG_ORACLE_OUT:-$repo_root/.tmp/commons-rng-1.0-sampling-collections-java-oracle}"
exec bash "$repo_root/scripts/run-commons-rng-1.0-java-oracle.sh"
