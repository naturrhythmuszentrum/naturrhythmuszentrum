<!-- SPDX-License-Identifier: CC0-1.0 -->

# nrz-bindings · `guide:bindings:overview`

## Purpose · `sec:bindings:purpose`

`nrz-bindings` joins the engine's exact descriptor graph to its floating computational graph. It checks what symbols and operations mean, and supplies evidence for their numerical representations without owning numerical storage. CAS supplies exact structure and proof rules; bindings supplies physical vocabulary such as kinds, units, affine roles and companion relationships. A float region has no inherent voltage, current, Real or Complex meaning: a binding's reading gives it that meaning.

## Place in the engine · `sec:bindings:engine`

The dependency direction is matrix → bindings → CAS. Bindings depends on CAS and no numerical engine crate; matrix and docgen consume its checks. Scalar and accuracy sit beneath matrix, independently of bindings. The target descriptor graph retains sorts, units, axes, representations and checked relations as exact CAS structure; the computational graph retains region versions and kernel applications with numerical error. `docs/design/descriptor-binding.md` specifies that separation and the remaining migration from Rust descriptor records to graph-backed meanings.

Bindings owns original physical checks, exact measure and coordinate maps, immutable declaration evidence, companion pairings and closed symbolic lowering recipes. Matrix resolves live numerical tickets under an execution lease and validates extents, layouts, precision, versions and numerical evidence. Neither a descriptor nor a bindings witness certifies a float value by itself. Kernels are intended to be meaning-free float procedures with an exact law and enclosure theorem; their meaning checks belong on this side before lowering.

Several symbols may eventually read the same floats through checked related bindings. A Complex symbol could read a 2×2 block while a Real symbol reads its `(0,0)` corner; an LC state `(v,i)` could read a two-float buffer while its components read entries in volts and amperes. Every overlap requires a checked reading relation. The implemented stage 3a subset still requires one symbol owner per migrated region, and aliases retain that owner. It does not implement the general shared-reading design.

## In the LC target · `sec:bindings:lc-target`

For `L=25 mH`, `C=10 µF`, the state operator `[[0,1/C],[-1/L,0]]` acts on `(voltage,current)` and produces `(voltage/time,current/time)`. Bindings must check every entry, including typed diagonal zeros, before normalization or elimination. The reference spectrum gives `ω₀=2000 rad/s` and `f₀=1000/π Hz`; equal inverse-time dimensions do not identify angular frequency, cyclic frequency and derivative rate.

The angle/frequency registry already has exact degree and turn scales `π/180` and `2π`, and `FrequencyRelation` retains the full-cycle factor between the two frequency kinds. These are exact descriptor checks. Non-rational coefficient export still returns `UnsupportedExactConstantExport`; a successful π-scale relation does not imply a numerical π kernel. The crossing happens once: bindings exports a rational spelling, scalar supplies a proven enclosure and import error, and matrix uses that enclosure. No heap coordinate is converted into CAS arithmetic.

Stage 3a provides homogeneous import/copy symbol admission. In `docs/design/lc-resonance.md`, stage 3b adds the shaped graph and representation definitions, 3c the general versioned bindings, 3d mixed maps and restoration, and 3e cumulative symbolic policy and exact LC theorem transport. Stages 3f and 3g add checked phasor-block and restricted spectral lowerings, with integrated acceptance in 3h. Stage 5 supplies certified constant/phase evaluation and the quantity surface; stage 6 assembles reusable LC plans. Existing scalar checks do not yet execute the mixed LC operator.

## Main concepts and types · `sec:bindings:concepts`

`Descriptor` combines a `Dimension`, `Kind`, `Measure` and `Role`. `Registry` contains explicit ordered semantic rules; dimension equality never creates a kind conversion. `Scale::Expression` carries an `ExactConstant`, and measure admission needs checked positivity. Reporting names or unchecked symbolic text supply no evidence. The registered frequency relation is explicit:

```rust
use nrz_bindings::{ExponentPolicy, FrequencyDirection, Registry, UnitError};

let registry = Registry::with_angle_frequency(ExponentPolicy::Rational)?;
let cyclic = registry.measure("Hz").ok_or(UnitError::UnsupportedKindRule)?;
let angular = registry.measure("rad/s").ok_or(UnitError::UnsupportedKindRule)?;
assert_eq!(cyclic.dimension(), angular.dimension());
assert!(cyclic.conversion_to(angular).is_err());
let relation = registry.frequency_relation(cyclic, angular)?;
assert_eq!(relation.direction(), FrequencyDirection::ToAngular);
# Ok::<(), UnitError>(())
```

`Context` declares meanings, parses through CAS, walks every original occurrence and derives `CheckedExpression` evidence. `freeze` validates even unused declarations and consumes the builder into `CheckedDeclarations`. A `SnapshotId` distinguishes both foreign contexts and stale semantic revisions. Checked originals retain their operands, descriptors, conversions and diagnostics rather than replacing the source with normalized coordinates.

`PairingWitness` derives uncertainty and output-over-input sensitivity schemas. `EvaluationDescriptor` describes error in a parameter such as covariance, separately from uncertainty in either physical target. `CheckedMap`, `MapIdentity` and `DescriptorConstant` retain exact references, origins and bounded exports. `CheckedSensitivity` keeps original checks, replayed CAS witnesses, ordered coefficient recipes and numerical obligations that matrix must discharge.

For the implemented binding path, `HomogeneousRecipe` checks original entries, `BindingSymbols` declares value and companion symbols, and `copy_result` derives outputs before execution. `CopyWitness` checks their exact source and target relations. `SymbolBinding<T>` joins that inventory to opaque region tickets; only the numerical client can establish that those tickets are live. Detailed contracts are in `src/admission/`, `src/evidence/`, `src/constants/`, `src/pairing/` and `src/sensitivity/`.

## Guarantees and refusals · `sec:bindings:guarantees`

Original checks precede simplification, including operands beneath zero, cancellation or inactive declarations. An untyped zero cannot supply missing physical meaning. Point minus point produces a difference; point plus point and multiplicative point operations refuse. Counts retain their declared integer domains and are never silently rounded into continuous values. Equal dimensions, equal numbers and equal reporting names establish neither source identity nor independence.

Exact scales use bounded CAS constant algebra and conservative definedness/positivity checks. A mathematically positive expression can refuse when the available rules cannot prove it. Non-rational definitions remain exact; rational-only exports refuse instead of approximating them. A frozen snapshot proves descriptor validity, not numerical domain safety, uncertainty propagation, physical model adequacy or an assurance claim.

Companions retain their distinct kinds and ordered targets. Covariance uses both difference measures; its evaluation error targets the covariance parameter itself. A symbolic nonzero hypothesis cannot establish that a runtime divisor box excludes zero. Closed sensitivity recipes require independently replayed CAS results and keep unresolved numerical premises explicit. Exhaustion and unsupported fragments remain typed failures, with finite work, depth, occurrence and output-byte limits.

## Current scope and limits · `sec:bindings:scope`

The implemented surface includes scalar descriptors and original-expression checks, exact measures with π/e constants, frequency relations, frozen evidence, formal pairings, bounded coordinate exports, native square/reciprocal/positive-root/solve sensitivities, and stage 3a homogeneous symbol admission. That recipe accepts coherent concrete differences and refuses points, heterogeneous entries and formal quotients even if their dimensions agree. Its restriction is a recipe predicate, not a law of float storage.

General shaped descriptor graphs, zero-region exact bindings, Complex pair/block readings, shared overlapping bindings and complete covariance/source-symbol inventories remain later stages. Shape and source semantics still partly live in matrix. Bindings executes no numerical expression, allocates no heap region and supplies no general implicit-function or numerical enclosure theorem. Exact judgments are machine-independent; numerical runs agree within tracked error under their admitted models rather than by bit identity.

## Testing · `sec:bindings:testing`

Public conformance tests exercise dimensional/kind/role refusals, exact scale identities, original invalid operands hidden by zero, stale snapshots, companion identities and missing premises. Integration tests cover exact frequency round trips, constant export, schema maps and symbolic sensitivity lifecycles. Admission tests pin exact entry limits and cumulative CAS `N`/`N−1` boundaries; sensitivity tests measure each fixture's required work afresh and check exact-limit success and one-below refusal. Compile twins protect checked pairing state. Run `cargo test -p nrz-bindings --test public` for the public contract; rustdoc checks the front-page example and included compile twins. Test evidence establishes the declared fragment, not the later LC numerical route.
