// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`dyadic_power_correspondence`] | bindings | Original syntax, reached inspection groups, carriers, exact represented box and the closed checked-outcome allow-list stay associated. |

use super::*;
use nrz_cas::{
    Exhaustion, FaultKind, Reference,
    domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr},
    syntax::{Declaration, Draft, Environment, ExpressionDraft},
};

/// Reproduction tests use the normal exact ledgers without replacing owner budgets.
fn limits() -> nrz_cas::limits::Limits {
    nrz_cas::limits::Limits {
        work: 2_000_000_000,
        arithmetic_steps: 50_000_000,
        integer_exponent: 4096,
        output_bytes: 4_194_304,
        staged_payload_bytes: 64_000_000,
        witness_bytes: 4_194_304,
        ..nrz_cas::limits::Limits::default()
    }
}

/// One genuine original real power of the free coordinate x.
struct Fixture {
    /// Producing graph, never reconstructed from equal syntax.
    cas: Cas,
    /// Actual original power target.
    target: Handle,
    /// Admitted region shares the target's header but is not an expression.
    region: Handle,
    /// Actual context whose header owns the original.
    context: Handle,
    /// Exact original free-coordinate identity.
    base: Handle,
    /// Ancestor header for a valid operand with different metadata.
    parent: Handle,
    /// Header shared by the admitted original.
    header: Handle,
    /// Exact literal operand of the admitted original.
    exponent: Handle,
}

impl Fixture {
    /// Admit one complete original graph with an exact rational exponent.
    fn new(n: i64, d: u64) -> Result<Self, Box<dyn std::error::Error>> {
        let mut budget = Budget::new(limits())?;
        let mut cas = Cas::new(limits())?;
        let sort = Sort::Value(AtomicSort::Scalar(Field::Real));
        let mut environment = Environment::default();
        environment.declarations.push(Declaration {
            key: "x".into(),
            display_name: "x".into(),
            sort: sort.clone(),
        });
        let drafts = [
            Draft::Header(HeaderDraft::Core(Environment::default())),
            Draft::Header(HeaderDraft::Extend {
                parent: Reference::Local(0),
                declarations: environment.declarations,
                spaces: vec![],
            }),
            Draft::Region(RegionDraft {
                header: Reference::Local(1),
                scope: vec![],
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Ambient,
            }),
            Draft::Context(ContextDraft::Root {
                header: Reference::Local(1),
                admissible: Reference::Local(2),
                assumptions: vec![],
            }),
        ];
        let [parent, header, region, context]: [Handle; 4] = cas
            .admit(&drafts, &mut budget)?
            .try_into()
            .map_err(|_| "header fixture")?;
        let make = |operator: Operator, operands: Vec<Handle>| {
            Draft::Expression(ExpressionDraft {
                space: None,
                operator,
                sort: sort.clone(),
                header: Reference::Existing(header.clone()),
                scope: vec![],
                binding: None,
                operands: operands.into_iter().map(Reference::Existing).collect(),
            })
        };
        let base = cas
            .admit(&[make(Operator::Free("x".into()), vec![])], &mut budget)?
            .remove(0);
        let exponent = cas
            .admit(
                &[make(
                    Operator::Rational(Rational::from_ratio(n, d, &mut budget)?),
                    vec![],
                )],
                &mut budget,
            )?
            .remove(0);
        let target = cas
            .admit(
                &[make(
                    Operator::PositivePower,
                    vec![base.clone(), exponent.clone()],
                )],
                &mut budget,
            )?
            .remove(0);
        Ok(Self {
            cas,
            target,
            region,
            context,
            base,
            parent,
            header,
            exponent,
        })
    }

    /// Admit a second context whose different header fails target metadata checks.
    fn other_context(&mut self) -> Result<Handle, Box<dyn std::error::Error>> {
        let mut budget = Budget::new(limits())?;
        let drafts = [
            Draft::Header(HeaderDraft::Core(Environment::default())),
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
        let [_, _, context]: [Handle; 3] = self
            .cas
            .admit(&drafts, &mut budget)?
            .try_into()
            .map_err(|_| "other context")?;
        Ok(context)
    }

    /// Admit a power whose operand has an embedded but different header.
    fn other_header_operand(&mut self) -> Result<Handle, Box<dyn std::error::Error>> {
        let mut budget = Budget::new(limits())?;
        let sort = Sort::Value(AtomicSort::Scalar(Field::Real));
        let base = self
            .cas
            .admit(
                &[Draft::Expression(ExpressionDraft {
                    space: None,
                    operator: Operator::Rational(Rational::from_ratio(1, 1, &mut budget)?),
                    sort: sort.clone(),
                    header: Reference::Existing(self.parent.clone()),
                    scope: vec![],
                    binding: None,
                    operands: vec![].into_iter().collect(),
                })],
                &mut budget,
            )?
            .remove(0);
        let target = self
            .cas
            .admit(
                &[Draft::Expression(ExpressionDraft {
                    space: None,
                    operator: Operator::PositivePower,
                    sort,
                    header: Reference::Existing(self.header.clone()),
                    scope: vec![],
                    binding: None,
                    operands: vec![
                        Reference::Existing(base),
                        Reference::Existing(self.exponent.clone()),
                    ]
                    .into_iter()
                    .collect(),
                })],
                &mut budget,
            )?
            .remove(0);
        Ok(target)
    }

    /// Admit a real power whose base is a rational expression rather than the free coordinate.
    fn other_base_operator(&mut self) -> Result<Handle, Box<dyn std::error::Error>> {
        let mut budget = Budget::new(limits())?;
        let sort = Sort::Value(AtomicSort::Scalar(Field::Real));
        let base = self
            .cas
            .admit(
                &[Draft::Expression(ExpressionDraft {
                    space: None,
                    operator: Operator::Rational(Rational::from_ratio(1, 1, &mut budget)?),
                    sort: sort.clone(),
                    header: Reference::Existing(self.header.clone()),
                    scope: vec![],
                    binding: None,
                    operands: vec![].into_iter().collect(),
                })],
                &mut budget,
            )?
            .remove(0);
        Ok(self
            .cas
            .admit(
                &[Draft::Expression(ExpressionDraft {
                    space: None,
                    operator: Operator::PositivePower,
                    sort,
                    header: Reference::Existing(self.header.clone()),
                    scope: vec![],
                    binding: None,
                    operands: vec![
                        Reference::Existing(base),
                        Reference::Existing(self.exponent.clone()),
                    ]
                    .into_iter()
                    .collect(),
                })],
                &mut budget,
            )?
            .remove(0))
    }
}

/// Explicit one-round policy tests inclusion even when the requested width is unavailable.
fn policy() -> RefinementPolicy {
    RefinementPolicy {
        version: 1,
        precision: 16,
        grid_ceiling: 256,
        rounds: 1,
        reduction_ceiling: 8,
        term_ceiling: 64,
        checking_work: 0,
    }
}

/// Count only public CAS API work on a separate matching original fixture.
fn api_inspection_work(
    fixture: &Fixture,
    target: &Handle,
    with_operands: bool,
) -> Result<u64, Box<dyn std::error::Error>> {
    let mut budget = Budget::new(limits())?;
    fixture.cas.describe_syntax(target, &mut budget)?;
    if with_operands {
        let operands = fixture.cas.operands(target, &mut budget)?;
        for operand in &operands {
            fixture.cas.describe_syntax(operand, &mut budget)?;
        }
    }
    Ok(budget.usage().work)
}

/// Compare one inspected prefix with fresh API calls and the reached wrapper groups.
/// # Panics
/// Panics only if the wrapper's prepaid group total differs from its declared tariff.
#[expect(
    clippy::too_many_lines,
    reason = "the one correspondence control compares all reached inspection groups"
)]
fn check_inspection_receipts() -> Result<(), Box<dyn std::error::Error>> {
    let valid = Fixture::new(1, 2)?;
    let valid_api = Fixture::new(1, 2)?;
    let mut budget = Budget::new(limits())?;
    assert!(inspect(&valid.cas, &valid.target, &valid.context, &mut budget).is_ok());
    let valid_api_work = api_inspection_work(&valid_api, &valid_api.target, true)?;
    assert_eq!(budget.usage().work, valid_api_work + 21);

    // Admitted PositivePower syntax requires expression operands, so the node-kind
    // group is reached on the valid graph and bounded before the following group.
    let mut budget = Budget::new(nrz_cas::limits::Limits {
        work: valid_api_work + 17,
        ..limits()
    })?;
    let Err(DyadicPowerPreparationError::Exact(fault)) =
        inspect(&valid.cas, &valid.target, &valid.context, &mut budget)
    else {
        return Err("bounded operand node-kind group".into());
    };
    assert_eq!(budget.usage().work, valid_api_work + 17);
    assert_eq!(
        fault.kind,
        FaultKind::Exhausted {
            reason: Exhaustion::Limit,
            budget: BudgetKind::Work,
            limit: valid_api_work + 17,
            needed: valid_api_work + 21,
        }
    );

    let nonexpression = Fixture::new(1, 2)?;
    let mut budget = Budget::new(limits())?;
    assert!(matches!(
        inspect(
            &nonexpression.cas,
            &nonexpression.region,
            &nonexpression.context,
            &mut budget,
        ),
        Err(DyadicPowerPreparationError::UnsupportedOriginal)
    ));
    let api = Fixture::new(1, 2)?;
    assert_eq!(
        budget.usage().work,
        api_inspection_work(&api, &api.region, false)? + 5
    );

    let wrong_operator = Fixture::new(1, 2)?;
    let mut budget = Budget::new(limits())?;
    assert!(matches!(
        inspect(
            &wrong_operator.cas,
            &wrong_operator.exponent,
            &wrong_operator.context,
            &mut budget,
        ),
        Err(DyadicPowerPreparationError::UnsupportedOriginal)
    ));
    let api = Fixture::new(1, 2)?;
    assert_eq!(
        budget.usage().work,
        api_inspection_work(&api, &api.exponent, false)? + 7
    );

    let mut wrong_base = Fixture::new(1, 2)?;
    let target = wrong_base.other_base_operator()?;
    let mut api = Fixture::new(1, 2)?;
    let api_target = api.other_base_operator()?;
    let mut budget = Budget::new(limits())?;
    assert!(matches!(
        inspect(&wrong_base.cas, &target, &wrong_base.context, &mut budget),
        Err(DyadicPowerPreparationError::UnsupportedOriginal)
    ));
    assert_eq!(
        budget.usage().work,
        api_inspection_work(&api, &api_target, true)? + 21
    );

    let mut bad_target = Fixture::new(1, 2)?;
    let other_context = bad_target.other_context()?;
    let mut bad_target_api = Fixture::new(1, 2)?;
    bad_target_api.other_context()?;
    let mut budget = Budget::new(limits())?;
    assert!(matches!(
        inspect(
            &bad_target.cas,
            &bad_target.target,
            &other_context,
            &mut budget
        ),
        Err(DyadicPowerPreparationError::UnsupportedOriginal)
    ));
    assert_eq!(
        budget.usage().work,
        api_inspection_work(&bad_target_api, &bad_target_api.target, false)? + 4
    );

    let mut bad_operand = Fixture::new(1, 2)?;
    let target = bad_operand.other_header_operand()?;
    let mut bad_operand_api = Fixture::new(1, 2)?;
    let api_target = bad_operand_api.other_header_operand()?;
    let mut budget = Budget::new(limits())?;
    assert!(matches!(
        inspect(&bad_operand.cas, &target, &bad_operand.context, &mut budget),
        Err(DyadicPowerPreparationError::UnsupportedOriginal)
    ));
    assert_eq!(
        budget.usage().work,
        api_inspection_work(&bad_operand_api, &api_target, true)? + 15
    );
    Ok(())
}

/// Exercise every accepted and stopped enclosure outcome with the original request.
/// # Panics
/// Panics only if a checked outcome changes its admission class.
fn check_outcomes(
    fixture: &mut Fixture,
    request: &nrz_cas::enclosure::EnclosureRequest,
    budget: &mut Budget,
) -> Result<(), Box<dyn std::error::Error>> {
    let original = fixture
        .cas
        .enclose(request, policy(), budget, &mut |_| false)?;
    assert!(matches!(
        original.outcome,
        EnclosureOutcome::Bound | EnclosureOutcome::Undecided(UndecidedReason::WidthLimit)
    ));
    assert!(completed(original).is_ok());
    for outcome in [
        EnclosureOutcome::Exhausted,
        EnclosureOutcome::Undecided(UndecidedReason::WorkExhausted),
        EnclosureOutcome::Interrupted,
        EnclosureOutcome::Unsupported,
        EnclosureOutcome::Undecided(UndecidedReason::DomainBoundary),
        EnclosureOutcome::Proved,
        EnclosureOutcome::Refuted,
        EnclosureOutcome::Undecided(UndecidedReason::Straddle),
    ] {
        let mut report = fixture
            .cas
            .enclose(request, policy(), budget, &mut |_| false)?;
        report.outcome = outcome;
        assert!(matches!(
            completed(report),
            Err(DyadicPowerPreparationError::Stopped(_))
        ));
    }
    for accepted in [
        EnclosureOutcome::Bound,
        EnclosureOutcome::Undecided(UndecidedReason::WidthLimit),
    ] {
        let mut report = fixture
            .cas
            .enclose(request, policy(), budget, &mut |_| false)?;
        report.outcome = accepted;
        assert!(completed(report).is_ok());
        let mut missing = fixture
            .cas
            .enclose(request, policy(), budget, &mut |_| false)?;
        missing.outcome = accepted;
        missing.checked = None;
        assert!(matches!(
            completed(missing),
            Err(DyadicPowerPreparationError::MissingChecked(_))
        ));
    }
    Ok(())
}

/// Original correspondence, width-only request and all closed outcome classes stay distinct.
/// # Panics
/// Panics only if one of the original identities, exact endpoints or closed outcomes changes.
/// ´claim:bindings:dyadic-power-correspondence´
/// ´test:unit:dyadic-power-correspondence´
#[test]
fn dyadic_power_correspondence() -> Result<(), Box<dyn std::error::Error>> {
    check_inspection_receipts()?;
    let mut fixture = Fixture::new(1, 2)?;
    let mut budget = Budget::new(limits())?;
    assert_eq!(
        DyadicPowerPreparation::original_exponent(
            &fixture.cas,
            &fixture.target,
            &fixture.context,
            &mut budget
        )?,
        (1, 2)
    );
    let mut receipt = u64::MAX;
    let prepared = DyadicPowerPreparation::prepare(
        &mut fixture.cas,
        &fixture.target,
        &fixture.context,
        [1.0, 2.0],
        policy(),
        &mut budget,
        &mut |_| false,
        &mut receipt,
    )?;
    assert!(receipt > 0 && receipt < budget.usage().work);
    let request = prepared.judgment().request();
    assert_eq!(request.target, fixture.target);
    assert_eq!(request.context, fixture.context);
    assert_eq!(request.goal, EnclosureGoal::Width);
    assert_eq!(request.coordinates.len(), 1);
    assert_eq!(request.coordinates[0].coordinate, fixture.base);
    assert_eq!(request.coordinates[0].interval.lower(), &Rational::one());
    assert_eq!(
        request.coordinates[0].interval.upper(),
        &Rational::from_ratio(2, 1, &mut budget)?
    );
    assert_ne!(prepared.proof_bytes(), []);
    assert!(prepared.retained_bytes() >= 512);
    assert_eq!(prepared.policy(), policy());

    check_outcomes(&mut fixture, request, &mut budget)?;

    assert!(matches!(
        DyadicPowerPreparation::original_exponent(
            &fixture.cas,
            &fixture.base,
            &fixture.context,
            &mut budget
        ),
        Err(DyadicPowerPreparationError::UnsupportedOriginal)
    ));

    let outside = Fixture::new(4, 3)?;
    assert!(matches!(
        DyadicPowerPreparation::original_exponent(
            &outside.cas,
            &outside.target,
            &outside.context,
            &mut budget
        ),
        Err(DyadicPowerPreparationError::ExponentOutsideDomain)
    ));
    let invalid = Fixture::new(-1, 3)?;
    assert!(matches!(
        DyadicPowerPreparation::original_exponent(
            &invalid.cas,
            &invalid.target,
            &invalid.context,
            &mut budget
        ),
        Err(DyadicPowerPreparationError::ExponentOutsideDomain)
    ));
    Ok(())
}
