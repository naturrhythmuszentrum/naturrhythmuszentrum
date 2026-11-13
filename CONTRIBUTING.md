<!-- SPDX-License-Identifier: CC0-1.0 -->

# Contributing to Naturrhythmuszentrum · `guide:nrz:contributing`

## Commits · `sec:nrz:contributing-commits`

**Rule (Coherent main commits)** · `rule:nrz:coherent-main-commits`

A commit on main is one meaningful unit of work: one coherent change that is reviewable alone and green alone. It is neither a fragment, a mixture of unrelated work, nor a repair needed to make the preceding commit acceptable. Tests accompany the behavior they validate. An indivisible change across packages remains together when splitting it would make intermediate commits incoherent. This preserves a history whose boundaries explain and verify the change independently (`rule:vision:meaningful-commit-series`), (`prin:vision:reviewable-boundaries`).

**Rule (Contributions from other platforms)** · `rule:nrz:other-platform-contributions`

Bug reports and patches from other platforms, such as WebAssembly or aarch64, are welcome when they make the code more robust across platforms. Submit them through the repository's usual contribution route, with their own tests and the same review and verification requirements (`rule:nrz:coherent-main-commits`), (`rule:nrz:required-verification`). The accompanying tests run and pass under the gate of record on x86_64 Linux; behaviour only observable on the reporting platform is recorded as unverified. Landing such a fix improves portability without widening official support: the claim remains x86_64 Linux only, and widening it requires test runs on the new target first under (`dec:vision:supported-platform`).

**Rule (Commit subject and body)** · `rule:nrz:commit-subject-and-body`

Each commit has one Conventional Commits subject of at most 72 characters in total and a non-empty body stating why the change is needed. The subject names the coherent change; the body supplies its justification so the reviewer can assess purpose as well as implementation. The gate checks subject syntax and body presence over every commit past the base (`gate:nrz:verification-of-candidate`).

The accepted subject forms are `type: description`, `type(scope): description`, `type!: description`, and `type(scope)!: description`. The optional scope contains one or more lowercase ASCII letters, digits, `.`, `_`, or `-`; the optional `!` immediately before the colon marks a breaking change. The colon is followed by one space and a description starting with a lowercase ASCII letter (`a` through `z`). The project's 72-character total limit includes the type, scope, punctuation, spaces, and description.

Use one of these nine types:

- `feat`: add engine or tooling capabilities.
- `fix`: correct faulty behavior.
- `docs`: change documentation, including measurement and dataset records; use the scope `engineering` for engineering records.
- `chore`: maintain repository housekeeping.
- `refactor`: restructure code without changing its behavior.
- `test`: add or correct tests and their supporting fixtures.
- `perf`: improve runtime or memory performance.
- `build`: change build configuration or dependencies.
- `style`: change code formatting or whitespace without changing behavior.

Generated renders travel with their cause under the cause's type (`rule:nrz:generated-artifacts-with-causes`). Subjects starting with `squash:` never ship; fold working squashes before shipping (`rule:nrz:fold-working-squashes`). The body must remain non-empty after ignoring blank lines and `Co-Authored-By:`, `Signed-off-by:`, and `Reviewed-by:` trailers; trailers alone do not explain why the change is needed.

**Rule (Generated artifacts accompany their causes)** · `rule:nrz:generated-artifacts-with-causes`

Renders, derived datasets, and the lock file belong in the commit that causes them to change. During import, a server-generated lock file is amended into the commit that changed dependencies. Keeping causes and generated effects together makes each commit reproducible and prevents a later repair commit from supplying evidence missing from its predecessor (`rule:vision:meaningful-commit-series`), (`rule:vision:fixed-point-gate-and-bootstrap`), (`alg:nrz:import-commit-series`).

**Rule (Fold working squashes before shipping)** · `rule:nrz:fold-working-squashes`

Contributors fold their working squash commits before shipping a named series. They never appear on main, and the gate rejects squash prefixes. This keeps the shipped history composed of meaningful, independently green units rather than intermediate working fragments (`gate:nrz:verification-of-candidate`). The signed import of the named series follows (`alg:nrz:import-commit-series`).

## Documentation · `sec:nrz:contributing-documentation`

**Rule (Edit documentation sources)** · `rule:nrz:edit-documentation-sources`

Every repository Markdown file is a committed render of a minijinja template. Change the owning template and TOML inputs, regenerate derived JSON and Markdown, and include the effects in the causing commit. Templates live at `templates/{output}.j2`; root and cross-cutting inputs and datasets use `data/`. Input envelopes identify the document or dataset, labels, ownership reference, license classification, and source relationships; exact fields follow the actual linter schema. Derived data excludes live clocks, network responses, unordered traversal, and live benchmark timings because reproducibility requires committed, identified sources (`prin:vision:documentation-is-reproducible`), (`dec:vision:documentation-layout`), (`rule:vision:documentation-envelope-and-derivation`), (`rule:nrz:generated-artifacts-with-causes`).

**Rule (Verify the documentation fixed point)** · `rule:nrz:document-fixed-point`

Use `nrz-docgen data` to derive datasets, `nrz-docgen render` to render documents, and `nrz-docgen check` to verify the committed result. The check regenerates into an isolated output area, compares bytes and output inventory, and verifies that another generation produces no change. It exits 3 on drift. This catches edited renders, stale or missing outputs, and transformations that feed their own output back into later generations (`rule:vision:fixed-point-gate-and-bootstrap`), (`step:nrz:bite-documentation-generator`).

**Rule (Stable labels and resolving citations)** · `rule:nrz:stable-labels-and-citations`

A label has three colon-separated components: a lowercase alphanumeric kind, a lowercase alphanumeric area, and a lowercase semantic name with hyphens between words. Each name is unique within its area, including across kinds, and remains stable across file moves. A head mints its label in one backtick span after the middle-dot separator: document titles use a level-one heading, sections use a section heading, and environment heads use the bold environment name and title as the first line of a paragraph. These identities make references independent of file layout (`dec:vision:labels-and-ownership`).

Document-title kinds are `spec` for the architecture, `reg` for the register, `plan` for the plan, and `guide` for the readme and contributing guide. Sections use `sec`. Environment name and kind pairs are Definition `def`, Decision `dec`, Rule `rule`, Requirement `req`, Principle `prin`, Convention `conv`, Table `tab`, Step `step`, Caveat `cav`, Open Question `open`, Example `ex`, Problem `prob`, Counterexample `cex`, Criterion `crit`, Procedure `alg`, Gate `gate`, and Register `reg`. The architecture uses area `vision`, the register uses `exemplars`, and these three new documents use `nrz`.

A citation is its label as a backtick span inside parentheses in running text, never in square brackets, and resolves to exactly one mint. These five documents share one owner, so citations across them use the same ordinary form. Never enclose an inline-code span in bare square brackets. Prose paragraphs occupy one line, tables use Markdown, and these five documents use no code fences except the readme's single three-command fence and the contributing guide's verification-command fence. The linter checks the label and citation structure so that a rendered explanation remains traceable.

**Rule (Check ownership and metadata)** · `rule:nrz:linter-and-ownership`

Sources, inputs, templates, datasets, and renders carry the required metadata and pass the host linter's `check` and `fmt --check`. Ownership follows the declared subtree responsibility: NRZ owns the root files and declarations, while DOCS owns the design and engineering documents, README and this contributing guide, together with their templates. Package owners hold their declared partitions. Every owner may cite the root corpus; root prose cannot import a package premise. DOCS cites package statements through its declared imports. Use the actual linter configuration to resolve owner identities and envelope fields. This keeps generated and handwritten material within the same ownership and traceability rules (`dec:vision:labels-and-ownership`), (`rule:vision:metadata-and-licenses`), (`cav:vision:estate-schema-and-reproducibility`).

## Verification · `sec:nrz:contributing-verification`

**Rule (Required verification)** · `rule:nrz:required-verification`

Every shipped unit must meet the gate of record (`gate:nrz:verification-of-candidate`). After fetching the candidate, first check commit policy over every commit past the base: a Conventional Commits subject of at most 72 characters, a non-empty body explaining why, and no `squash:` prefix (`rule:nrz:commit-subject-and-body`), (`rule:nrz:fold-working-squashes`). Then run these commands from the repository root, in order:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
RUSTDOCFLAGS=-Dwarnings cargo doc --locked --workspace --no-deps
cargo test --locked --workspace --no-fail-fast
cargo run -q --locked -p nrz-docgen -- check
```

After these commands, run `superloop-lint check` and `superloop-lint fmt --check .` from this repository's root on the host. The linter's source is published in [superloop tools](https://github.com/coraxloop/superloop-tools) at tag `v0.1.0`. Vendor that source and build `superloop-lint` from the tools repository with `cargo build -p superloop-lint --release` using its pinned toolchain. Keep your own `Cargo.lock` because the tools project ships neither binaries nor a lockfile.

Before 1.0, any later tools release may change the linter's commands or findings without compatibility guarantees. The linter used by the gate of record decides whether the candidate passes (`gate:nrz:verification-of-candidate`).

`rust-toolchain.toml` selects a floating `nightly` channel with the rustfmt and clippy components. Its comment explains that a dated pin is adopted only once the compute toolchain carries that date, because worker accounts cannot install a toolchain there.

Appropriate package and integration tests, acceptance and rejection cases for dimensions, measures, counts, semantic kinds and affine roles, implemented data-time algebraic validation and assembly-time block checks (`def:vision:dimensions`), (`def:vision:measures`), (`def:vision:counts`), (`rule:vision:semantic-kinds-and-roles`), (`req:vision:algebraic-unit-checks`), architecture dependency checks, and relevant supported feature configurations supply the required evidence (`rule:vision:gate-of-record`), (`rule:vision:verification-locations`), (`rule:vision:layer-enforcement`). Unit-facing changes also verify intrinsic measurement and computational companions: each uncertainty has its own derived descriptor, affine offsets do not enter differences or radii, covariance uses product dimensions, and count standard uncertainty need not be integral (`def:vision:intrinsic-uncertainty-companions`), (`rule:vision:uncertainty-unit-algebra`). Data expressions require declared symbols and an explicit migration; table-shape checks alone do not prove algebraic consistency (`req:vision:expression-and-symbol-contract`), (`req:vision:unit-data-migration`). A contributor submits the candidate branch through the approved Git transport; the integrator authorizes each transfer to the verification mirror, where the verifier runs the gate of record.

Workspace rustc and clippy groups are denied except nursery and cargo, which are configured to warn; the gate still uses its stated warnings-denied clippy invocation. Unsafe code is forbidden. Missing documentation, missing Debug implementations, unreachable public items, unwrap, todo, unimplemented, dbg, and print macros are denied. Routine tooling output uses fallible I/O. Numerical verification combines exact or independently bounded cases, adversarial fixtures, structural identities, and whole worked problems because any one oracle has limited coverage (`rule:vision:gate-of-record`), (`rule:vision:independent-numerical-evidence`). The workspace safety rule does not assert that dependency internals contain no unsafe code (`cav:vision:transitive-unsafe-code`).

**Procedure (READY FOR VERIFY handoff)** · `alg:nrz:ready-for-verify-handoff`

The contributor uses READY FOR VERIFY to hand its named commit series to the verifier. Acceptance depends on the gate of record, and import follows the named-commit procedure (`gate:nrz:verification-of-candidate`), (`alg:nrz:import-commit-series`). Readiness and verification results belong in the execution records; LC standing and remaining work are committed in its closing register and backlog, while the plan retains its obligations and completion criteria (`conv:nrz:status-records`), (`rule:vision:completion-boundaries`).

## Licensing · `sec:nrz:contributing-licensing`

**Rule (Source license classification)** · `rule:nrz:source-license-classification`

Code is AGPL-3.0-only under `LICENSE-CODE`; data and documentation are CC0-1.0 under `LICENSE-DOCS`. Code sources, including generator logic, carry `SPDX-License-Identifier: AGPL-3.0-only`. Documentation, data inputs, and narrative documentation templates carry `SPDX-License-Identifier: CC0-1.0`. Fixtures and generated outputs receive their applicable explicit classification, so generation does not transfer a tooling file's license to unrelated content. Third-party material retains its applicable attribution. These distinctions preserve the intended code and content boundaries through the generation pipeline (`rule:vision:metadata-and-licenses`).
