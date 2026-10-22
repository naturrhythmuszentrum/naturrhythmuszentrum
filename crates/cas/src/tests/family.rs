// SPDX-License-Identifier: AGPL-3.0-only
//! Local family operations retain exact declarations, regions and edge associations.

use crate::budget::{Budget, Limits};
use crate::domain_model::ContextDraft;
use crate::graph::{Cas, Draft, Handle};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::solving::{FamilyArgument, FamilyArgumentRole, FamilyExpressionData};
use crate::testing::common::TestResult;
use crate::testing::sorts::{Family, Fixture, admit, expression, reference};

/// One request uses one producer and one checker ledger; replay has its own unchanged profile.
fn prove(fixture: &mut Fixture, context: &Handle, left: &Handle, right: &Handle) -> TestResult {
    let mut generation = Budget::new(Limits::default())?;
    let mut checker = Budget::new(Limits::default())?;
    let witness =
        fixture
            .cas
            .family_identity(context, left, right, &mut generation, &mut checker)?;
    assert!(witness.is_discharged());
    let bytes = fixture
        .cas
        .export_witness(&witness, &mut Budget::new(Limits::default())?)?;
    let mut fresh = Cas::new(Limits::default())?;
    let replayed = fresh.inspect(&bytes, &mut Budget::new(Limits::default())?)?;
    assert!(replayed.is_discharged());
    Ok(())
}

/// An explicit product-domain context supplies no global assumptions about the Complex operand.
fn context(fixture: &mut Fixture, family: &Family) -> TestResult<Handle> {
    admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: reference(&fixture.header),
            admissible: reference(&family.region),
            assumptions: Vec::new(),
        }),
    )
}

/// Project the local body while preserving its original nested argument association.
fn component(
    fixture: &mut Fixture,
    family: &Family,
    operation: &Operator,
    part: &Operator,
) -> TestResult<Handle> {
    let mut value = fixture.body_data(family);
    value.operator = operation.clone();
    let local = admit(
        &mut fixture.cas,
        Draft::Solve(crate::solving::SolveDraft::FamilyExpression(value)),
    )?;
    let projected = FamilyExpressionData {
        family: reference(&family.handle),
        ambient: reference(&fixture.real_space),
        region: reference(&family.region),
        scope: Vec::new(),
        operator: part.clone(),
        result: Sort::Value(AtomicSort::Scalar(Field::Real)),
        arguments: vec![FamilyArgument {
            role: FamilyArgumentRole::Family,
            value: reference(&local),
        }],
    };
    Ok(fixture
        .cas
        .family_expression(&projected, &mut Budget::new(Limits::default())?)?)
}

/// The independent right side uses ordinary Real operations on r and the original Complex components.
fn expected(fixture: &mut Fixture, operation: &Operator, part: &Operator) -> TestResult<Handle> {
    let component = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        part.clone(),
        Vec::new(),
        vec![reference(&fixture.complex)],
    )?;
    if *operation == Operator::Add && *part == Operator::ImagPart {
        return Ok(component);
    }
    expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        operation.clone(),
        Vec::new(),
        vec![reference(&fixture.real), reference(&component)],
    )
}

/// Addition retains r in Real and both independent components of the Complex parameter.
/// ´test:crate:family-addition-component-identities´
#[test]
fn family_addition_component_identities() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let context = context(&mut fixture, &family)?;
    for part in [Operator::RealPart, Operator::ImagPart] {
        let left = component(&mut fixture, &family, &Operator::Add, &part)?;
        let right = expected(&mut fixture, &Operator::Add, &part)?;
        prove(&mut fixture, &context, &left, &right)?;
    }
    assert_eq!(
        fixture.cas.expression(&fixture.real)?.1,
        &Sort::Value(AtomicSort::Scalar(Field::Real))
    );
    assert_eq!(
        fixture.cas.expression(&fixture.complex)?.1,
        &Sort::Value(AtomicSort::Scalar(Field::Complex))
    );
    Ok(())
}

/// Multiplication is checked independently in each component, with no real-axis condition on c.
/// ´test:crate:family-multiplication-component-identities´
#[test]
fn family_multiplication_component_identities() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let context = context(&mut fixture, &family)?;
    for part in [Operator::RealPart, Operator::ImagPart] {
        let left = component(&mut fixture, &family, &Operator::Mul, &part)?;
        let right = expected(&mut fixture, &Operator::Mul, &part)?;
        prove(&mut fixture, &context, &left, &right)?;
    }
    Ok(())
}

/// An independent nonzero imaginary component cannot be erased by the placement.
/// ´test:crate:family-does-not-constrain-complex-operand´
#[test]
fn family_does_not_constrain_complex_operand() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let context = context(&mut fixture, &family)?;
    let left = component(&mut fixture, &family, &Operator::Add, &Operator::ImagPart)?;
    let zero = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        Operator::Rational(crate::number::Rational::zero()),
        Vec::new(),
        Vec::new(),
    )?;
    assert!(
        fixture
            .cas
            .family_identity(
                &context,
                &left,
                &zero,
                &mut Budget::new(Limits::default())?,
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    Ok(())
}

/// A Complex literal is a query value, with no copied or retyped coefficient declaration.
fn query(fixture: &mut Fixture, family: &Family, imaginary: bool) -> TestResult<Handle> {
    use crate::solving::{
        CoefficientDomainData, CoefficientMapData, CoefficientValueData, CoefficientValueObject,
        MapMode, MapTarget, SolveDraft,
    };
    let domain = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Gaussian,
        )),
    )?;
    let value = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: reference(&domain),
            value: CoefficientValueData::Gaussian {
                real: crate::number::Rational::one(),
                imaginary: if imaginary {
                    crate::number::Rational::one()
                } else {
                    crate::number::Rational::zero()
                },
            },
        })),
    )?;
    let map = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
            source: reference(&domain),
            target: MapTarget::ScalarAmbient {
                header: reference(&fixture.header),
                field: Field::Complex,
            },
            mode: MapMode::Specialization,
            region: reference(&family.region),
            images: Vec::new(),
            denominators: Vec::new(),
        })),
    )?;
    admit(
        &mut fixture.cas,
        Draft::ExactCoefficient {
            header: reference(&fixture.header),
            value: reference(&value),
            map: reference(&map),
        },
    )
}

/// Both incidence and exclusion export the original placement and all component premises.
fn replay(fixture: &Fixture, witness: &crate::proof_model::VerifiedWitness) -> TestResult {
    let bytes = fixture
        .cas
        .export_witness(witness, &mut Budget::new(Limits::default())?)?;
    let mut fresh = Cas::new(Limits::default())?;
    let checked = fresh.inspect(&bytes, &mut Budget::new(Limits::default())?)?;
    assert!(checked.is_discharged());
    assert!(matches!(
        checked.judgment().statement,
        crate::proof_model::Statement::Equality { .. }
    ));
    Ok(())
}

/// The registered incidence supplies a local Real argument while retaining the original Complex query.
/// ´test:crate:family-placement-inside-replays´
#[test]
fn family_placement_inside_replays() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let context = context(&mut fixture, &family)?;
    let piece = fixture.piece(&family.region)?;
    let location = query(&mut fixture, &family, false)?;
    let outcome = fixture.cas.placement_membership(
        &context,
        &piece,
        &location,
        &mut Budget::new(Limits::default())?,
        &mut Budget::new(Limits::default())?,
    )?;
    let crate::solving::PlacementOutcome::Inside {
        argument,
        incidence,
        ..
    } = outcome
    else {
        return Err("inside query unresolved".into());
    };
    assert_eq!(
        fixture.cas.expression(&argument)?.1,
        &Sort::Value(AtomicSort::Scalar(Field::Real))
    );
    assert_eq!(
        fixture.cas.expression(&location)?.1,
        &Sort::Value(AtomicSort::Scalar(Field::Complex))
    );
    for proof in &*incidence {
        replay(&fixture, proof)?;
    }
    Ok(())
}

/// Nonzero imaginary component evidence refuses the point; it never creates an empty root set.
/// ´test:crate:family-placement-outside-replays´
#[test]
fn family_placement_outside_replays() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let context = context(&mut fixture, &family)?;
    let piece = fixture.piece(&family.region)?;
    let location = query(&mut fixture, &family, true)?;
    let outcome = fixture.cas.placement_membership(
        &context,
        &piece,
        &location,
        &mut Budget::new(Limits::default())?,
        &mut Budget::new(Limits::default())?,
    )?;
    let crate::solving::PlacementOutcome::Refused {
        reason, exclusion, ..
    } = outcome
    else {
        return Err("outside query unresolved".into());
    };
    assert_eq!(
        reason,
        crate::request_model::RefusalReason::NoApplicablePiece
    );
    replay(&fixture, &exclusion)
}

/// An arbitrary Complex query has no guessed real-locus membership.
/// ´test:crate:family-placement-membership-pending´
#[test]
fn family_placement_membership_pending() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let context = context(&mut fixture, &family)?;
    let piece = fixture.piece(&family.region)?;
    let location = fixture.complex.clone();
    let outcome = fixture.cas.placement_membership(
        &context,
        &piece,
        &location,
        &mut Budget::new(Limits::default())?,
        &mut Budget::new(Limits::default())?,
    )?;
    assert!(matches!(
        outcome,
        crate::solving::PlacementOutcome::Unresolved {
            reason: crate::solving::SolveReason::PieceMembershipPending
        }
    ));
    Ok(())
}

/// A base guard is copied verbatim into its total preimage; metadata supplies no truth.
fn stratum(fixture: &mut Fixture, family: &Family, predicate: &Handle) -> TestResult<Family> {
    use crate::domain_model::{AmbientDomain, RegionDraft, RegionExpr};
    let base = admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: reference(&fixture.header),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: family.parameters.clone(),
            },
            expression: RegionExpr::GuardTrue {
                base: reference(&family.base_region),
                predicate: reference(predicate),
            },
        }),
    )?;
    let region = admit(
        &mut fixture.cas,
        Draft::FamilyRegion {
            region: RegionDraft {
                header: reference(&fixture.header),
                scope: Vec::new(),
                ambient: AmbientDomain::Product {
                    coordinates: family.total.clone(),
                },
                expression: RegionExpr::GuardTrue {
                    base: reference(&family.region),
                    predicate: reference(predicate),
                },
            },
            lies_over: crate::solving::LiesOver {
                family: reference(&family.handle),
                stratum: reference(&base),
            },
        },
    )?;
    Ok(Family {
        parameters: family.parameters.clone(),
        base_region: base,
        handle: family.handle.clone(),
        region,
        total: family.total.clone(),
    })
}

/// Positivity is an ordinary Real predicate, independent of family evaluation.
fn positive(fixture: &mut Fixture, value: &Handle) -> TestResult<Handle> {
    admit(
        &mut fixture.cas,
        Draft::Condition {
            header: reference(&fixture.header),
            scope: Vec::new(),
            kind: crate::model::PredicateKind::Positive,
            arguments: vec![reference(value)],
        },
    )
}

/// Conjugation fixes a placed Real operand locally, with no theorem outside the current stratum.
/// ´test:crate:family-conjugation-retains-stratum´
#[test]
fn family_conjugation_retains_stratum() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let real = fixture.real.clone();
    let predicate = positive(&mut fixture, &real)?;
    let piece = stratum(&mut fixture, &family, &predicate)?;
    let context = context(&mut fixture, &piece)?;
    let mut value = fixture.body_data(&piece);
    value.operator = Operator::Neg;
    value.arguments.truncate(1);
    let base = admit(
        &mut fixture.cas,
        Draft::Solve(crate::solving::SolveDraft::FamilyExpression(value.clone())),
    )?;
    let right = fixture
        .cas
        .family_expression(&value, &mut Budget::new(Limits::default())?)?;
    value.operator = Operator::Conjugate;
    value.arguments = vec![FamilyArgument {
        role: FamilyArgumentRole::Family,
        value: reference(&base),
    }];
    let left = fixture
        .cas
        .family_expression(&value, &mut Budget::new(Limits::default())?)?;
    let proof = fixture.cas.family_identity(
        &context,
        &left,
        &right,
        &mut Budget::new(Limits::default())?,
        &mut Budget::new(Limits::default())?,
    )?;
    assert!(
        matches!(&proof.judgment().statement, crate::proof_model::Statement::Equality { domain, .. } if *domain == piece.region)
    );
    replay(&fixture, &proof)
}

/// Restriction exports ordinary subset and weakening, and cannot be reversed to widen the theorem.
/// ´test:crate:family-restriction-rejects-widening´
#[test]
fn family_restriction_rejects_widening() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let parent = context(&mut fixture, &family)?;
    let left = component(&mut fixture, &family, &Operator::Add, &Operator::RealPart)?;
    let right = expected(&mut fixture, &Operator::Add, &Operator::RealPart)?;
    let proof = fixture.cas.family_identity(
        &parent,
        &left,
        &right,
        &mut Budget::new(Limits::default())?,
        &mut Budget::new(Limits::default())?,
    )?;
    let real = fixture.real.clone();
    let predicate = positive(&mut fixture, &real)?;
    let piece = stratum(&mut fixture, &family, &predicate)?;
    let child = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Local {
            parent: reference(&parent),
            region: reference(&piece.region),
            hypotheses: Vec::new(),
        }),
    )?;
    let restricted = fixture.cas.family_restrict(
        &child,
        &proof,
        &mut Budget::new(Limits::default())?,
        &mut Budget::new(Limits::default())?,
    )?;
    replay(&fixture, &restricted)?;
    assert!(
        fixture
            .cas
            .family_restrict(
                &parent,
                &restricted,
                &mut Budget::new(Limits::default())?,
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    Ok(())
}

/// Overlap compatibility retains both original associations, including their placement operands.
/// ´test:crate:family-overlap-compatibility-replays´
#[test]
fn family_overlap_compatibility_replays() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let context = context(&mut fixture, &family)?;
    let mut value = fixture.body_data(&family);
    let first = fixture
        .cas
        .family_expression(&value, &mut Budget::new(Limits::default())?)?;
    value.arguments.reverse();
    let second = fixture
        .cas
        .family_expression(&value, &mut Budget::new(Limits::default())?)?;
    let left = fixture.cas.handle(
        *fixture
            .cas
            .state
            .expression(first.logical_id())?
            .arms
            .first()
            .ok_or("missing arm")?,
    );
    let right = fixture.cas.handle(
        *fixture
            .cas
            .state
            .expression(second.logical_id())?
            .arms
            .first()
            .ok_or("missing arm")?,
    );
    let proof = fixture.cas.family_agreement(
        &context,
        &left,
        &right,
        &mut Budget::new(Limits::default())?,
        &mut Budget::new(Limits::default())?,
    )?;
    assert!(matches!(
        proof.judgment().statement,
        crate::proof_model::Statement::Compatibility { .. }
    ));
    let bytes = fixture
        .cas
        .export_witness(&proof, &mut Budget::new(Limits::default())?)?;
    assert!(
        Cas::new(Limits::default())?
            .inspect(&bytes, &mut Budget::new(Limits::default())?)?
            .is_discharged()
    );
    Ok(())
}

/// A projection link cannot invent membership in its stated positive base stratum.
/// ´test:crate:family-projection-metadata-is-not-evidence´
#[test]
fn family_projection_metadata_is_not_evidence() -> TestResult {
    use crate::domain_model::{AmbientDomain, RegionDraft, RegionExpr};
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let real = fixture.real.clone();
    let predicate = positive(&mut fixture, &real)?;
    let positive = stratum(&mut fixture, &family, &predicate)?;
    let mut over = crate::solving::LiesOver {
        family: reference(&family.handle),
        stratum: reference(&family.base_region),
    };
    assert_eq!(over.stratum, reference(&family.base_region));
    let replacement = reference(&positive.base_region);
    assert_ne!(over.stratum, replacement);
    over.stratum = replacement;
    let region = admit(
        &mut fixture.cas,
        Draft::FamilyRegion {
            region: RegionDraft {
                header: reference(&fixture.header),
                scope: Vec::new(),
                ambient: AmbientDomain::Product {
                    coordinates: family.total.clone(),
                },
                expression: RegionExpr::Ambient,
            },
            lies_over: over,
        },
    )?;
    let forged = Family {
        parameters: family.parameters.clone(),
        base_region: positive.base_region,
        handle: family.handle,
        region,
        total: family.total,
    };
    let context = context(&mut fixture, &forged)?;
    let left = component(&mut fixture, &forged, &Operator::Add, &Operator::RealPart)?;
    let right = expected(&mut fixture, &Operator::Add, &Operator::RealPart)?;
    assert!(
        fixture
            .cas
            .family_identity(
                &context,
                &left,
                &right,
                &mut Budget::new(Limits::default())?,
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    Ok(())
}

/// A family base stratum cannot depend on a fibre coefficient, even through a Real component.
/// ´test:crate:family-base-guard-rejects-fibre-dependency´
#[test]
fn family_base_guard_rejects_fibre_dependency() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let projected = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        Operator::RealPart,
        Vec::new(),
        vec![reference(&fixture.complex)],
    )?;
    let predicate = positive(&mut fixture, &projected)?;
    assert!(stratum(&mut fixture, &family, &predicate).is_err());
    Ok(())
}

/// Moving the original Real operand into a Complex fibre role is a pinned single-field mutation.
/// ´test:crate:family-real-operand-role-tamper´
#[test]
fn family_real_operand_role_tamper() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let mut value = fixture.body_data(&family);
    let argument = value.arguments.first_mut().ok_or("missing base operand")?;
    assert_eq!(argument.role, FamilyArgumentRole::Base);
    let replacement = FamilyArgumentRole::Fibre;
    assert_ne!(argument.role, replacement);
    argument.role = replacement;
    assert!(
        fixture
            .cas
            .family_expression(&value, &mut Budget::new(Limits::default())?)
            .is_err()
    );
    Ok(())
}

/// Positive base evidence is checked by the existing Real sign schema on exactly its base stratum.
fn base_positive(
    fixture: &mut Fixture,
    family: &Family,
    predicate: &Handle,
) -> TestResult<crate::proof_model::VerifiedWitness> {
    use crate::proof_model::{ApplicationDraft, Certificate, Statement};
    use crate::solving::{
        CertificateRealSign, RealSignConclusion, RealSignRule, RealSignStep, SolveCertificate,
        SolveSchema,
    };
    let context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: reference(&fixture.header),
            admissible: reference(&family.base_region),
            assumptions: Vec::new(),
        }),
    )?;
    let step = ApplicationDraft {
        context: context.clone(),
        rule: crate::rule::Primitive::Solve(SolveSchema::RealSign),
        conclusion: Statement::PredicateFact {
            region: family.base_region.clone(),
            predicate: predicate.clone(),
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Solve(Box::new(SolveCertificate::RealSign(
            CertificateRealSign {
                subject: fixture.real.clone(),
                region: family.base_region.clone(),
                root: 0,
                steps: vec![RealSignStep {
                    rule: RealSignRule::Hypothesis,
                    subject: fixture.real.clone(),
                    conclusion: RealSignConclusion::Positive,
                    arguments: Vec::new(),
                    premises: Vec::new(),
                    exponent: 0,
                }],
            },
        ))),
    };
    Ok(fixture
        .cas
        .check_proof(&context, &[step], &mut Budget::new(Limits::default())?)?)
}

/// Base facts travel only through a proved preimage and never onto a sibling or the larger base.
/// ´test:crate:family-restriction-transports-base-sign´
#[test]
fn family_restriction_transports_base_sign() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let real = fixture.real.clone();
    let predicate = positive(&mut fixture, &real)?;
    let piece = stratum(&mut fixture, &family, &predicate)?;
    let proof = base_positive(&mut fixture, &piece, &predicate)?;
    let context = context(&mut fixture, &piece)?;
    let transported = fixture.cas.family_restrict(
        &context,
        &proof,
        &mut Budget::new(Limits::default())?,
        &mut Budget::new(Limits::default())?,
    )?;
    assert!(
        matches!(&transported.judgment().statement, crate::proof_model::Statement::PredicateFact { region, predicate: actual }
        if *region == piece.region && *actual == predicate)
    );
    let bytes = fixture
        .cas
        .export_witness(&transported, &mut Budget::new(Limits::default())?)?;
    assert!(
        Cas::new(Limits::default())?
            .inspect(&bytes, &mut Budget::new(Limits::default())?)?
            .is_discharged()
    );
    let whole = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: reference(&fixture.header),
            admissible: reference(&family.region),
            assumptions: Vec::new(),
        }),
    )?;
    assert!(
        fixture
            .cas
            .family_restrict(
                &whole,
                &proof,
                &mut Budget::new(Limits::default())?,
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    Ok(())
}
