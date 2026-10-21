#!/usr/bin/env bash
# Shared manifest-to-upstream-module mapping. Callers validate the pinned checkout.
commons_rng_source_for_relative() {
  local checkout="$1" relative="$2" module
  case "$relative" in
    org/apache/commons/rng/core/*) module=commons-rng-core ;;
    org/apache/commons/rng/sampling/*) module=commons-rng-sampling ;;
    org/apache/commons/rng/*.java) module=commons-rng-client-api ;;
    *) echo "Unrecognized Commons RNG source path: $relative" >&2; return 1 ;;
  esac
  if [[ "$relative" == *..* ]]; then
    echo "Invalid Commons RNG source path: $relative" >&2
    return 1
  fi
  printf '%s/%s/src/main/java/%s\n' "$checkout" "$module" "$relative"
}
