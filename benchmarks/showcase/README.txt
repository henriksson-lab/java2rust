REPRODUCIBLE JAVA / GENERATED RUST SHOWCASE BENCHMARKS

Run from the repository root. Requires Python 3, GNU /usr/bin/time, javac/java,
rustc/cargo, and offline Rust dependencies. The Commons Statistics goal stays
paused. These runners neither run the translator nor modify Java sources or
generated Rust bodies.

Reproduce the workloads (each output directory must be fresh):

  python3 scripts/benchmark-showcase.py varscan --out /tmp/varscan-showcase-new
  python3 scripts/benchmark-showcase.py jahmm --out /tmp/jahmm-showcase-new

Defaults:
  VarScan: 100,000 pileup positions.
  Jahmm: 16 states, 5 observation symbols, 4,000 observations per sequence,
         32 sequences (128,000 observations in total).

Override workload sizes with --positions, or --states / --length / --sequences.
For example: --states 8 --length 1000 --sequences 16.

Source paths default to the original trees under
/home/mahogny/github/claude/java2rust/testdata. Frozen translations default to
.tmp/varscan-full-v5 and .tmp/jahmm-full-v122. Override with --source-root and
--translation-root. Use --target-dir to retain a separate Cargo build cache.

Each configuration runs once in a fresh process: default Java, release Rust,
then Java with -Xms32m -Xmx256m. Use --java-heap none to skip the additional
capped-heap run, or --java-heap 512m to change it. The cap limits the Java heap,
not total process RSS. Builds finish before timing. Startup/JIT, allocation,
file I/O and output are included; no CPU affinity or core limit is imposed.
These runs estimate performance, not statistical stability or minimum memory.

VarScan
The Rust entrypoint calls the generated VarScan::main. Input repeats four
single-sample, 10x pileup patterns at unique ordered positions: reference,
heterozygous SNP, high-alternate SNP and low-quality filtering. All 18 original
production Java files and the complete archived Rust crate are compiled.
The exact command and flags are recorded with stdout, stderr, times and hashes.

Jahmm
This is a library inference workload. Both callers construct identical positive,
normalized discrete HMM parameters and deterministic LCG observations, then
invoke ViterbiCalculator and ForwardBackwardScaledCalculator. The callers do
not implement inference algorithms. Every decoded state and the raw IEEE bits
of both log-likelihoods are printed. The runner reports exact agreement or
absolute/ULP differences, without silently applying a numerical tolerance.
All 95 original production Java files and the complete archived Rust crate are
compiled; only the measured operations are behaviorally validated. The old
summary records four no-op facade call sites, so this is not whole-API acceptance.

Provenance and limitations
All archived raw modules must match their translator manifest FNV hashes
(18/18 or 95/95). SHA-256 hashes record sources, generated inputs, callers,
runner and executable. Inputs must remain unchanged during execution.
These old summaries lack an immutable translator-producer binary SHA. Results
are frozen-artifact runtime evidence, not fresh regeneration with the current
unfinished translator. Crate bodies are copied unchanged; only separate example
entrypoints are added to the copies.

results.json and provenance.json preserve commands, metrics, parity and hashes.
*.stdout / *.stderr preserve observations; *.time contains GNU time wall, user
and system seconds and maximum RSS in KiB. MiB = KiB / 1024.
Exit 0 means measured stdout parity; exit 2 means an output/FP difference.
Stderr parity and capped-run parity are reported separately. Build/runtime
errors return nonzero and retain logs.

Saved measurements
  .tmp/showcase-varscan-20261025/       original 100,000-position measurement
  .tmp/showcase-jahmm-20261025/         original 16 x 4,000 x 32 measurement
  .tmp/showcase-varscan-runner-smoke/  1,000-position runner verification

The main result directories include reproduction-inputs.tar.gz containing Java,
frozen raw/assembled Rust, the local ndarray shim, manifests, runner and callers.
They contain no build targets. Reproduce independently of old temporary paths:

  mkdir /tmp/jahmm-showcase-inputs
  tar -xzf .tmp/showcase-jahmm-20261025/reproduction-inputs.tar.gz \
    -C /tmp/jahmm-showcase-inputs
  python3 /tmp/jahmm-showcase-inputs/scripts/benchmark-showcase.py jahmm \
    --source-root /tmp/jahmm-showcase-inputs/source \
    --translation-root /tmp/jahmm-showcase-inputs/translation \
    --out /tmp/jahmm-showcase-reproduced

For VarScan, use its archive and corpus name with different extraction/output
paths. Preserve these bundles with the showcase if old .tmp directories are
cleaned. Archived results include the exact workload and observed parity scope.
