// SPDX-License-Identifier: AGPL-3.0-only
//! Public source vocabulary and checked service contracts.
//!
//! # Construction verification
//!
//! The public tests cover exact literals and independent small-integer/fraction oracles, Kahan capacity and refusals, ledger boundaries, original occurrence sharing, deterministic snapshots, UTF-8 transaction rollback, foreign/forward references, scope and sort refusals, required receipts, generated provenance, simultaneous substitution, lifting under a binder, bound instantiation, permanent storage limits, partial expressions beneath zero, explicit contradictory hypotheses and every operator variant. The proof fixtures additionally cover exact field schemas, conditional and assumed cancellation, retained partial operands, supplied product/quotient/power derivative premises, Jacobian ordering, simultaneous guard substitution, exact expected claims, every wire truncation, changed semantic versions, forward references, strict shortcut improvement and proof/byte/replay/coefficient ceilings. An independently assembled 492-byte v4 cancellation fixture checks the encoder separately from the decoder. New generator fixtures cover derivative laws, strict domains, the pendulum model, deterministic responses, frozen costs, changed policies, shortcut reuse, queue quotas and resource/support dominance. Tiny frozen graphs are compared with complete simple-path enumeration. No existing algebra or bindings implementation or tests supply these fixtures.
//!
//! # Original cases and syntax inspection
//!
//! `source::ordered_cases` and `source::case_literal` build complete original forms and distinct occurrences atomically. `describe_syntax` exposes constructor payloads and typed dependency roles for expressions, conditions, regions, arms, boundaries, spaces and source forms. Original walks include every selector and branch, including inactive partial expressions. `ElaborateCases` requires an original receipt and checks the ordered reached-region recurrence; an undefined reached guard never falls through.
//!
//! # Original parser and occurrence coverage
//!
//! `source::parse_original(&mut cas, source_key, text, &declaration_header, &mut budget)` returns `Result<OriginalForm, ParseError<'_>>`. Obtain the declaration header directly from admission or through `Cas::context_header`. The source factory stages ordinary `Draft::Source`, `Draft::Expression` and `Draft::Occurrence` records and admits them together. `OriginalForm::syntax` is shared mathematical syntax; `OriginalForm::root` is the distinct original occurrence root. Every occurrence has its source key, precise byte span and ordered children. The caller still completes `walk_original` before making a request. The declared text grammar is:
//!
//! ```ebnf
//! expression     = sum ;
//! sum            = product, { ("+" | "-"), product } ;
//! product        = unary, { ("*" | "/"), unary } ;
//! unary          = ("+" | "-"), unary | power ;
//! power          = atom, [ "^", exponent ] ;
//! atom           = number | name, [ "(", [ sum, { ",", sum } ], ")" ]
//!                | "(", sum, ")" ;
//! exponent       = signed_integer | "(", signed_integer, [ "/", digits ], ")" ;
//! number         = digits, [ ".", digits ], [ ("e" | "E"), signed_integer ] ;
//! signed_integer = [ "+" | "-" ], digits ;
//! digits         = digit, { digit } ;
//! name           = (letter | "_"), { letter | digit | "_" } ;
//! digit          = "0" ... "9" ;
//! letter         = "A" ... "Z" | "a" ... "z" ;
//! ```
//!
//! ASCII whitespace may separate tokens; a number and each signed integer are contiguous. The complete input must match. Binary operators associate left; powers bind above unary signs and have exactly one literal exponent. A fraction denominator must be nonzero. Ordinary `1/2` retains `Div` and both original operands; decimals and scientific tokens become exact `Operator::Rational` values. A directly prefixed negative numeric token becomes a signed rational unless a power intervenes: `-2^2` retains `Neg(IntegerPower(2))`. Parentheses and unary plus widen the enclosed occurrence's span without adding an operator. Leading/trailing whitespace is outside the root span.
//!
//! Names resolve by exact declaration key in the supplied admitted header, including inherited declarations; display names do not create aliases. Numeric literals use that header's scalar field, with no implicit sort coercion. Calls retain ordered arguments and their declared signature. Reduced exponents fitting `i64` use `IntegerPower`; all other exact rational exponents use `GeneralPower` with the header's explicit `Branch` and an original rational exponent operand. An integer exponent is operator payload, so it has no separate operand occurrence; a general exponent's occurrence span includes its optional parentheses.
//!
//! `ParseError` borrows the caller's source key and any offending name, carries a half-open UTF-8 byte `span`, and reports the first unconsumed byte as `parsed_to`. `ParseFailure::Syntax` has stable `UnexpectedToken`, `UnclosedParenthesis`, `TrailingOperator`, `InvalidLiteral` and `InvalidExponent` codes. Missing closing delimiters identify the unmatched opening parenthesis; trailing operators identify the operator itself. `UnknownName`, `NotCallable` and `Arity` retain the identifier and its span; arity also reports expected and actual argument counts. `ParseFailure::Fault` preserves arithmetic, admission and exhaustion classifications unchanged. Refusal publishes no graph prefix or source key and does not refund consumed work.
//!
//! Parsing shares the caller's `Budget`, intersected with store limits and restored on return. `Depth` bounds both grammar nesting (root depth one; each sign, group or call argument descends once) and resulting operand depth. `OriginalOccurrences` includes existing store occupancy and every newly staged use. `LiteralBytes` counts a numeric token, a directly folded minus, or the combined signed numerator, slash and denominator of a rational exponent, excluding separating whitespace and exponent parentheses. Source/name bytes, staged vertices/payload, arithmetic and work use the same existing limits. Explicit continuation frames avoid native-stack recursion even when the caller raises depth; temporary storage is conservatively charged cumulatively before allocation. No dependency or separate authoritative expression representation is added.
//!
//! `walk_original` visits every original operand in postorder, including operands beneath zero multiplication and cancellation candidates, with explicit binder-entry events. It returns a sealed `TraversalReceipt` only when the entire walk succeeds. A `WalkFailure` retains bounded partial events and the fault, with no receipt. A receipt proves coverage, not physical validity or mathematical definedness. A client must inspect the events and perform its own checks before submitting a request. `substitute` performs simultaneous capture-safe free substitution through the complete typed dependency graph; `instantiate_bound` replaces one explicit frame. Both return typed syntax, which must be materialized and walked again for a fresh receipt. `rename_axes` requires an `AxisMap` with exact coordinate, header and space correspondences; ordinary substitution cannot change a declared space or silently perform a coordinate pullback. `materialize_derived` produces fresh occurrences carrying provenance and no invented source spans, which `walk_derived` covers separately.
//!
//! Walk work is charged as frames and events are produced, with each reached immutable interpretation record charged once per walk. Its record charge is at least twice that record's logical payload, so a walk reaching the entire store retains the former conservative full-store floor. Unrelated records contribute no work. Staged payload checks cover the walk's own live frames, retained events, dependency frontier, visited-record set and encoding scratch.

pub use crate::traversal::{
    CompleteWalk, SourceLocation, TraversalReceipt, Visit, WalkEvent, WalkFailure,
};

pub use crate::axis_map::AxisMap;
pub use crate::descriptor::{SyntaxDependency, SyntaxDescriptor, SyntaxRole};
pub use crate::descriptor_model::{ExpressionSyntax, RegionConstructor, SpaceSyntax, SyntaxNode};
pub use crate::source_factory::{OriginalForm, case_literal, ordered_cases};
pub use crate::source_model::{SourceClauseDraft, SourceFormDraft, SourceFormKind};

pub use crate::parse_error::{ParseError, ParseFailure, SyntaxCode};
pub use crate::parser::parse_original;
