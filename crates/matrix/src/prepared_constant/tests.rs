// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`constant_export_goldens`] | matrix | Both retained golden rows and the stored reading guarantee hold. |
//! | [`constant_export_identity`] | matrix | Original operators, operands, context and live graph identities cannot be substituted. |
//! | [`constant_export_draft_arity`] | matrix | Malformed product and logarithm drafts refuse before becoming original handles. |
//! | [`constant_export_resources`] | matrix | Failed preparation and import retain charged prefixes and distinct outcomes. |
//! | [`constant_export_conversion`] | matrix | Each endpoint and stored error is checked independently. |
//! | [`constant_export_storage`] | matrix | Complete owner storage includes its graph and caller reservation at exact boundaries. |
//! | [`constant_export_import_bounds`] | matrix | Numerical publication checks its complete output before decoding. |

use super::*;
use nrz_bindings::readings::algebra::{
    Handle, Reference,
    domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr},
    enclosure::EnclosureGoal,
    limits::Limits,
    syntax::{AtomicSort, Draft, Environment, ExpressionDraft, Field, Operator, Sort},
};

/// Fallible test construction does not turn errors into mutant kills.
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Preserve the returned graph for refusal fixtures without giving production a mutable accessor.
fn attempt(
    cas: &mut Cas,
    request: &EnclosureRequest,
    policy: RefinementPolicy,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
    used: &mut u64,
) -> std::result::Result<PreparedConstant, PreparedConstantError> {
    let empty = Cas::new(budget.limits().clone())?;
    let graph = std::mem::replace(cas, empty);
    match PreparedConstant::new(graph, request, policy, budget, interrupt, used) {
        Ok(owner) => Ok(owner),
        Err((graph, error)) => {
            *cas = graph;
            Err(error)
        }
    }
}

/// Explicit policy for the merged corpus's width goal, with no numerical export helper.
fn policy() -> RefinementPolicy {
    RefinementPolicy {
        version: 1,
        precision: 160,
        grid_ceiling: 256,
        rounds: 1,
        reduction_ceiling: 4,
        term_ceiling: 128,
        checking_work: 0,
    }
}

/// Finite test allowances, not a default production profile.
fn ledger() -> Result<Budget> {
    Ok(Budget::new(Limits {
        work: 2_000_000_000,
        literal_bytes: 4096,
        integer_exponent: 4096,
        arithmetic_steps: 50_000_000,
        ..Limits::default()
    })?)
}

/// Build originals without parsing names, importing golden endpoints or supplied judgments.
fn original(
    factor: i64,
    argument: i64,
    operator: Operator,
) -> Result<(Cas, EnclosureRequest, Vec<Handle>)> {
    original_operator(factor, argument, operator, Operator::NaturalLog)
}

/// Keep the inner operator independently variable for the original-law control.
fn original_operator(
    factor: i64,
    argument: i64,
    operator: Operator,
    inner: Operator,
) -> Result<(Cas, EnclosureRequest, Vec<Handle>)> {
    let mut b = ledger()?;
    let mut cas = Cas::new(b.limits().clone())?;
    let mut environment = Environment::default();
    if let Operator::Call(key) = &operator {
        environment
            .declarations
            .push(nrz_bindings::readings::algebra::syntax::Declaration {
                key: key.clone(),
                display_name: key.clone(),
                sort: Sort::Function {
                    inputs: vec![AtomicSort::Scalar(Field::Real); 2],
                    output: AtomicSort::Scalar(Field::Real),
                },
            });
    }
    let mut drafts = vec![
        Draft::Header(HeaderDraft::Core(environment)),
        Draft::Region(RegionDraft {
            header: Reference::Local(0),
            scope: vec![],
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
        Draft::Context(ContextDraft::Root {
            header: Reference::Local(0),
            admissible: Reference::Local(1),
            assumptions: vec![],
        }),
    ];
    for (operator, operands) in [
        (
            Operator::Rational(Rational::from_ratio(factor, 1, &mut b)?),
            vec![],
        ),
        (
            Operator::Rational(Rational::from_ratio(argument, 1, &mut b)?),
            vec![],
        ),
        (inner, vec![Reference::Local(4)]),
        (operator, vec![Reference::Local(3), Reference::Local(5)]),
    ] {
        drafts.push(Draft::Expression(ExpressionDraft {
            space: None,
            operator,
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            header: Reference::Local(0),
            scope: vec![],
            binding: None,
            operands,
        }));
    }
    let handles = cas.admit(&drafts, &mut b)?;
    let request = EnclosureRequest {
        target: handles[6].clone(),
        context: handles[2].clone(),
        coordinates: vec![],
        goal: EnclosureGoal::Width,
    };
    Ok((cas, request, handles))
}

/// Both retained golden rows and the stored reading guarantee hold.
/// # Panics
/// Panics only if the export loses containment, target association or warm authority.
/// ´claim:matrix:constant-export-goldens´
/// ´test:unit:constant-export-goldens´
#[test]
fn constant_export_goldens() -> Result {
    let (mut cas, request, _) = original(2, 2, Operator::Mul)?;
    let mut b = ledger()?;
    b.work(17, AT)?;
    let mut used = 0;
    let result = attempt(
        &mut cas,
        &request,
        policy(),
        &mut b,
        &mut |_| false,
        &mut used,
    );
    assert!(
        result.is_ok(),
        "original preparation must succeed: {result:?}"
    );
    let prepared = result?;
    let graph_counts = prepared.graph.counts();
    assert!(
        prepared
            .graph
            .expression(&request.target, &mut Budget::new(Limits::default())?)
            .is_ok(),
        "producing graph must retain the original target"
    );
    assert_eq!(b.usage().work, 17 + used);
    assert_eq!(prepared.authority().judgment().request(), &request);
    assert_eq!(prepared.authority().policy(), policy());
    let context = ArithmeticContext::new()?;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/golden/exact-constant.json");
    let corpus = nrz_docgen::golden::read(
        std::fs::File::open(path)?,
        nrz_docgen::golden::Limits::default(),
        &mut b,
    )?;
    let rows = corpus.value()["entries"].as_array().ok_or("rows")?;
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(
            row["width_goal"]["bits"].as_u64(),
            Some(u64::from(prepared.authority().policy().precision))
        );
        let judgment = prepared.authority().judgment();
        assert_eq!(
            row["judgment"]["checker_version"].as_u64(),
            Some(u64::from(judgment.certificate().version))
        );
        assert_eq!(
            row["judgment"]["schema"].as_str(),
            Some(judgment.formal_schema().name)
        );
        let mut scope = String::new();
        for byte in &judgment.certificate().scope {
            use std::fmt::Write;
            write!(scope, "{byte:02x}")?;
        }
        assert_eq!(row["judgment"]["scope_hex"].as_str(), Some(scope.as_str()));
        for (index, spelling) in prepared.authority().endpoints().iter().enumerate() {
            assert_eq!(row["judgment"]["interval"][index].as_str(), Some(*spelling));
        }
        let bits = u32::try_from(row["target"]["bits"].as_u64().ok_or("bits")?)?;
        let before = b.usage().work;
        let literals = LiteralLimits::default();
        let result = prepared.import_with_evidence(bits, literals, &context, &mut b, &mut used);
        assert_eq!(b.usage().work, before + used);
        if row["id"] == "twice-log-two-spacing" {
            assert_eq!(
                row["expected"]["refusal"].as_str(),
                Some("target-not-certifiable")
            );
            assert!(matches!(
                result,
                Err(PreparedConstantError::TargetNotCertifiable)
            ));
            continue;
        }
        assert!(
            result.is_ok(),
            "admitted golden row must export: {result:?}"
        );
        let (value, endpoints) = result?;
        assert_eq!(row["expected"].as_str(), Some("enclosure"));
        imported(
            &prepared,
            [
                row["golden"]["point"][0].as_str().ok_or("lo")?,
                row["golden"]["point"][1].as_str().ok_or("hi")?,
            ],
            (value, endpoints),
            bits,
            &context,
            &mut b,
            used,
        )?;
        assert_eq!(prepared.graph.counts(), graph_counts);
    }
    targets(&prepared, &context, &mut b);
    Ok(())
}

/// Golden readings, public import and warm conversion share the charged evidence.
/// # Panics
/// Panics only if golden containment, public import or warm reuse disagrees with
/// the checked readings, receipt or endpoints.
fn imported(
    prepared: &PreparedConstant,
    golden: [&str; 2],
    (value, endpoints): (ConstantImport, [f64; 2]),
    bits: u32,
    context: &ArithmeticContext,
    b: &mut Budget,
    import_work: u64,
) -> Result {
    assert_eq!(value.bits(), bits);
    assert_eq!(value.arithmetic_revision(), context.revision());
    readings(value, endpoints, golden, bits, b)?;
    let mut used = 0;
    let public = prepared.import(bits, LiteralLimits::default(), context, b, &mut used)?;
    assert_eq!(public.readings(), value.readings());
    assert_eq!(public.bits(), value.bits());
    assert_eq!(public.arithmetic_revision(), value.arithmetic_revision());
    assert_eq!(
        used, import_work,
        "public import shares the conversion receipt"
    );
    let again =
        prepared.import_with_evidence(bits, LiteralLimits::default(), context, b, &mut used);
    assert!(again.is_ok(), "warm import must succeed: {again:?}");
    let (again, again_endpoints) = again?;
    assert_eq!(again.readings(), value.readings());
    assert_eq!(again_endpoints, endpoints);
    Ok(())
}

/// Original operators, operands, context and live graph identities cannot be substituted.
/// # Panics
/// Panics only if an unsupported or foreign original gains export authority.
/// ´claim:matrix:constant-export-identity´
/// ´test:unit:constant-export-identity´
#[test]
fn constant_export_identity() -> Result {
    for (factor, argument, operator) in [
        (1, 2, Operator::Mul),
        (2, 3, Operator::Mul),
        (2, 2, Operator::Add),
        (2, 2, Operator::Call("Mul(2, Log(2))".into())),
    ] {
        let sum = matches!(operator, Operator::Add);
        let (mut cas, request, _) = original(factor, argument, operator)?;
        let mut used = 0;
        assert!(matches!(
            attempt(
                &mut cas,
                &request,
                policy(),
                &mut ledger()?,
                &mut |_| false,
                &mut used
            ),
            Err(PreparedConstantError::Preparation(
                ExpressionEnclosureError::Unsupported(
                    nrz_bindings::UnitError::UnsupportedExactConstantExport
                )
            ))
        ));
        if sum {
            assert_eq!(used, 1, "original admission charges before the sum refusal");
        }
    }
    operand_refusals()?;
    let (mut cas, request, handles) = original(2, 2, Operator::Mul)?;
    let interval = nrz_bindings::readings::algebra::enclosure::Interval::new(
        Rational::one(),
        Rational::one(),
        &mut ledger()?,
    )?;
    let mut coordinates = request.clone();
    coordinates.coordinates.push(
        nrz_bindings::readings::algebra::enclosure::CoordinateBound {
            coordinate: handles[3].clone(),
            interval,
        },
    );
    let goal = EnclosureRequest {
        goal: EnclosureGoal::AtMost(Rational::one()),
        ..request.clone()
    };
    for changed in [coordinates, goal] {
        assert!(matches!(
            attempt(
                &mut cas,
                &changed,
                policy(),
                &mut ledger()?,
                &mut |_| false,
                &mut 0
            ),
            Err(PreparedConstantError::Preparation(
                ExpressionEnclosureError::Unsupported(_)
            ))
        ));
    }
    foreign_originals(&mut cas, &request)?;
    other_header(&mut cas, &request)?;
    let changed = EnclosureRequest {
        target: handles[5].clone(),
        ..request
    };
    assert!(matches!(
        attempt(
            &mut cas,
            &changed,
            policy(),
            &mut ledger()?,
            &mut |_| false,
            &mut 0
        ),
        Err(PreparedConstantError::Preparation(
            ExpressionEnclosureError::Unsupported(_)
        ))
    ));
    let (mut cas, request, _) = original_operator(2, 2, Operator::Mul, Operator::Exponential)?;
    assert!(matches!(
        attempt(
            &mut cas,
            &request,
            policy(),
            &mut ledger()?,
            &mut |_| false,
            &mut 0
        ),
        Err(PreparedConstantError::Preparation(
            ExpressionEnclosureError::Unsupported(_)
        ))
    ));
    Ok(())
}

/// Ordered original operands and their exact arity are admission facts.
/// # Panics
/// Panics only if a swapped product becomes an admitted export.
fn operand_refusals() -> Result {
    let indices = [5, 3];
    let (mut cas, request, handles) = original(2, 2, Operator::Mul)?;
    let target = cas
        .admit(
            &[Draft::Expression(ExpressionDraft {
                space: None,
                operator: Operator::Mul,
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                header: Reference::Existing(handles[0].clone()),
                scope: vec![],
                binding: None,
                operands: indices
                    .iter()
                    .map(|&i| Reference::Existing(handles[i].clone()))
                    .collect(),
            })],
            &mut ledger()?,
        )
        .map_err(|error| format!("operand fixture {indices:?} refused before export: {error:?}"))?;
    let changed = EnclosureRequest {
        target: target[0].clone(),
        ..request
    };
    let result = attempt(
        &mut cas,
        &changed,
        policy(),
        &mut ledger()?,
        &mut |_| false,
        &mut 0,
    );
    assert!(
        matches!(
            result,
            Err(PreparedConstantError::Preparation(
                ExpressionEnclosureError::Unsupported(
                    nrz_bindings::UnitError::UnsupportedExactConstantExport
                )
            ))
        ),
        "ordered two-operand original required: {indices:?}: {result:?}"
    );
    Ok(())
}

/// Malformed product and logarithm drafts refuse before becoming original handles.
/// # Panics
/// Panics only if invalid operand arity passes draft validation or changes its cause.
/// ´claim:matrix:constant-export-draft-arity´
/// ´test:unit:constant-export-draft-arity´
#[test]
fn constant_export_draft_arity() -> Result {
    let (mut cas, _, handles) = original(2, 2, Operator::Mul)?;
    for (operator, indices) in [
        (Operator::Mul, vec![3, 5, 3]),
        (Operator::NaturalLog, vec![4, 4]),
    ] {
        let result = cas.admit(
            &[Draft::Expression(ExpressionDraft {
                space: None,
                operator,
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                header: Reference::Existing(handles[0].clone()),
                scope: vec![],
                binding: None,
                operands: indices
                    .iter()
                    .map(|&i| Reference::Existing(handles[i].clone()))
                    .collect(),
            })],
            &mut ledger()?,
        );
        // Neither malformed original becomes a handle, so its constant key cannot be queried.
        assert!(
            matches!(
                result,
                Err(Fault {
                    kind: nrz_bindings::readings::algebra::FaultKind::Invalid(
                        nrz_bindings::readings::algebra::InvalidCode::Sort
                    ),
                    location: Location {
                        operation: "draft validation",
                        ..
                    },
                    ..
                })
            ),
            "malformed operand shape refuses at draft validation: {indices:?}: {result:?}"
        );
    }
    Ok(())
}

/// The successful import's whole Work receipt fixes its final charged step.
/// # Panics
/// Panics only if an exact allowance fails or one less publishes a reading.
fn import_work(prepared: &PreparedConstant, context: &ArithmeticContext) -> Result {
    let mut used = 0;
    let result = prepared.import(
        40,
        LiteralLimits::default(),
        context,
        &mut ledger()?,
        &mut used,
    );
    assert!(result.is_ok(), "baseline charged import: {result:?}");
    let required = 52_446;
    assert_eq!(
        used, required,
        "successful import includes its displacement charge"
    );
    for work in [required, required - 1] {
        let mut b = Budget::new(Limits {
            work,
            ..ledger()?.limits().clone()
        })?;
        let result = prepared.import(40, LiteralLimits::default(), context, &mut b, &mut used);
        if work == required {
            assert!(result.is_ok(), "exact import Work boundary: {result:?}");
            assert_eq!(used, required);
        } else {
            assert!(
                matches!(
                    result,
                    Err(PreparedConstantError::Exact(Fault {
                        kind: nrz_bindings::readings::algebra::FaultKind::Exhausted {
                            budget: BudgetKind::Work,
                            ..
                        },
                        ..
                    }))
                ),
                "one-below import Work boundary: {result:?}"
            );
        }
    }
    Ok(())
}

/// Independent integer decoding includes small positive radii outside the value binade.
/// # Panics
/// Panics only if an exported finite reading becomes nonfinite.
fn dyadic(value: f64, b: &mut Budget) -> Result<Rational> {
    assert!(value.is_finite());
    let bits = value.to_bits();
    let field = (bits >> 52) & 2047;
    let significand = (bits & ((1_u64 << 52) - 1)) + if field == 0 { 0 } else { 1_u64 << 52 };
    let exponent = if field == 0 {
        -1074
    } else {
        i64::try_from(field)? - 1075
    };
    let sign = if bits >> 63 == 0 { 1 } else { -1 };
    Ok(
        Rational::from_ratio(sign * i64::try_from(significand)?, 1, b)?
            .mul(&Rational::from_ratio(2, 1, b)?.pow(exponent, b)?, b)?,
    )
}

/// Failed preparation and import retain charged prefixes and distinct outcomes.
/// # Panics
/// Panics only if a failed attempt loses its Work prefix or changes its typed cause.
/// ´claim:matrix:constant-export-resources´
/// ´test:unit:constant-export-resources´
#[test]
fn constant_export_resources() -> Result {
    let (mut cas, request, _) = original(2, 2, Operator::Mul)?;
    let mut used = 999;
    let mut short = Budget::new(Limits {
        work: 0,
        ..ledger()?.limits().clone()
    })?;
    assert!(matches!(
        attempt(
            &mut cas,
            &request,
            policy(),
            &mut short,
            &mut |_| false,
            &mut used
        ),
        Err(PreparedConstantError::Preparation(
            ExpressionEnclosureError::Exact(_)
        ))
    ));
    assert_eq!(used, 0);
    schedule_refusals(&mut cas, &request)?;
    let mut b = ledger()?;
    b.work(7, AT)?;
    assert!(matches!(
        attempt(
            &mut cas,
            &request,
            policy(),
            &mut b,
            &mut |_| true,
            &mut used
        ),
        Err(PreparedConstantError::Preparation(
            ExpressionEnclosureError::Interrupted
        ))
    ));
    assert!(used > 0);
    assert_eq!(b.usage().work, 7 + used);
    let prepared = attempt(
        &mut cas,
        &request,
        policy(),
        &mut b,
        &mut |_| false,
        &mut used,
    )?;
    let context = ArithmeticContext::new()?;
    let mut zero = Budget::new(Limits {
        work: 0,
        ..ledger()?.limits().clone()
    })?;
    assert!(matches!(
        prepared.import(40, LiteralLimits::default(), &context, &mut zero, &mut used),
        Err(PreparedConstantError::Exact(_))
    ));
    assert_eq!(used, 0);
    let before = b.usage().work;
    assert!(matches!(
        prepared.import(
            40,
            LiteralLimits::new(0, 4096, 100_000),
            &context,
            &mut b,
            &mut used
        ),
        Err(PreparedConstantError::Arithmetic(
            ArithmeticFailure::ResourceLimit
        ))
    ));
    assert!(used > 1);
    assert_eq!(b.usage().work, before + used);
    Ok(())
}

/// Each endpoint and stored error is checked independently.
/// # Panics
/// Panics only if an inward endpoint or unattained target is admitted.
/// ´claim:matrix:constant-export-conversion´
/// ´test:unit:constant-export-conversion´
#[test]
fn constant_export_conversion() -> Result {
    let (mut cas, request, _) = original(2, 2, Operator::Mul)?;
    let mut b = ledger()?;
    let prepared = attempt(&mut cas, &request, policy(), &mut b, &mut |_| false, &mut 0)?;
    let context = ArithmeticContext::new()?;
    let lo = f64::from_bits(0x3ff6_2e42_fefa_39ef);
    let hi = f64::from_bits(0x3ff6_2e42_fefa_39f0);
    let slack = 1.5 * f64::from_bits(983_u64 << 52);
    assert!(finish(prepared.authority(), [lo, hi], 40, &context, &mut b).is_ok());
    for endpoints in [[hi, hi], [lo, lo], [lo - slack, hi], [lo, hi + slack]] {
        assert!(matches!(
            finish(prepared.authority(), endpoints, 40, &context, &mut b),
            Err(PreparedConstantError::TargetNotCertifiable)
        ));
    }
    let ceiling = f64::from_bits(984_u64 << 52);
    assert!(error_target(ceiling, 40).is_ok());
    assert!(matches!(
        error_target(f64::from_bits(ceiling.to_bits() + 1), 40),
        Err(PreparedConstantError::TargetNotCertifiable)
    ));
    for value in [
        0.0,
        -0.0,
        f64::from_bits(1),
        -f64::from_bits(1),
        f64::MIN_POSITIVE,
        -f64::MIN_POSITIVE,
        0.5,
        -0.5,
        1.0,
        2.0,
        f64::from_bits(1.0_f64.to_bits() - 1),
        f64::MAX,
        -f64::MAX,
    ] {
        let exact = represented(nrz_scalar::Finite::new(value)?, &mut b);
        assert!(
            exact.is_ok(),
            "every finite value decodes: {value:?}: {exact:?}"
        );
        assert_eq!(
            exact?.compare(&dyadic(value, &mut b)?, &mut b)?,
            Ordering::Equal,
            "finite decoding preserves the represented dyadic: {value:?}"
        );
    }
    for invalid in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert!(matches!(
            finish(prepared.authority(), [invalid, hi], 40, &context, &mut b),
            Err(PreparedConstantError::Arithmetic(
                ArithmeticFailure::NonFinite
            ))
        ));
    }
    Ok(())
}

/// Complete owner storage includes its graph and caller reservation at exact boundaries.
/// # Panics
/// Panics only if either complete storage ceiling or its one-below control changes.
/// ´claim:matrix:constant-export-storage´
/// ´test:unit:constant-export-storage´
#[test]
fn constant_export_storage() -> Result {
    let (mut cas, request, _) = original(2, 2, Operator::Mul)?;
    let prepared = attempt(
        &mut cas,
        &request,
        policy(),
        &mut ledger()?,
        &mut |_| false,
        &mut 0,
    )?;
    let bytes = prepared.authority.owned_bytes() + size_of::<PreparedConstant>() as u64;
    let caller = 37;
    let graph = prepared.graph.counts().payload_bytes;
    for (output, staged, expected) in [
        (bytes, graph + bytes + caller, true),
        (bytes - 1, graph + bytes + caller, false),
        (bytes, graph + bytes + caller - 1, false),
    ] {
        let mut b = Budget::new(Limits {
            output_bytes: output,
            staged_payload_bytes: staged,
            ..ledger()?.limits().clone()
        })?;
        let result = b.with_retained_payload(caller, AT, |b| {
            finalize(&prepared.graph, &prepared.authority, b)
        });
        assert_eq!(
            result.is_ok(),
            expected,
            "complete owner boundary: {result:?}"
        );
        if !expected {
            assert!(matches!(
                result,
                Err(Fault {
                    kind: nrz_bindings::readings::algebra::FaultKind::Exhausted { .. },
                    ..
                })
            ));
        }
    }
    let context = ArithmeticContext::new()?;
    let mut b = Budget::new(Limits {
        work: 2,
        ..ledger()?.limits().clone()
    })?;
    let mut used = 0;
    let result = prepared.import(40, LiteralLimits::default(), &context, &mut b, &mut used);
    assert!(
        matches!(
            result,
            Err(PreparedConstantError::Arithmetic(
                ArithmeticFailure::ResourceLimit
            ))
        ),
        "literal child is capped before work: {result:?}"
    );
    assert_eq!(used, 1);
    let mut b = Budget::new(Limits {
        staged_payload_bytes: graph + bytes - 1,
        ..ledger()?.limits().clone()
    })?;
    let result = prepared.import(40, LiteralLimits::default(), &context, &mut b, &mut used);
    assert!(
        matches!(result, Err(PreparedConstantError::Exact(_))),
        "complete import preflight: {result:?}"
    );
    assert_eq!(
        used, 0,
        "all retained import storage precedes numerical work"
    );
    Ok(())
}

/// Numerical publication checks its complete output before decoding.
/// # Panics
/// Panics only if the publication ceiling or its admission receipt changes.
/// ´claim:matrix:constant-export-import-bounds´
/// ´test:unit:constant-export-import-bounds´
#[test]
fn constant_export_import_bounds() -> Result {
    let (mut cas, request, _) = original(2, 2, Operator::Mul)?;
    let prepared = attempt(
        &mut cas,
        &request,
        policy(),
        &mut ledger()?,
        &mut |_| false,
        &mut 0,
    )?;
    let context = ArithmeticContext::new()?;
    import_work(&prepared, &context)?;
    let proof = prepared.authority.judgment().certificate();
    let structural = size_of_val(proof.records.as_slice())
        + size_of_val(proof.logarithms.as_slice())
        + proof.scope.capacity()
        + size_of::<ExpressionEnclosure>();
    assert!(
        prepared.authority.owned_bytes() >= structural as u64,
        "retained evidence covers at least its structural payload"
    );
    let bytes = size_of::<ConstantImport>() as u64;
    assert_eq!(
        bytes, 32,
        "three binary64 readings and two u32 metadata fields"
    );
    for (output, expected) in [(bytes, true), (bytes - 1, false)] {
        let mut b = Budget::new(Limits {
            output_bytes: output,
            ..ledger()?.limits().clone()
        })?;
        let mut used = 0;
        let result = prepared.import(40, LiteralLimits::default(), &context, &mut b, &mut used);
        assert_eq!(
            result.is_ok(),
            expected,
            "numerical publication boundary: {result:?}"
        );
        if !expected {
            assert!(matches!(
                result,
                Err(PreparedConstantError::Exact(Fault {
                    kind: nrz_bindings::readings::algebra::FaultKind::Exhausted {
                        budget: BudgetKind::OutputBytes,
                        ..
                    },
                    ..
                }))
            ));
            assert_eq!(used, 1, "output admission precedes literal decoding");
        }
    }
    Ok(())
}

/// Decode the exported readings independently and compare the whole golden interval.
/// # Panics
/// Panics only if containment, both endpoint targets or the stored error guarantee fails.
fn readings(
    value: ConstantImport,
    [lo, hi]: [f64; 2],
    golden: [&str; 2],
    bits: u32,
    b: &mut Budget,
) -> Result {
    let [center, total, error] = value.readings();
    assert_eq!(
        center.to_bits(),
        lo.to_bits(),
        "center is the lower outer endpoint"
    );
    assert_eq!(total.to_bits(), error.to_bits());
    assert!(error > 0.0 && lo <= center && center <= hi);
    let radius = dyadic(error, b)?;
    let c = dyadic(center, b)?;
    let lower = Rational::parse(golden[0], b)?;
    let upper = Rational::parse(golden[1], b)?;
    let l = dyadic(lo, b)?;
    let u = dyadic(hi, b)?;
    assert_ne!(l.compare(&lower, b)?, Ordering::Greater);
    assert_ne!(u.compare(&upper, b)?, Ordering::Less);
    assert_ne!(c.sub(&radius, b)?.compare(&lower, b)?, Ordering::Greater);
    assert_ne!(c.add(&radius, b)?.compare(&upper, b)?, Ordering::Less);
    assert_ne!(u.sub(&c, b)?.compare(&radius, b)?, Ordering::Greater);
    assert_ne!(c.sub(&l, b)?.compare(&radius, b)?, Ordering::Greater);
    let epsilon = Rational::from_ratio(2, 1, b)?.pow(-i64::from(bits), b)?;
    assert_ne!(
        radius.compare(&epsilon.add(&epsilon, b)?, b)?,
        Ordering::Greater
    );
    assert_ne!(upper.sub(&l, b)?.compare(&epsilon, b)?, Ordering::Greater);
    assert_ne!(u.sub(&lower, b)?.compare(&epsilon, b)?, Ordering::Greater);
    Ok(())
}

/// An independently valid equal original cannot substitute either live handle.
/// # Panics
/// Panics only if a foreign target or context misses its typed identity refusal.
fn foreign_originals(cas: &mut Cas, request: &EnclosureRequest) -> Result {
    let (_, foreign, _) = original(2, 2, Operator::Mul)?;
    for changed in [
        EnclosureRequest {
            target: foreign.target,
            ..request.clone()
        },
        EnclosureRequest {
            context: foreign.context,
            ..request.clone()
        },
    ] {
        assert!(matches!(
            attempt(
                cas,
                &changed,
                policy(),
                &mut ledger()?,
                &mut |_| false,
                &mut 0
            ),
            Err(PreparedConstantError::Preparation(
                ExpressionEnclosureError::Exact(Fault {
                    kind: nrz_bindings::readings::algebra::FaultKind::Invalid(
                        nrz_bindings::readings::algebra::InvalidCode::ForeignHandle
                    ),
                    ..
                })
            ))
        ));
    }
    Ok(())
}

/// A same-store context with a different header cannot supply authority.
/// # Panics
/// Panics only if engine preflight accepts the mismatched header or changes its cause.
fn other_header(cas: &mut Cas, request: &EnclosureRequest) -> Result {
    let mut environment = Environment::default();
    environment
        .declarations
        .push(nrz_bindings::readings::algebra::syntax::Declaration {
            key: "other".into(),
            display_name: "other".into(),
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
        });
    let handles = cas.admit(
        &[
            Draft::Header(HeaderDraft::Core(environment)),
            Draft::Region(RegionDraft {
                header: Reference::Local(0),
                scope: vec![],
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Ambient,
            }),
            Draft::Context(ContextDraft::Root {
                header: Reference::Local(0),
                admissible: Reference::Local(1),
                assumptions: vec![],
            }),
        ],
        &mut ledger()?,
    )?;
    let changed = EnclosureRequest {
        context: handles[2].clone(),
        ..request.clone()
    };
    let result = attempt(
        cas,
        &changed,
        policy(),
        &mut ledger()?,
        &mut |_| false,
        &mut 0,
    );
    assert!(
        matches!(
            result,
            Err(PreparedConstantError::Preparation(
                ExpressionEnclosureError::Exact(Fault {
                    kind: nrz_bindings::readings::algebra::FaultKind::Invalid(
                        nrz_bindings::readings::algebra::InvalidCode::Scope
                    ),
                    ..
                })
            ))
        ),
        "same-store header mismatch refuses as Scope: {result:?}"
    );
    Ok(())
}

/// Finite refinement and exact resource exhaustion retain separate causes.
/// # Panics
/// Panics only if a finite schedule or exhausted attempt changes its typed outcome.
fn schedule_refusals(cas: &mut Cas, request: &EnclosureRequest) -> Result {
    let mut used = 0;
    let mut limited = Budget::new(Limits {
        work: 20_000,
        ..ledger()?.limits().clone()
    })?;
    let limited_result = attempt(
        cas,
        request,
        policy(),
        &mut limited,
        &mut |_| false,
        &mut used,
    );
    assert!(
        matches!(
            limited_result,
            Err(PreparedConstantError::Preparation(
                ExpressionEnclosureError::Exact(Fault {
                    kind: nrz_bindings::readings::algebra::FaultKind::Exhausted { .. },
                    ..
                })
            ))
        ),
        "finite exact allowance: {limited_result:?}"
    );
    assert_eq!(used, limited.usage().work);
    let narrow_schedule = RefinementPolicy {
        term_ceiling: 1,
        ..policy()
    };
    let result = attempt(
        cas,
        request,
        narrow_schedule,
        &mut ledger()?,
        &mut |_| false,
        &mut used,
    );
    assert!(
        matches!(
            result,
            Err(PreparedConstantError::Preparation(
                ExpressionEnclosureError::TargetNotCertifiable
            ))
        ),
        "finite refinement: {result:?}"
    );
    let result = attempt(
        cas,
        request,
        RefinementPolicy {
            version: 2,
            ..policy()
        },
        &mut ledger()?,
        &mut |_| false,
        &mut used,
    );
    assert!(matches!(
        result,
        Err(PreparedConstantError::Preparation(
            ExpressionEnclosureError::Exact(Fault {
                kind: nrz_bindings::readings::algebra::FaultKind::Invalid(
                    nrz_bindings::readings::algebra::InvalidCode::Policy
                ),
                ..
            })
        ))
    ));
    Ok(())
}

/// The spacing floor is a charged admission refusal, not a failed literal attempt.
/// # Panics
/// Panics only if a target cause or its admission tariff changes.
fn targets(prepared: &PreparedConstant, context: &ArithmeticContext, b: &mut Budget) {
    let mut used = 0;
    let reached = prepared.import(52, LiteralLimits::default(), context, b, &mut used);
    assert!(
        used > 1,
        "52 bits lies above the spacing floor: {reached:?}"
    );
    assert!(
        matches!(
            reached,
            Ok(_) | Err(PreparedConstantError::TargetNotCertifiable)
        ),
        "52-bit decoding returns only a reading or certification refusal: {reached:?}"
    );
    for bits in [53, 54, 60, u32::MAX] {
        assert!(matches!(
            prepared.import(bits, LiteralLimits::default(), context, b, &mut used),
            Err(PreparedConstantError::TargetNotCertifiable)
        ));
        assert_eq!(used, 1, "spacing floor spends its single admission charge");
    }
    assert!(matches!(
        prepared.import(0, LiteralLimits::default(), context, b, &mut used),
        Err(PreparedConstantError::InvalidTarget)
    ));
}
