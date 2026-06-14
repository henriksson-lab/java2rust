#!/usr/bin/env bash
# Regenerate golden fixtures in tests/corpus from the original java2rust.jar.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
jar="${JAVA2RUST_JAR:-$here/../java-to-rust/java2rust.jar}"

if [[ ! -f "$jar" ]]; then
  echo "java2rust.jar not found at $jar (set JAVA2RUST_JAR)" >&2
  exit 1
fi

javac -cp "$jar" -d "$here/tools" "$here/tools/GenGolden.java"
java -cp "$jar:$here/tools" GenGolden "$here/tests/corpus"
