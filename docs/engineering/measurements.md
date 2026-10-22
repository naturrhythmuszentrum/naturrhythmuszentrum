<!-- SPDX-License-Identifier: CC0-1.0 -->

# Recorded engineering measurements · `spec:nrz:engineering-measurements`

**Definition (Measurement identity)** · `def:nrz:engineering-measurement-identity`

Each observation retains its exact value, unit, UTC date, tested commit, executed command and source record. The recorded diagnostic concerns its named tree and environment; documentation generation validates and renders the input without running the observed commands (`[DOCGEN-rule:docgen:recorded-observations]`).

**Rule (Comparison scope)** · `rule:nrz:engineering-measurement-comparison`

Compare observations only after identifying the same operation, input, completion boundary and environment. A source declaration count differs from an executed test count, and a test-suite wall is not application latency. Missing observations remain absent rather than becoming fabricated values.

**Table (Dated observations)** · `tab:nrz:engineering-observed-runs`

|Observation|Metric|Value|Unit|Date|Commit|Command|Source|
|-----------|------|-----|----|----|------|-------|------|
|adopted-claimed-tests|Tests carrying claims|103|declarations|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`orchestration-linter coverage`|diagnostic 5 on the host; same seed and diagnostic patch; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|adopted-constants|Registered constants|29|constants|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`orchestration-linter check`|diagnostic 5 on the host; same seed and diagnostic patch; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|adopted-file-indexes|Current file indexes|26|indexes|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`orchestration-linter check`|diagnostic 5 on the host; same seed and diagnostic patch; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|adopted-matrix-folders|Current matrix folders|14|folders|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`orchestration-linter check`|diagnostic 5 on the host; same seed and diagnostic patch; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|docgen-check-peak-rss|Peak resident memory|195520|KiB|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`cargo run -p nrz-docgen -- check`|diagnostic 5 on sojka; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|docgen-check-wall|Process wall|1.30|s|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`cargo run -p nrz-docgen -- check`|diagnostic 5 on sojka; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|docgen-data-peak-rss|Peak resident memory|403452|KiB|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`cargo run -p nrz-docgen -- data`|diagnostic 5 on sojka; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|docgen-data-wall|Process wall|3.16|s|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`cargo run -p nrz-docgen -- data`|diagnostic 5 on sojka; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|docgen-render-peak-rss|Peak resident memory|34816|KiB|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`cargo run -p nrz-docgen -- render`|diagnostic 5 on sojka; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|docgen-render-wall|Process wall|0.28|s|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`cargo run -p nrz-docgen -- render`|diagnostic 5 on sojka; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|host-check-peak-rss|Peak resident memory|36076|KiB|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`orchestration-linter check`|diagnostic 5 on the host; same seed and diagnostic patch; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|host-check-wall|Process wall|3.48|s|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`orchestration-linter check`|diagnostic 5 on the host; same seed and diagnostic patch; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|host-coverage-peak-rss|Peak resident memory|40944|KiB|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`orchestration-linter coverage`|diagnostic 5 on the host; same seed and diagnostic patch; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|host-coverage-wall|Process wall|3.83|s|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`orchestration-linter coverage`|diagnostic 5 on the host; same seed and diagnostic patch; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|host-project-peak-rss|Peak resident memory|39244|KiB|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`orchestration-linter project --write`|diagnostic 5 on the host; same seed and diagnostic patch; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|host-project-wall|Process wall|2.83|s|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`orchestration-linter project --write`|diagnostic 5 on the host; same seed and diagnostic patch; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|strict-clippy-peak-rss|Peak resident memory|942352|KiB|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic -W clippy::nursery`|diagnostic 5 on sojka; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|strict-clippy-wall|Process wall|35.13|s|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic -W clippy::nursery`|diagnostic 5 on sojka; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|workspace-rustdoc-peak-rss|Peak resident memory|829412|KiB|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`|diagnostic 5 on sojka; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|workspace-rustdoc-wall|Process wall|27.50|s|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`|diagnostic 5 on sojka; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|workspace-test-declarations|Covered test declarations|1280|declarations|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`orchestration-linter coverage`|diagnostic 5 on the host; same seed and diagnostic patch; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|workspace-tests-failed|Failed test cases|0|tests|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`cargo test --workspace --no-fail-fast`|diagnostic 5 on sojka; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|workspace-tests-passed|Passed test cases|1383|tests|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`cargo test --workspace --no-fail-fast`|diagnostic 5 on sojka; includes the command's unit, integration and documentation tests; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|workspace-tests-peak-rss|Peak resident memory|2370916|KiB|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`cargo test --workspace --no-fail-fast`|diagnostic 5 on sojka; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|
|workspace-tests-wall|Process wall|154.58|s|2026-10-22|`c9cd0b2ac48b6fc92b9eecb59f2b8cb6d38daede`|`cargo test --workspace --no-fail-fast`|diagnostic 5 on sojka; seed 51105aeb4927f5e2d0bb76a607d87aae1d40c00b; diag-5.patch SHA-256 fd6f0bf7144ac9d808bd2bf3503d40812c9ec37842c508acee660c9f22247668|

**Definition (Catalogue currency)** · `def:nrz:engineering-measurement-currency`

Recorded observations stay attached to their historical tree when the current workspace changes. Catalogue source fingerprints instead require an explicit review of changed implementation sources; neither mechanism upgrades a historical observation into a current performance claim (`[DOCGEN-claim:docgen:engineering-source-drift-refuses]`), (`[DOCGEN-claim:docgen:engineering-render-rechecks-inputs]`).

**Definition (Delivery acceptance)** · `def:nrz:engineering-measurement-acceptance`

The delivery contract requires recorded provenance without an extra target-environment or performance-envelope gate; numerical acceptance and fresh-process comparison retain their stated correctness and tracked-error obligations (`dec:nrz:lc-resonance-delivery-contract`), (`step:nrz:lc-resonance-numerical-acceptance`), (`step:nrz:lc-resonance-record-bite`).

## Open intents · `sec:nrz:engineering-measurement-intents`

**Claim (Remaining logical cost ledgers)** · `claim:nrz:engineering-logical-cost-ledgers`

Bindings, matrix and CAS documentation will attach complete command/date/tree provenance to CAS production and complete-answer replay costs, per-region binding admission and exact-scale construction costs; symbolic lowering and numerical acceptance own their cumulative LC use (`step:nrz:lc-resonance-symbolic-lowering`), (`step:nrz:lc-resonance-numerical-acceptance`).

**Claim (Integrated LC cost record)** · `claim:nrz:engineering-integrated-lc-costs`

Numerical acceptance will record construction, replay, storage, runtime and memory by owner for the original mixed LC lifecycle, preserving shared graph/source identities and invariant-region reuse (`step:nrz:lc-resonance-numerical-acceptance`).

**Claim (Admitted-environment comparisons)** · `claim:nrz:engineering-environment-costs`

The record bite will repeat admitted physical computations in fresh processes/builds with arithmetic provenance and tracked-error comparison; the accuracy-and-benchmarks bite will compare their end-to-end error and cost (`step:nrz:lc-resonance-record-bite`), (`step:nrz:bite-accuracy-and-benchmarks`).
