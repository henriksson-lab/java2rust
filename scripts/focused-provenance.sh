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

# Capture runtime code from the actual producer, then reuse only that verified
# snapshot during assembly resumes (which may use another validator binary).
focused_emit_runtime_support() {
  local producer="$1" producer_digest="$2" support_kind="$3" support_path="$4"
  local metadata="$support_path.provenance.tsv" key value digest
  local recorded_producer="" recorded_support="" recorded_kind="" temporary
  if [[ ! "$support_kind" =~ ^[a-z0-9-]+$ || ! "$producer_digest" =~ ^[a-f0-9]{64}$ ]]; then
    echo "runtime support requires known translation producer provenance" >&2
    return 1
  fi
  if [[ -f "$support_path" && -f "$metadata" ]]; then
    while IFS=$'\t' read -r key value; do
      case "$key" in
        producer_sha256) recorded_producer="$value" ;;
        support_sha256) recorded_support="$value" ;;
        kind) recorded_kind="$value" ;;
      esac
    done < "$metadata"
    digest="$(sha256sum < "$support_path")" || return 1
    if [[ "$recorded_producer" != "$producer_digest" || "$recorded_kind" != "$support_kind" \
          || "$recorded_support" != "${digest%% *}" ]]; then
      echo "runtime support snapshot provenance mismatch: $support_path" >&2
      return 1
    fi
  else
    focused_verify_translation_producer "$producer" "$producer_digest" || return 1
    temporary="$(mktemp "$support_path.pending.XXXXXX")" || return 1
    if ! "$producer" emit-runtime-support "$support_kind" > "$temporary"; then
      echo "runtime export failed; use a matching producer/packager (partial output: $temporary)" >&2
      return 1
    fi
    focused_verify_translation_producer "$producer" "$producer_digest" || return 1
    digest="$(sha256sum < "$temporary")" || return 1
    mv -- "$temporary" "$support_path" || return 1
    {
      printf 'producer_sha256\t%s\n' "$producer_digest"
      printf 'support_sha256\t%s\n' "${digest%% *}"
      printf 'kind\t%s\n' "$support_kind"
    } > "$metadata" || return 1
  fi
  cat "$support_path"
}
