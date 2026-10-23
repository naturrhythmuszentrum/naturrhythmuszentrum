// SPDX-License-Identifier: AGPL-3.0-only
//! Independent typed region, header, context and replay boundaries.

use nrz_cas::domain::{
    AmbientDomain, CaseArmDraft, CasesDraft, ContextDraft, HeaderDraft, HypothesisRef, Layer,
    RegionDraft, RegionExpr,
};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::{
    ApplicationDraft, Certificate, ConstructorLaw, Primitive, RegionCertificate, Statement,
};
use nrz_cas::syntax::{Draft, Field, Operator, PredicateKind};
use nrz_cas::{Cas, FaultKind, Handle, InvalidCode, Rational, Reference};

use crate::testing::budgets::proof_budget as budget;
use crate::testing::builders::{admit, environment, expression};
use crate::testing::common::{TestResult, real};

/// One exact real axis and an independent scalar used by local hypotheses.
struct Fixture {
    cas: Cas,
    header: Handle,
    context: Handle,
    ambient: Handle,
    ambient_kind: AmbientDomain,
    x: Handle,
    y: Handle,
    zero: Handle,
    one: Handle,
}

impl Fixture {
    /// Declare semantic dependencies before any scoped theorem exists.
    fn new() -> TestResult<Self> {
        Self::axis(false)
    }

    /// Use exact integer or real coordinate sorts in the same independent fixture.
    fn axis(integer: bool) -> TestResult<Self> {
        let sort = if integer {
            nrz_cas::syntax::Sort::Value(nrz_cas::syntax::AtomicSort::Integer)
        } else {
            real()
        };
        let ambient_kind = if integer {
            AmbientDomain::Integer("x".into())
        } else {
            AmbientDomain::Real("x".into())
        };
        let mut declarations = environment(Field::Real, &["x", "y"]);
        for declaration in &mut declarations.declarations {
            declaration.sort = sort.clone();
        }
        let mut cas = Cas::new(budget()?.limits().clone())?;
        let header = admit(
            &mut cas,
            Draft::Header(HeaderDraft::Core(declarations)),
            &mut budget()?,
        )?;
        let ambient = admit(
            &mut cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                ambient: ambient_kind.clone(),
                expression: RegionExpr::Ambient,
            }),
            &mut budget()?,
        )?;
        let context = admit(
            &mut cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header.clone()),
                admissible: Reference::Existing(ambient.clone()),
                assumptions: Vec::new(),
            }),
            &mut budget()?,
        )?;
        let x = expression(
            &mut cas,
            &context,
            Operator::Free("x".into()),
            sort.clone(),
            &[],
            &mut budget()?,
        )?;
        let y = expression(
            &mut cas,
            &context,
            Operator::Free("y".into()),
            sort.clone(),
            &[],
            &mut budget()?,
        )?;
        let zero = expression(
            &mut cas,
            &context,
            Operator::Rational(Rational::zero()),
            sort.clone(),
            &[],
            &mut budget()?,
        )?;
        let one = expression(
            &mut cas,
            &context,
            Operator::Rational(Rational::one()),
            sort,
            &[],
            &mut budget()?,
        )?;
        Ok(Self {
            cas,
            header,
            context,
            ambient,
            ambient_kind,
            x,
            y,
            zero,
            one,
        })
    }

    /// Retain an exact scalar predicate without asserting its truth.
    fn predicate(&mut self, kind: PredicateKind, arguments: &[Handle]) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                kind,
                arguments: arguments.iter().cloned().map(Reference::Existing).collect(),
            },
            &mut budget()?,
        )
    }

    /// Retain exact region syntax over the same declared axis.
    fn region(&mut self, expression: RegionExpr) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                ambient: self.ambient_kind.clone(),
                expression,
            }),
            &mut budget()?,
        )
    }

    /// A guard-true region includes only defined true points.
    fn guarded(&mut self, predicate: &Handle) -> TestResult<Handle> {
        self.region(RegionExpr::GuardTrue {
            base: Reference::Existing(self.ambient.clone()),
            predicate: Reference::Existing(predicate.clone()),
        })
    }
}

/// One explicit typed rule proposal; no builder can mark it accepted.
fn application(
    context: &Handle,
    rule: Primitive,
    conclusion: Statement,
    certificate: Certificate,
    premises: Vec<u32>,
) -> ApplicationDraft {
    ApplicationDraft {
        context: context.clone(),
        rule,
        conclusion,
        certificate,
        premises,
        substitutions: Vec::new(),
    }
}

/// Hand-specified affine cuts retain their singleton and exterior cells.
/// ´test:integration:affine-region-cells´
#[test]
fn affine_region_cells() -> TestResult {
    let mut fixture = Fixture::new()?;
    let two = expression(
        &mut fixture.cas,
        &fixture.context,
        Operator::Rational(Rational::from_ratio(2, 1, &mut budget()?)?),
        real(),
        &[],
        &mut budget()?,
    )?;
    let twice = expression(
        &mut fixture.cas,
        &fixture.context,
        Operator::Mul,
        real(),
        &[two, fixture.x.clone()],
        &mut budget()?,
    )?;
    let affine = expression(
        &mut fixture.cas,
        &fixture.context,
        Operator::Sub,
        real(),
        &[twice, fixture.one.clone()],
        &mut budget()?,
    )?;
    let positive = fixture.predicate(PredicateKind::Positive, std::slice::from_ref(&affine))?;
    let nonnegative =
        fixture.predicate(PredicateKind::NonNegative, std::slice::from_ref(&affine))?;
    let positive = fixture.guarded(&positive)?;
    let nonnegative = fixture.guarded(&nonnegative)?;
    let half = Rational::from_ratio(1, 2, &mut budget()?)?;
    let certificate = RegionCertificate {
        cuts: vec![half],
        membership: vec![vec![false, false, true], vec![false, true, true]],
    };
    let step = application(
        &fixture.context,
        Primitive::RegionDecide,
        Statement::Subset {
            subset: positive,
            superset: nonnegative,
        },
        Certificate::Region(certificate.clone()),
        Vec::new(),
    );
    let witness = fixture.cas.check_proof(
        &fixture.context,
        std::slice::from_ref(&step),
        &mut budget()?,
    )?;
    assert!(witness.is_discharged());
    assert_eq!(witness.judgment().layer, Layer::Logic);
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    let mut changed = step.clone();
    let mut wrong = certificate;
    *wrong
        .membership
        .first_mut()
        .and_then(|row| row.get_mut(1))
        .ok_or("missing cut cell")? = true;
    changed.certificate = Certificate::Region(wrong);
    assert!(
        fixture
            .cas
            .check_proof(&fixture.context, &[changed], &mut budget()?)
            .is_err()
    );
    let mut short = Budget::new(Limits {
        region_cells: 2,
        ..budget()?.limits().clone()
    })?;
    let fault = fixture
        .cas
        .check_proof(&fixture.context, std::slice::from_ref(&step), &mut short)
        .err()
        .ok_or("accepted too few region cells")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::RegionCells,
            ..
        }
    ));
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    let mut exact = Budget::new(Limits {
        region_cells: 3,
        ..budget()?.limits().clone()
    })?;
    fixture
        .cas
        .check_proof(&fixture.context, &[step], &mut exact)?;
    assert_eq!(exact.usage().region_cells, 3);
    Ok(())
}

/// Negative, zero and positive cells cover the line; dropping the singleton is false.
/// ´test:integration:singleton-cover-hole´
#[test]
fn singleton_cover_hole() -> TestResult {
    let mut fixture = Fixture::new()?;
    let negative_value = expression(
        &mut fixture.cas,
        &fixture.context,
        Operator::Neg,
        real(),
        std::slice::from_ref(&fixture.x),
        &mut budget()?,
    )?;
    let negative = fixture.predicate(PredicateKind::Positive, &[negative_value])?;
    let zero = fixture.predicate(
        PredicateKind::Equal,
        &[fixture.x.clone(), fixture.zero.clone()],
    )?;
    let positive = fixture.predicate(PredicateKind::Positive, &[fixture.x.clone()])?;
    let negative = fixture.guarded(&negative)?;
    let zero = fixture.guarded(&zero)?;
    let positive = fixture.guarded(&positive)?;
    let rows = vec![
        vec![true, true, true],
        vec![true, false, false],
        vec![false, true, false],
        vec![false, false, true],
    ];
    let step = application(
        &fixture.context,
        Primitive::RegionDecide,
        Statement::Cover {
            domain: fixture.ambient.clone(),
            members: vec![negative.clone(), zero, positive.clone()],
        },
        Certificate::Region(RegionCertificate {
            cuts: vec![Rational::zero()],
            membership: rows,
        }),
        Vec::new(),
    );
    let witness = fixture.cas.check_proof(
        &fixture.context,
        std::slice::from_ref(&step),
        &mut budget()?,
    )?;
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let mut permuted = step;
    if let Statement::Cover { members, .. } = &mut permuted.conclusion {
        members.reverse();
    }
    if let Certificate::Region(value) = &mut permuted.certificate {
        value
            .membership
            .get_mut(1..)
            .ok_or("missing member rows")?
            .reverse();
    }
    let alternate = fixture
        .cas
        .check_proof(&fixture.context, &[permuted], &mut budget()?)?;
    assert_eq!(
        fixture.cas.export_witness(&alternate, &mut budget()?)?,
        bytes
    );
    let mut receiver = Cas::new(budget()?.limits().clone())?;
    let replayed = receiver.inspect(&bytes, &mut budget()?)?;
    assert_eq!(receiver.export_witness(&replayed, &mut budget()?)?, bytes);
    let missing = application(
        &fixture.context,
        Primitive::RegionDecide,
        Statement::Cover {
            domain: fixture.ambient.clone(),
            members: vec![negative, positive],
        },
        Certificate::Region(RegionCertificate {
            cuts: vec![Rational::zero()],
            membership: vec![
                vec![true, true, true],
                vec![true, false, false],
                vec![false, false, true],
            ],
        }),
        Vec::new(),
    );
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    assert!(
        fixture
            .cas
            .check_proof(&fixture.context, &[missing], &mut budget()?)
            .is_err()
    );
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// A used local predicate becomes a region-guarded residual; an unused slot contributes nothing.
/// ´test:integration:qualified-local-closure´
#[test]
fn qualified_local_closure() -> TestResult {
    let mut fixture = Fixture::new()?;
    let positive = fixture.predicate(PredicateKind::Positive, &[fixture.x.clone()])?;
    let region = fixture.guarded(&positive)?;
    let predicate = fixture.predicate(PredicateKind::NonZero, &[fixture.y.clone()])?;
    let unused = fixture.predicate(PredicateKind::Defined, &[fixture.x.clone()])?;
    let local = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(fixture.context.clone()),
            region: Reference::Existing(region.clone()),
            hypotheses: vec![
                nrz_cas::domain::AssumptionDraft {
                    key: "used".into(),
                    condition: Reference::Existing(predicate.clone()),
                },
                nrz_cas::domain::AssumptionDraft {
                    key: "unused".into(),
                    condition: Reference::Existing(unused),
                },
            ],
        }),
        &mut budget()?,
    )?;
    let domain = fixture.cas.admissible_region(&local)?;
    let fact = Statement::PredicateFact {
        region: domain.clone(),
        predicate: predicate.clone(),
    };
    let steps = vec![
        application(
            &local,
            Primitive::ContextHypothesis,
            fact.clone(),
            Certificate::Hypothesis(HypothesisRef {
                context: local.clone(),
                slot: 1,
            }),
            Vec::new(),
        ),
        application(
            &fixture.context,
            Primitive::RegionConstruct,
            Statement::Constructor {
                region: domain.clone(),
                law: ConstructorLaw::Membership,
            },
            Certificate::None,
            Vec::new(),
        ),
        application(
            &fixture.context,
            Primitive::ContextClose,
            fact,
            Certificate::Close {
                frame: local.clone(),
            },
            vec![0, 1],
        ),
    ];
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &steps, &mut budget()?)?;
    assert_eq!(
        witness.judgment().residuals,
        vec![nrz_cas::proof::GuardedObligation {
            home: fixture.context.clone(),
            scope: Vec::new(),
            layer: Layer::Logic,
            space: None,
            region: domain,
            predicate
        }]
    );
    assert_eq!(witness.hypotheses(), []);
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let mut fresh = Cas::new(budget()?.limits().clone())?;
    let replayed = fresh.inspect(&bytes, &mut budget()?)?;
    assert!(!replayed.is_discharged());
    assert_eq!(fresh.export_witness(&replayed, &mut budget()?)?, bytes);
    let mut wrong = witness.judgment().clone();
    wrong.residuals.clear();
    assert!(fixture.cas.replay(&bytes, &wrong, &mut budget()?).is_err());
    let sibling = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(fixture.context.clone()),
            region: Reference::Existing(region),
            hypotheses: Vec::new(),
        }),
        &mut budget()?,
    )?;
    let mut stolen = steps.first().ok_or("missing hypothesis step")?.clone();
    stolen.context = sibling.clone();
    assert!(
        fixture
            .cas
            .check_proof(&sibling, &[stolen], &mut budget()?)
            .is_err()
    );
    Ok(())
}

/// A local frame count and depth both refuse before publication at one less than needed.
/// ´test:integration:local-context-resource-boundaries´
#[test]
fn local_context_resource_boundaries() -> TestResult {
    let mut fixture = Fixture::new()?;
    let draft = Draft::Context(ContextDraft::Local {
        parent: Reference::Existing(fixture.context.clone()),
        region: Reference::Existing(fixture.ambient.clone()),
        hypotheses: Vec::new(),
    });
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    for limits in [
        Limits {
            local_contexts: 0,
            ..budget()?.limits().clone()
        },
        Limits {
            hypothesis_depth: 0,
            ..budget()?.limits().clone()
        },
    ] {
        assert!(
            fixture
                .cas
                .admit(std::slice::from_ref(&draft), &mut Budget::new(limits)?)
                .is_err()
        );
        assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    }
    let local = admit(&mut fixture.cas, draft, &mut budget()?)?;
    let nested = Draft::Context(ContextDraft::Local {
        parent: Reference::Existing(local),
        region: Reference::Existing(fixture.ambient.clone()),
        hypotheses: Vec::new(),
    });
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    assert!(
        fixture
            .cas
            .admit(
                std::slice::from_ref(&nested),
                &mut Budget::new(Limits {
                    hypothesis_depth: 1,
                    ..budget()?.limits().clone()
                })?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    let nested = admit(
        &mut fixture.cas,
        nested,
        &mut Budget::new(Limits {
            hypothesis_depth: 2,
            ..budget()?.limits().clone()
        })?,
    )?;
    let step = application(
        &nested,
        Primitive::ContextHypothesis,
        Statement::Subset {
            subset: fixture.cas.admissible_region(&nested)?,
            superset: fixture.ambient.clone(),
        },
        Certificate::Hypothesis(HypothesisRef {
            context: nested.clone(),
            slot: 0,
        }),
        Vec::new(),
    );
    let witness = fixture.cas.check_proof(&nested, &[step], &mut budget()?)?;
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let mut receiver = Cas::new(budget()?.limits().clone())?;
    let before = receiver.canonical_snapshot(&mut budget()?)?;
    let mut short = Budget::new(Limits {
        local_contexts: 1,
        ..budget()?.limits().clone()
    })?;
    let fault = receiver
        .inspect(&bytes, &mut short)
        .err()
        .ok_or("one frame budget accepted two")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::LocalContexts,
            ..
        }
    ));
    assert_eq!(receiver.canonical_snapshot(&mut budget()?)?, before);
    let mut exact = Budget::new(Limits {
        local_contexts: 2,
        ..budget()?.limits().clone()
    })?;
    receiver.inspect(&bytes, &mut exact)?;
    assert_eq!(exact.usage().local_contexts, 2);
    Ok(())
}

/// Native arm syntax shares values without claiming a cover or inventing a source receipt.
/// ´test:integration:native-case-admission-boundary´
#[test]
fn native_case_admission_boundary() -> TestResult {
    let mut fixture = Fixture::new()?;
    let first = admit(
        &mut fixture.cas,
        Draft::CaseArm(CaseArmDraft {
            placements: Vec::new(),
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            region: Reference::Existing(fixture.ambient.clone()),
            value: Reference::Existing(fixture.zero.clone()),
        }),
        &mut budget()?,
    )?;
    let second = admit(
        &mut fixture.cas,
        Draft::CaseArm(CaseArmDraft {
            placements: Vec::new(),
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            region: Reference::Existing(fixture.ambient.clone()),
            value: Reference::Existing(fixture.one.clone()),
        }),
        &mut budget()?,
    )?;
    let cases = Draft::Cases(CasesDraft {
        header: Reference::Existing(fixture.header.clone()),
        scope: Vec::new(),
        domain: Reference::Existing(fixture.ambient.clone()),
        arms: vec![Reference::Existing(first), Reference::Existing(second)],
    });
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    assert!(
        fixture
            .cas
            .admit(
                std::slice::from_ref(&cases),
                &mut Budget::new(Limits {
                    case_arms: 1,
                    ..budget()?.limits().clone()
                })?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    let cases = admit(
        &mut fixture.cas,
        cases,
        &mut Budget::new(Limits {
            case_arms: 2,
            ..budget()?.limits().clone()
        })?,
    )?;
    assert_eq!(*fixture.cas.expression(&cases)?.0, Operator::Cases);
    let root = fixture
        .cas
        .materialize_derived(&cases, &cases, &mut budget()?)?;
    let receipt = fixture.cas.walk_derived(&root, &mut budget()?)?.receipt;
    assert!(!receipt.is_original());
    assert!(receipt.visited() > 3);
    Ok(())
}

/// The removed wire version is refused explicitly and publishes no imported prefix.
/// ´test:integration:old-witness-version-refusal´
#[test]
fn old_witness_version_refusal() -> TestResult {
    let mut cas = Cas::new(budget()?.limits().clone())?;
    let before = cas.canonical_snapshot(&mut budget()?)?;
    let fault = cas
        .inspect(b"NRZCAS01", &mut budget()?)
        .err()
        .ok_or("accepted removed wire version")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedVersion)
    );
    assert_eq!(cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Construct independent integer-affine coefficients using the public strict grammar.
fn integer_affine(fixture: &mut Fixture, slope: i64, intercept: i64) -> TestResult<Handle> {
    let sort = nrz_cas::syntax::Sort::Value(nrz_cas::syntax::AtomicSort::Integer);
    let slope = expression(
        &mut fixture.cas,
        &fixture.context,
        Operator::Rational(Rational::from_ratio(slope, 1, &mut budget()?)?),
        sort.clone(),
        &[],
        &mut budget()?,
    )?;
    let intercept = expression(
        &mut fixture.cas,
        &fixture.context,
        Operator::Rational(Rational::from_ratio(intercept, 1, &mut budget()?)?),
        sort.clone(),
        &[],
        &mut budget()?,
    )?;
    let product = expression(
        &mut fixture.cas,
        &fixture.context,
        Operator::Mul,
        sort.clone(),
        &[slope, fixture.x.clone()],
        &mut budget()?,
    )?;
    expression(
        &mut fixture.cas,
        &fixture.context,
        Operator::Add,
        sort,
        &[product, intercept],
        &mut budget()?,
    )
}

/// Exact integer cells prove both hand-rounded inequalities, including negative bounds.
/// ´test:integration:integer-affine-rounding´
#[test]
fn integer_affine_rounding() -> TestResult {
    let mut fixture = Fixture::axis(true)?;
    let first = integer_affine(&mut fixture, 3, -1)?;
    let rounded = integer_affine(&mut fixture, 1, -1)?;
    let first = fixture.predicate(PredicateKind::NonNegative, &[first])?;
    let rounded = fixture.predicate(PredicateKind::NonNegative, &[rounded])?;
    let first = fixture.guarded(&first)?;
    let rounded = fixture.guarded(&rounded)?;
    let cuts = vec![Rational::from_ratio(1, 3, &mut budget()?)?, Rational::one()];
    let row = vec![false, false, false, true, true];
    for (left, right) in [(&first, &rounded), (&rounded, &first)] {
        let step = application(
            &fixture.context,
            Primitive::RegionDecide,
            Statement::Subset {
                subset: left.clone(),
                superset: right.clone(),
            },
            Certificate::Region(RegionCertificate {
                cuts: cuts.clone(),
                membership: vec![row.clone(), row.clone()],
            }),
            Vec::new(),
        );
        fixture
            .cas
            .check_proof(&fixture.context, &[step], &mut budget()?)?;
    }
    let first = integer_affine(&mut fixture, 2, 3)?;
    let rounded = integer_affine(&mut fixture, -1, -2)?;
    let nonnegative = fixture.predicate(PredicateKind::NonNegative, &[first])?;
    let first = fixture.region(RegionExpr::GuardFalse {
        base: Reference::Existing(fixture.ambient.clone()),
        predicate: Reference::Existing(nonnegative),
    })?;
    let rounded = fixture.predicate(PredicateKind::NonNegative, &[rounded])?;
    let rounded = fixture.guarded(&rounded)?;
    let cuts = vec![
        Rational::from_ratio(-2, 1, &mut budget()?)?,
        Rational::from_ratio(-3, 2, &mut budget()?)?,
    ];
    let row = vec![true, true, false, false, false];
    for (left, right) in [(&first, &rounded), (&rounded, &first)] {
        let step = application(
            &fixture.context,
            Primitive::RegionDecide,
            Statement::Subset {
                subset: left.clone(),
                superset: right.clone(),
            },
            Certificate::Region(RegionCertificate {
                cuts: cuts.clone(),
                membership: vec![row.clone(), row.clone()],
            }),
            Vec::new(),
        );
        fixture
            .cas
            .check_proof(&fixture.context, &[step], &mut budget()?)?;
    }
    Ok(())
}

/// A finite integer range includes its lower endpoint and excludes its upper endpoint.
/// ´test:integration:integer-range-endpoints´
#[test]
fn integer_range_endpoints() -> TestResult {
    let mut fixture = Fixture::axis(true)?;
    let lower = integer_affine(&mut fixture, 0, -2)?;
    let upper = integer_affine(&mut fixture, 0, 3)?;
    let predicate = fixture.predicate(
        PredicateKind::IntegerRange,
        &[fixture.x.clone(), lower, upper],
    )?;
    let region = fixture.guarded(&predicate)?;
    let cuts = vec![
        Rational::from_ratio(-2, 1, &mut budget()?)?,
        Rational::from_ratio(3, 1, &mut budget()?)?,
    ];
    let step = application(
        &fixture.context,
        Primitive::RegionDecide,
        Statement::Subset {
            subset: region,
            superset: fixture.ambient.clone(),
        },
        Certificate::Region(RegionCertificate {
            cuts,
            membership: vec![
                vec![false, true, true, false, false],
                vec![true, true, true, true, true],
            ],
        }),
        Vec::new(),
    );
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &[step], &mut budget()?)?;
    assert!(witness.is_discharged());
    Ok(())
}

/// Earlier spaces support conservative headers; shadows and forward dependencies refuse.
/// ´test:integration:header-space-acyclicity´
#[test]
fn header_space_acyclicity() -> TestResult {
    use nrz_cas::domain::{DeclarationSpaceDraft, SpaceDraft};
    use nrz_cas::syntax::{Declaration, ExpressionDraft, Sort};
    let mut fixture = Fixture::new()?;
    let space = admit(
        &mut fixture.cas,
        Draft::Space(SpaceDraft {
            header: Reference::Existing(fixture.header.clone()),
            domain: Reference::Existing(fixture.ambient.clone()),
            coordinate: "x".into(),
            field: Field::Real,
            test_family: "CompactSmooth".into(),
        }),
        &mut budget()?,
    )?;
    let declaration = Declaration {
        key: "phi".into(),
        display_name: "phi".into(),
        sort: Sort::Test,
    };
    let extended = admit(
        &mut fixture.cas,
        Draft::Header(HeaderDraft::Extend {
            parent: Reference::Existing(fixture.header.clone()),
            declarations: vec![declaration.clone()],
            spaces: vec![DeclarationSpaceDraft {
                key: "phi".into(),
                space: Reference::Existing(space),
            }],
        }),
        &mut budget()?,
    )?;
    let test = admit(
        &mut fixture.cas,
        Draft::Expression(ExpressionDraft {
            space: None,
            header: Reference::Existing(extended.clone()),
            scope: Vec::new(),
            sort: Sort::Test,
            operator: Operator::Free("phi".into()),
            binding: None,
            operands: Vec::new(),
        }),
        &mut budget()?,
    )?;
    assert_eq!(fixture.cas.expression_header(&test)?, extended);
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    assert!(
        fixture
            .cas
            .admit(
                &[Draft::Header(HeaderDraft::Extend {
                    parent: Reference::Existing(fixture.header.clone()),
                    declarations: vec![Declaration {
                        key: "x".into(),
                        display_name: "changed".into(),
                        sort: real()
                    }],
                    spaces: Vec::new()
                })],
                &mut budget()?
            )
            .is_err()
    );
    assert!(
        fixture
            .cas
            .admit(
                &[Draft::Header(HeaderDraft::Extend {
                    parent: Reference::Existing(fixture.header.clone()),
                    declarations: vec![declaration],
                    spaces: vec![DeclarationSpaceDraft {
                        key: "phi".into(),
                        space: Reference::Local(0)
                    }]
                })],
                &mut budget()?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Restriction transports a theorem explicitly while retaining the actual syntax header.
/// ´test:integration:explicit-context-restriction´
#[test]
fn explicit_context_restriction() -> TestResult {
    let mut fixture = Fixture::new()?;
    let predicate = fixture.predicate(PredicateKind::Positive, &[fixture.x.clone()])?;
    let positive = fixture.guarded(&predicate)?;
    let local = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(fixture.context.clone()),
            region: Reference::Existing(positive),
            hypotheses: Vec::new(),
        }),
        &mut budget()?,
    )?;
    let restricted = fixture.cas.admissible_region(&local)?;
    let original = Statement::Equality {
        left: fixture.x.clone(),
        right: fixture.x.clone(),
        domain: fixture.ambient.clone(),
        domain_contract: nrz_cas::proof::DomainContract::DefinedAndEqual,
    };
    let expected = Statement::Equality {
        left: fixture.x.clone(),
        right: fixture.x.clone(),
        domain: restricted.clone(),
        domain_contract: nrz_cas::proof::DomainContract::DefinedAndEqual,
    };
    let steps = vec![
        application(
            &fixture.context,
            Primitive::Reflexivity,
            original,
            Certificate::None,
            Vec::new(),
        ),
        application(
            &local,
            Primitive::RegionConstruct,
            Statement::Subset {
                subset: restricted,
                superset: fixture.ambient.clone(),
            },
            Certificate::None,
            Vec::new(),
        ),
        application(
            &local,
            Primitive::ContextWeaken,
            expected.clone(),
            Certificate::None,
            vec![0, 1],
        ),
    ];
    let witness = fixture.cas.check_proof(&local, &steps, &mut budget()?)?;
    assert_eq!(witness.judgment().statement, expected);
    assert_eq!(fixture.cas.expression_header(&fixture.x)?, fixture.header);
    assert!(witness.is_discharged());
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let mut fresh = Cas::new(budget()?.limits().clone())?;
    let replayed = fresh.inspect(&bytes, &mut budget()?)?;
    assert_eq!(fresh.export_witness(&replayed, &mut budget()?)?, bytes);
    let mut missing = steps.last().ok_or("missing weaken step")?.clone();
    missing.premises.clear();
    assert!(
        fixture
            .cas
            .check_proof(&local, &[missing], &mut budget()?)
            .is_err()
    );
    Ok(())
}

/// Vacuous local predicates discharge on empty regions, but empty function requests refuse.
/// ´test:integration:empty-domain-distinction´
#[test]
fn empty_domain_distinction() -> TestResult {
    let mut fixture = Fixture::new()?;
    let empty = fixture.region(RegionExpr::Empty)?;
    let predicate = fixture.predicate(PredicateKind::NonZero, &[fixture.zero.clone()])?;
    let step = application(
        &fixture.context,
        Primitive::RegionDecide,
        Statement::PredicateFact {
            region: empty.clone(),
            predicate,
        },
        Certificate::Region(RegionCertificate {
            cuts: Vec::new(),
            membership: vec![vec![false]],
        }),
        Vec::new(),
    );
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &[step], &mut budget()?)?;
    assert!(witness.is_discharged());
    let context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(fixture.header.clone()),
            admissible: Reference::Existing(empty),
            assumptions: Vec::new(),
        }),
        &mut budget()?,
    )?;
    let request = crate::testing::builders::request(
        &mut fixture.cas,
        &context,
        nrz_cas::request::Task::Simplify,
        &[fixture.one.clone()],
    )?;
    assert!(matches!(
        fixture.cas.run(&request),
        nrz_cas::request::Outcome::Refused {
            fault: nrz_cas::Fault {
                kind: FaultKind::Invalid(InvalidCode::EmptyDomain),
                ..
            },
            ..
        }
    ));
    Ok(())
}

/// Every single-record schema and every framing truncation is checked before publication.
/// ´test:integration:typed-wire-record-mutations´
#[test]
fn typed_wire_record_mutations() -> TestResult {
    let bytes = crate::testing::wire::canonical_fixture()?;
    let mut receiver = Cas::new(budget()?.limits().clone())?;
    let before = receiver.canonical_snapshot(&mut budget()?)?;
    for offset in [33, 81, 102, 140, 181, 225, 253, 278, 305, 334, 362] {
        let mut wrong_tag = bytes.clone();
        *wrong_tag.get_mut(offset).ok_or("missing record tag")? = 255;
        assert!(receiver.inspect(&wrong_tag, &mut budget()?).is_err());
        let mut wrong_schema = bytes.clone();
        *wrong_schema
            .get_mut(offset.checked_add(5).ok_or("offset overflow")?)
            .ok_or("missing schema")? = 2;
        assert!(receiver.inspect(&wrong_schema, &mut budget()?).is_err());
    }
    for length in 0..bytes.len() {
        assert!(
            receiver
                .inspect(bytes.get(..length).ok_or("missing prefix")?, &mut budget()?)
                .is_err()
        );
    }
    let mut extra = bytes;
    extra.push(0);
    assert!(receiver.inspect(&extra, &mut budget()?).is_err());
    assert_eq!(receiver.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Two rational cuts retain both singleton endpoints and certify a genuine incident interval.
/// ´test:integration:rational-incidence-cells´
#[test]
fn rational_incidence_cells() -> TestResult {
    let mut fixture = Fixture::new()?;
    let (upper, interval) = incidence_region(&mut fixture)?;
    let boundary = incidence_boundary(&mut fixture, upper)?;
    let cuts = vec![
        Rational::from_ratio(-1, 2, &mut budget()?)?,
        Rational::from_ratio(3, 2, &mut budget()?)?,
    ];
    let step = application(
        &fixture.context,
        Primitive::RegionDecide,
        Statement::Incidence {
            region: fixture.ambient.clone(),
            boundary,
            side: nrz_cas::domain::Side::Below,
            neighborhood: interval,
        },
        Certificate::Region(RegionCertificate {
            cuts,
            membership: vec![
                vec![true, true, true, true, true],
                vec![false, false, true, false, false],
            ],
        }),
        Vec::new(),
    );
    let witness = fixture.cas.check_proof(
        &fixture.context,
        std::slice::from_ref(&step),
        &mut budget()?,
    )?;
    assert!(witness.is_discharged());
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    incidence_replay(&bytes)?;
    incidence_wrong_side(&mut fixture, step)?;
    Ok(())
}

/// Construct the independent open interval with exact endpoints -1/2 and 3/2.
fn incidence_region(fixture: &mut Fixture) -> TestResult<(Handle, Handle)> {
    let lower = expression(
        &mut fixture.cas,
        &fixture.context,
        Operator::Rational(Rational::from_ratio(-1, 2, &mut budget()?)?),
        real(),
        &[],
        &mut budget()?,
    )?;
    let upper = expression(
        &mut fixture.cas,
        &fixture.context,
        Operator::Rational(Rational::from_ratio(3, 2, &mut budget()?)?),
        real(),
        &[],
        &mut budget()?,
    )?;
    let from_lower = expression(
        &mut fixture.cas,
        &fixture.context,
        Operator::Sub,
        real(),
        &[fixture.x.clone(), lower],
        &mut budget()?,
    )?;
    let to_upper = expression(
        &mut fixture.cas,
        &fixture.context,
        Operator::Sub,
        real(),
        &[upper.clone(), fixture.x.clone()],
        &mut budget()?,
    )?;
    let left = fixture.predicate(PredicateKind::Positive, &[from_lower])?;
    let right = fixture.predicate(PredicateKind::Positive, &[to_upper])?;
    let left = fixture.guarded(&left)?;
    let right = fixture.guarded(&right)?;
    let interval = fixture.region(RegionExpr::Meet(vec![
        Reference::Existing(left),
        Reference::Existing(right),
    ]))?;
    Ok((upper, interval))
}

/// Check zero-budget rollback and exact-one accounting before retaining the boundary.
fn incidence_boundary(fixture: &mut Fixture, upper: Handle) -> TestResult<Handle> {
    let draft = Draft::Boundary(nrz_cas::domain::BoundaryDraft {
        header: Reference::Existing(fixture.header.clone()),
        scope: Vec::new(),
        ambient: fixture.ambient_kind.clone(),
        location: Reference::Existing(upper),
    });
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    let mut zero = Budget::new(Limits {
        boundary_nodes: 0,
        ..budget()?.limits().clone()
    })?;
    let fault = fixture
        .cas
        .admit(std::slice::from_ref(&draft), &mut zero)
        .err()
        .ok_or("zero boundary budget accepted a location")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::BoundaryNodes,
            ..
        }
    ));
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    let mut exact = Budget::new(Limits {
        boundary_nodes: 1,
        ..budget()?.limits().clone()
    })?;
    let boundary = admit(&mut fixture.cas, draft, &mut exact)?;
    assert_eq!(exact.usage().boundary_nodes, 1);
    Ok(boundary)
}

/// Check the same zero/one boundary limit in fresh and already populated replay stores.
fn incidence_replay(bytes: &[u8]) -> TestResult {
    let mut receiver = Cas::new(budget()?.limits().clone())?;
    for _ in 0..2 {
        let before = receiver.canonical_snapshot(&mut budget()?)?;
        let mut zero = Budget::new(Limits {
            boundary_nodes: 0,
            ..budget()?.limits().clone()
        })?;
        let fault = receiver
            .inspect(bytes, &mut zero)
            .err()
            .ok_or("zero boundary budget accepted replay")?;
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::BoundaryNodes,
                ..
            }
        ));
        assert_eq!(receiver.canonical_snapshot(&mut budget()?)?, before);
        let mut exact = Budget::new(Limits {
            boundary_nodes: 1,
            ..budget()?.limits().clone()
        })?;
        let replayed = receiver.inspect(bytes, &mut exact)?;
        assert!(replayed.is_discharged());
        assert_eq!(exact.usage().boundary_nodes, 1);
    }
    Ok(())
}

/// Reverse the proved side and require the original certificate to be refused.
fn incidence_wrong_side(fixture: &mut Fixture, step: ApplicationDraft) -> TestResult {
    let mut wrong = step;
    if let Statement::Incidence { side, .. } = &mut wrong.conclusion {
        *side = nrz_cas::domain::Side::Above;
    }
    assert!(
        fixture
            .cas
            .check_proof(&fixture.context, &[wrong], &mut budget()?)
            .is_err()
    );
    Ok(())
}

/// New and imported region records charge the exact arena count before publication.
/// ´test:integration:region-arena-boundaries´
#[test]
fn region_arena_boundaries() -> TestResult {
    let mut fixture = Fixture::new()?;
    let draft = Draft::Region(RegionDraft {
        header: Reference::Existing(fixture.header.clone()),
        scope: Vec::new(),
        ambient: fixture.ambient_kind.clone(),
        expression: RegionExpr::Empty,
    });
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    let mut zero = Budget::new(Limits {
        region_nodes: 0,
        ..budget()?.limits().clone()
    })?;
    let fault = fixture
        .cas
        .admit(std::slice::from_ref(&draft), &mut zero)
        .err()
        .ok_or("zero region budget accepted a new region")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::RegionNodes,
            ..
        }
    ));
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    let mut one = Budget::new(Limits {
        region_nodes: 1,
        ..budget()?.limits().clone()
    })?;
    let region = admit(&mut fixture.cas, draft, &mut one)?;
    assert_eq!(one.usage().region_nodes, 1);
    let step = application(
        &fixture.context,
        Primitive::RegionConstruct,
        Statement::Empty { region },
        Certificate::None,
        Vec::new(),
    );
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &[step], &mut budget()?)?;
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let mut receiver = Cas::new(budget()?.limits().clone())?;
    let before = receiver.canonical_snapshot(&mut budget()?)?;
    let mut short = Budget::new(Limits {
        region_nodes: 1,
        ..budget()?.limits().clone()
    })?;
    let fault = receiver
        .inspect(&bytes, &mut short)
        .err()
        .ok_or("one region budget accepted two")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::RegionNodes,
            ..
        }
    ));
    assert_eq!(receiver.canonical_snapshot(&mut budget()?)?, before);
    let mut exact = Budget::new(Limits {
        region_nodes: 2,
        ..budget()?.limits().clone()
    })?;
    receiver.inspect(&bytes, &mut exact)?;
    assert_eq!(exact.usage().region_nodes, 2);
    Ok(())
}

/// The compact contradiction agrees with the independent exhaustive affine-cell checker.
/// ´test:integration:emptiness-sign-agrees-with-cell-oracle´
#[test]
fn emptiness_sign_agrees_with_cell_oracle() -> TestResult {
    use nrz_cas::proof::{EmptinessDag, EmptyEvidence, EmptyFact, EmptyLeaf, EmptyNode};
    let mut fixture = Fixture::new()?;
    let positive = fixture.predicate(PredicateKind::Positive, &[fixture.x.clone()])?;
    let positive = fixture.guarded(&positive)?;
    let zero = fixture.predicate(
        PredicateKind::Equal,
        &[fixture.x.clone(), fixture.zero.clone()],
    )?;
    let empty = fixture.region(RegionExpr::GuardTrue {
        base: Reference::Existing(positive.clone()),
        predicate: Reference::Existing(zero),
    })?;
    let node = EmptyNode {
        region: empty.clone(),
        evidence: EmptyEvidence::Leaf(Box::new(EmptyLeaf::Sign {
            subject: fixture.x.clone(),
            nonzero: EmptyFact::Guard(positive),
        })),
    };
    let mut step = application(
        &fixture.context,
        Primitive::RegionDecide,
        Statement::Empty { region: empty },
        Certificate::Emptiness(Box::new(EmptinessDag { nodes: vec![node] })),
        Vec::new(),
    );
    let compact = fixture.cas.check_proof(
        &fixture.context,
        std::slice::from_ref(&step),
        &mut budget()?,
    )?;
    step.certificate = Certificate::Region(RegionCertificate {
        cuts: vec![Rational::zero()],
        membership: vec![vec![false, false, false]],
    });
    let exhaustive = fixture
        .cas
        .check_proof(&fixture.context, &[step], &mut budget()?)?;
    assert_eq!(compact.judgment(), exhaustive.judgment());
    Ok(())
}
