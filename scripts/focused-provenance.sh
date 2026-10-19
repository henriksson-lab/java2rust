#!/usr/bin/env bash

# Sourced by the focused runner; do not change the caller's shell options.
# A prebuilt profile label is caller-declared, never inferred from a filename.
focused_record_translation_producer() {
  local producer_binary="$1" snapshot_path="$2" profile="$3" profile_source="$4"
  local digest bytes
  if [[ ! "$profile" =~ ^[a-zA-Z0-9_.+-]+$ || ! "$profile_source" =~ ^[a-zA-Z0-9_.+-]+$ ]]; then
    echo "invalid translation producer profile label" >&2
    return 1
  fi
  digest="$(sha256sum < "$producer_binary")" || return 1
  digest="${digest%% *}"
  bytes="$(stat -Lc %s -- "$producer_binary")" || return 1
  {
    printf 'binary\t%s\n' "$producer_binary"
    printf 'sha256\t%s\n' "$digest"
    printf 'bytes\t%s\n' "$bytes"
    printf 'build_profile\t%s\n' "$profile"
    printf 'profile_source\t%s\n' "$profile_source"
  } > "$snapshot_path"
}

focused_load_translation_producer() {
  local snapshot_path="$1" key value
  translation_producer_binary=unknown
  translation_producer_sha256=unknown
  translation_producer_bytes=unknown
  translation_producer_build_profile=unknown
  translation_producer_profile_source=unknown-legacy
  [[ -f "$snapshot_path" ]] || return 0
  while IFS=$'\t' read -r key value; do
    case "$key" in
      binary) translation_producer_binary="$value" ;;
      sha256) translation_producer_sha256="$value" ;;
      bytes) translation_producer_bytes="$value" ;;
      build_profile) translation_producer_build_profile="$value" ;;
      profile_source) translation_producer_profile_source="$value" ;;
    esac
  done < "$snapshot_path"
  if [[ ! "$translation_producer_sha256" =~ ^[0-9a-f]{64}$ \
      || ! "$translation_producer_bytes" =~ ^[0-9]+$ \
      || ! "$translation_producer_build_profile" =~ ^[a-zA-Z0-9_.+-]+$ \
      || ! "$translation_producer_profile_source" =~ ^[a-zA-Z0-9_.+-]+$ \
      || "$translation_producer_binary" == unknown ]]; then
    echo "invalid translation producer snapshot: $snapshot_path" >&2
    return 1
  fi
}

focused_verify_translation_producer() {
  local producer_binary="$1" expected_digest="$2" digest
  digest="$(sha256sum < "$producer_binary")" || return 1
  if [[ "${digest%% *}" != "$expected_digest" ]]; then
    echo "translation producer changed during the run: $producer_binary" >&2
    return 1
  fi
}
