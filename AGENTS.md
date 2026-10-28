# Repository working rules

## Corpus feasibility and architectural planning

- For each new corpus, first establish a baseline and audit representative failing generated code before starting a fix campaign. If generation fails, diagnose that boundary first, then perform the rewrite audit as soon as output is available.
- Work out the smallest semantically correct Rust rewrites for representative failures, including the hardest ownership, aliasing, inheritance/generics and escaping-callback cases present. Trace the required changes through producers, storage, signatures and consumers; diagnostic counts alone are not a plan.
- Classify the required changes as local emission fixes, cross-function contract changes or representation/framework redesigns. Record concrete examples, expected rewrite scope, shared root causes and uncertainties, then choose coherent architectural batches from that evidence.
- Treat this audit as the entry gate for every new corpus: decide whether the current framework can express the required rewrites before committing to incremental fixes. If substantial redesign is needed, explain the evidence and proposed approach to the user upfront.
- Hypothetical generated-code corrections are diagnostic experiments only, never deliverables or substitutes for general translator fixes. Preserve Java semantics and keep pinned corpus sources untouched.
- Repeat the audit when new failure families appear or progress stalls. Surface substantial architectural gaps to the user instead of accumulating small fixes without a whole-path plan.

## Verification cadence

- Batch related translator fixes so that expensive compile and corpus-test cycles cover several coherent changes.
- While building a batch, use focused regression tests and cheap checks to catch failures early.
- Do not rerun the full real-world corpus after every individual fix. Rerun it after a substantial related batch, when a broad interaction needs verification, or when a fresh measurement is needed to choose the next work.
- Batching changes must not weaken correctness checks or replace focused regression coverage.

## Floating-point scope and prioritization

- Motivate floating-point corner-case work with actual production code paths and plausible workloads or a concrete downstream correctness need. An artificial stress input alone is not a reason to reproduce JVM-specific elementary-function rounding.
- Separate diagnostic stress results from acceptance requirements. Record unexplained differences, but do not automatically turn every extreme or ill-conditioned input into a blocking compatibility requirement.
- Preserve the distinctions between general arithmetic/runtime bugs, numerically significant behavior used by the corpus (including signed-zero branch cuts), and harmless implementation-dependent last-bit differences. Do not hide parsing, state, exception or algorithm bugs behind numerical tolerance.
