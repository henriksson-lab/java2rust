# BioJava Core API summaries

The Alignment corpus depends on BioJava Core, which is not part of the 53-file
translation input. These Java declarations are summary-only dependency stubs:
they describe the generic contracts needed to type and translate Alignment but
contain no replacement implementations and are never emitted as corpus code.

Keep this boundary minimal. Add a declaration only when untouched Alignment
source uses it, and preserve the BioJava generic relationship instead of
introducing a generated-Rust patch or a class-name rule in the translator.

