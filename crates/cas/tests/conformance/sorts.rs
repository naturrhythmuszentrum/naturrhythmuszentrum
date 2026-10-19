// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit scalar-space, family and cell admission laws and mutations.

use crate::testing::common::TestResult;
use crate::testing::sorts::{Fixture, admit, expression, reference};
use nrz_cas::domain::{CaseArmDraft, CasesDraft};
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::solving::{
    CoefficientEvaluationData, CoordinateRole, CoordinateSpaceData, EvaluationBinding,
    EvaluationTarget, FamilyArgumentRole, PlacementUse, RealCellData, RealEndpoint,
    ScalarSpaceData, SolveDraft, SolveView, SpacePlacementData,
};
use nrz_cas::syntax::{AtomicSort, Draft, ExpressionDraft, Field, Operator, Sort, VertexKind};
use nrz_cas::{FaultKind, InvalidCode, Reference};

/// A Real declaration retains its own checked ambient without a placement or case wrapper.
/// ´test:integration:scalar-own-space-is-real´
#[test]
fn scalar_own_space_is_real() -> TestResult {
    let fixture = Fixture::new()?;
    assert_eq!(
        fixture.cas.scalar_ambient(&fixture.real)?,
        Some(fixture.real_space)
    );
    let descriptor = fixture
        .cas
        .describe_syntax(&fixture.real, &mut Budget::new(Limits::default())?)?;
    let nrz_cas::source::SyntaxNode::Expression(term) = descriptor.node else {
        return Err("not scalar syntax".into());
    };
    assert_eq!(term.operator, Operator::Free("r".into()));
    assert_eq!(term.sort, Sort::Value(AtomicSort::Scalar(Field::Real)));
    let targets: Vec<_> = descriptor
        .dependencies
        .into_iter()
        .map(|dependency| dependency.target)
        .collect();
    assert_eq!(targets, [] as [nrz_cas::Handle; 0]);
    Ok(())
}

/// Each declared coordinate slot resolves to its exact installed node.
/// ´test:integration:scalar-slot-references-are-exact´
#[test]
fn scalar_slot_references_are_exact() -> TestResult {
    let mut fixture = Fixture::new()?;
    let frame = fixture.coordinates(&[
        ("r", Field::Real, CoordinateRole::Coefficient),
        ("c", Field::Complex, CoordinateRole::Coefficient),
    ])?;
    let SolveView::CoordinateSpace(value) = fixture.cas.solve_view(&frame)? else {
        return Err("wrong frame".into());
    };
    assert_eq!(
        value.scalar_spaces,
        vec![
            Some(fixture.real_space.clone()),
            Some(fixture.complex_space.clone())
        ]
    );
    let mut altered = CoordinateSpaceData {
        scalar_spaces: value
            .scalar_spaces
            .iter()
            .map(|space| space.as_ref().map(reference))
            .collect(),
        header: reference(&value.header),
        coordinates: value.coordinates.clone(),
    };
    altered.scalar_spaces.swap(0, 1);
    let draft = Draft::Solve(SolveDraft::CoordinateSpace(altered));
    let fault = fixture
        .cas
        .admit(&[draft], &mut Budget::new(Limits::default())?)
        .expect_err("false slot ambient accepted");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::AmbientSpaceMismatch)
    );
    Ok(())
}

/// Missing registration is distinct from a malformed registry identity.
/// ´test:integration:scalar-registration-refusals-are-named´
#[test]
fn scalar_registration_refusals_are_named() -> TestResult {
    let mut fixture = Fixture::new()?;
    for (name, code) in [
        ("scalar.future", InvalidCode::SpaceRegistrationUnavailable),
        ("", InvalidCode::Encoding),
    ] {
        let before = fixture.cas.counts();
        let draft = Draft::Solve(SolveDraft::ScalarSpace(ScalarSpaceData {
            specification: name.into(),
            version: 1,
        }));
        let fault = fixture
            .cas
            .admit(&[draft], &mut Budget::new(Limits::default())?)
            .expect_err("invalid registration accepted");
        assert_eq!(fault.kind, FaultKind::Invalid(code));
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// Reverse and invented placement paths do not follow from a registered directed edge.
/// ´test:integration:scalar-placement-is-directed´
#[test]
fn scalar_placement_is_directed() -> TestResult {
    let mut fixture = Fixture::new()?;
    for (name, source, ambient, code) in [
        (
            "placement.complex-in-real",
            fixture.complex_space.clone(),
            fixture.real_space.clone(),
            InvalidCode::SpacePlacementUnavailable,
        ),
        (
            "placement.real-in-complex",
            fixture.complex_space.clone(),
            fixture.real_space.clone(),
            InvalidCode::AmbientSpaceMismatch,
        ),
    ] {
        let draft = Draft::Solve(SolveDraft::SpacePlacement(SpacePlacementData {
            source: reference(&source),
            ambient: reference(&ambient),
            specification: name.into(),
            version: 1,
        }));
        let fault = fixture
            .cas
            .admit(&[draft], &mut Budget::new(Limits::default())?)
            .expect_err("invented placement accepted");
        assert_eq!(fault.kind, FaultKind::Invalid(code));
    }
    Ok(())
}

/// A mixed local body is retained through a native arm with paired placement support.
/// ´test:integration:scalar-family-arm-retains-original-operands´
#[test]
fn scalar_family_arm_retains_original_operands() -> TestResult {
    let mut fixture = Fixture::new()?;
    let shape = fixture.family()?;
    let body = fixture.body(&shape)?;
    let piece = fixture.piece(&shape.region)?;
    let arm = admit(
        &mut fixture.cas,
        Draft::CaseArm(CaseArmDraft {
            placements: vec![PlacementUse {
                operand: reference(&fixture.real),
                piece: reference(&piece),
            }],
            header: reference(&fixture.header),
            scope: Vec::new(),
            region: reference(&shape.region),
            value: reference(&body),
        }),
    )?;
    let cases = admit(
        &mut fixture.cas,
        Draft::Cases(CasesDraft {
            header: reference(&fixture.header),
            scope: Vec::new(),
            domain: reference(&shape.region),
            arms: vec![reference(&arm)],
        }),
    )?;
    assert_eq!(
        fixture.cas.scalar_ambient(&cases)?,
        Some(fixture.complex_space)
    );
    assert_eq!(piece.logical_id().kind, VertexKind::ScalarPiece);
    let SolveView::FamilyExpression(value) = fixture.cas.solve_view(&body)? else {
        return Err("body missing".into());
    };
    assert_eq!(value.arguments[0].value, fixture.real);
    assert_eq!(value.arguments[0].role, FamilyArgumentRole::Base);
    assert_eq!(value.arguments[1].value, fixture.complex);
    Ok(())
}

/// A body with a Real base operand cannot discard its placement association.
/// ´test:integration:scalar-family-arm-refuses-missing-placement´
#[test]
fn scalar_family_arm_refuses_missing_placement() -> TestResult {
    let mut fixture = Fixture::new()?;
    let shape = fixture.family()?;
    let body = fixture.body(&shape)?;
    let before = fixture.cas.counts();
    let draft = Draft::CaseArm(CaseArmDraft {
        placements: Vec::new(),
        header: reference(&fixture.header),
        scope: Vec::new(),
        region: reference(&shape.region),
        value: reference(&body),
    });
    assert!(
        fixture
            .cas
            .admit(&[draft], &mut Budget::new(Limits::default())?)
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Finite Real endpoints remain independent expressions and infinite endpoints must be open.
/// ´test:integration:scalar-real-cell-checks-endpoint-sorts´
#[test]
fn scalar_real_cell_checks_endpoint_sorts() -> TestResult {
    let mut fixture = Fixture::new()?;
    let shape = fixture.family()?;
    let cell = RealCellData {
        header: reference(&fixture.header),
        scope: Vec::new(),
        region: reference(&shape.base_region),
        left: RealEndpoint::RealExpression(reference(&fixture.real)),
        right: RealEndpoint::PositiveInfinity,
        left_open: true,
        right_open: true,
    };
    assert_eq!(
        admit(
            &mut fixture.cas,
            Draft::Solve(SolveDraft::RealCell(cell.clone()))
        )?
        .logical_id()
        .kind,
        VertexKind::RealCell
    );
    let mut bad = cell.clone();
    bad.right_open = false;
    assert!(
        fixture
            .cas
            .admit(
                &[Draft::Solve(SolveDraft::RealCell(bad))],
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    let mut bad = cell;
    bad.left = RealEndpoint::RealExpression(reference(&fixture.complex));
    assert!(
        fixture
            .cas
            .admit(
                &[Draft::Solve(SolveDraft::RealCell(bad))],
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    Ok(())
}

/// Finite evaluation is complete and same-sort at every original source slot.
/// ´test:integration:scalar-evaluation-refuses-retyped-image´
#[test]
fn scalar_evaluation_refuses_retyped_image() -> TestResult {
    let mut fixture = Fixture::new()?;
    let shape = fixture.family()?;
    let evaluation = CoefficientEvaluationData {
        source: reference(&shape.parameters),
        target: EvaluationTarget::Scalar {
            header: reference(&fixture.header),
            space: reference(&fixture.real_space),
        },
        region: reference(&shape.base_region),
        bindings: vec![EvaluationBinding {
            slot: 0,
            image: reference(&fixture.real),
        }],
        localization: Vec::new(),
    };
    assert_eq!(
        admit(
            &mut fixture.cas,
            Draft::Solve(SolveDraft::CoefficientEvaluation(evaluation.clone()))
        )?
        .logical_id()
        .kind,
        VertexKind::CoefficientEvaluation
    );
    let mut bad = evaluation.clone();
    bad.bindings[0].image = reference(&fixture.complex);
    assert!(
        fixture
            .cas
            .admit(
                &[Draft::Solve(SolveDraft::CoefficientEvaluation(bad))],
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    let mut bad = evaluation;
    bad.bindings.clear();
    assert!(
        fixture
            .cas
            .admit(
                &[Draft::Solve(SolveDraft::CoefficientEvaluation(bad))],
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    Ok(())
}

/// Exact same-sort substitution composes without changing a declaration ambient.
/// ´test:integration:scalar-same-sort-substitution-composes´
#[test]
fn scalar_same_sort_substitution_composes() -> TestResult {
    let mut fixture = Fixture::new()?;
    let zero = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        Operator::Rational(nrz_cas::Rational::zero()),
        Vec::new(),
        Vec::new(),
    )?;
    let substitution = nrz_cas::syntax::Substitution {
        key: "r".into(),
        replacement: zero.clone(),
    };
    let replaced = fixture.cas.substitute(
        &fixture.real,
        std::slice::from_ref(&substitution),
        &mut Budget::new(Limits::default())?,
    )?;
    assert_eq!(replaced, zero);
    assert_eq!(
        fixture.cas.substitute(
            &replaced,
            &[substitution],
            &mut Budget::new(Limits::default())?
        )?,
        zero
    );
    let wrong = nrz_cas::syntax::Substitution {
        key: "r".into(),
        replacement: fixture.complex,
    };
    assert!(
        fixture
            .cas
            .substitute(
                &fixture.real,
                &[wrong],
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    Ok(())
}

/// Local role and ambient claims are independently checked against original operand sorts.
/// ´test:integration:scalar-family-rejects-false-role-and-ambient´
#[test]
fn scalar_family_rejects_false_role_and_ambient() -> TestResult {
    let mut fixture = Fixture::new()?;
    let shape = fixture.family()?;
    let mut role = fixture.body_data(&shape);
    role.arguments[0].role = FamilyArgumentRole::Fibre;
    let fault = fixture
        .cas
        .admit(
            &[Draft::Solve(SolveDraft::FamilyExpression(role))],
            &mut Budget::new(Limits::default())?,
        )
        .expect_err("false family role accepted");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::SymbolSortMismatch)
    );
    let mut ambient = fixture.body_data(&shape);
    ambient.ambient = reference(&fixture.real_space);
    let fault = fixture
        .cas
        .admit(
            &[Draft::Solve(SolveDraft::FamilyExpression(ambient))],
            &mut Budget::new(Limits::default())?,
        )
        .expect_err("false family ambient accepted");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::AmbientSpaceMismatch)
    );
    Ok(())
}

/// Declarations and de Bruijn slots cannot be retyped by a result-sort claim.
/// ´test:integration:scalar-rejects-retyped-symbol-and-binder´
#[test]
fn scalar_rejects_retyped_symbol_and_binder() -> TestResult {
    let mut fixture = Fixture::new()?;
    for (operator, scope) in [
        (Operator::Free("r".into()), Vec::new()),
        (
            Operator::Bound { depth: 0, slot: 0 },
            vec![vec![AtomicSort::Scalar(Field::Real)]],
        ),
    ] {
        let draft = Draft::Expression(ExpressionDraft {
            space: None,
            operator,
            sort: Sort::Value(AtomicSort::Scalar(Field::Complex)),
            header: reference(&fixture.header),
            scope,
            binding: None,
            operands: Vec::new(),
        });
        assert!(
            fixture
                .cas
                .admit(&[draft], &mut Budget::new(Limits::default())?)
                .is_err()
        );
    }
    Ok(())
}

/// A mixed evaluation target keeps each base/fibre image in its original local space.
/// ´test:integration:scalar-family-evaluation-retains-local-sorts´
#[test]
fn scalar_family_evaluation_retains_local_sorts() -> TestResult {
    let mut fixture = Fixture::new()?;
    let shape = fixture.family()?;
    let evaluation = CoefficientEvaluationData {
        source: reference(&shape.total),
        target: EvaluationTarget::Family {
            family: reference(&shape.handle),
        },
        region: reference(&shape.region),
        bindings: vec![
            EvaluationBinding {
                slot: 0,
                image: reference(&fixture.real),
            },
            EvaluationBinding {
                slot: 1,
                image: reference(&fixture.complex),
            },
        ],
        localization: Vec::new(),
    };
    let value = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::CoefficientEvaluation(evaluation.clone())),
    )?;
    assert_eq!(value.logical_id().kind, VertexKind::CoefficientEvaluation);
    let mut missing = evaluation;
    missing.bindings[0].slot = 1;
    assert!(
        fixture
            .cas
            .admit(
                &[Draft::Solve(SolveDraft::CoefficientEvaluation(missing))],
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    Ok(())
}

/// Foreign and forward registry references refuse the whole structural transaction.
/// ´test:integration:scalar-reference-refusal-rolls-back´
#[test]
fn scalar_reference_refusal_rolls_back() -> TestResult {
    let mut fixture = Fixture::new()?;
    let other = Fixture::new()?;
    let before = fixture.cas.counts();
    for source in [reference(&other.real_space), Reference::Local(0)] {
        let draft = Draft::Solve(SolveDraft::SpacePlacement(SpacePlacementData {
            source,
            ambient: reference(&fixture.complex_space),
            specification: "placement.real-in-complex".into(),
            version: 1,
        }));
        assert!(
            fixture
                .cas
                .admit(&[draft], &mut Budget::new(Limits::default())?)
                .is_err()
        );
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// The measured successful space admission has an exact cap-minus-one rollback boundary.
/// ´test:integration:scalar-admission-work-cap-minus-one´
#[test]
fn scalar_admission_work_cap_minus_one() -> TestResult {
    let draft = Draft::Solve(SolveDraft::ScalarSpace(ScalarSpaceData {
        specification: "scalar.real".into(),
        version: 1,
    }));
    let mut first = nrz_cas::Cas::new(Limits::default())?;
    let mut measured = Budget::new(Limits::default())?;
    first.admit(std::slice::from_ref(&draft), &mut measured)?;
    let cap = measured
        .usage()
        .work
        .checked_sub(1)
        .ok_or("zero admission work")?;
    let mut second = nrz_cas::Cas::new(Limits::default())?;
    let before = second.counts();
    let mut limited = Budget::new(Limits {
        work: cap,
        ..Limits::default()
    })?;
    let fault = second
        .admit(&[draft], &mut limited)
        .expect_err("cap-minus-one accepted");
    assert!(matches!(fault.kind, FaultKind::Exhausted { .. }));
    assert_eq!(second.counts(), before);
    Ok(())
}

/// Unregistered declaration syntax cannot acquire an invented checked ambient.
/// ´test:integration:scalar-unregistered-declaration-has-no-ambient´
#[test]
fn scalar_unregistered_declaration_has_no_ambient() -> TestResult {
    use nrz_cas::domain::HeaderDraft;
    use nrz_cas::syntax::{Branch, Declaration, Environment};
    let mut fixture = Fixture::new()?;
    let field = Field::Named("future".into());
    let sort = Sort::Value(AtomicSort::Scalar(field.clone()));
    let header = admit(
        &mut fixture.cas,
        Draft::Header(HeaderDraft::Core(Environment {
            field: field.clone(),
            branch: Branch::Declared("future".into()),
            declarations: vec![Declaration {
                key: "u".into(),
                display_name: "u".into(),
                sort,
            }],
        })),
    )?;
    let value = expression(
        &mut fixture.cas,
        &header,
        field.clone(),
        Operator::Free("u".into()),
        Vec::new(),
        Vec::new(),
    )?;
    assert_eq!(fixture.cas.scalar_ambient(&value)?, None);
    let draft = Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
        scalar_spaces: vec![Some(reference(&fixture.real_space))],
        header: reference(&header),
        coordinates: vec![nrz_cas::solving::Coordinate {
            key: "u".into(),
            field,
            role: CoordinateRole::Coefficient,
        }],
    }));
    let fault = fixture
        .cas
        .admit(&[draft], &mut Budget::new(Limits::default())?)
        .expect_err("invented ambient accepted");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::AmbientSpaceMismatch)
    );
    Ok(())
}

/// Conservative extension preserves the original scalar node and forbids retyping a key.
/// ´test:integration:scalar-header-extension-is-conservative´
#[test]
fn scalar_header_extension_is_conservative() -> TestResult {
    use nrz_cas::domain::HeaderDraft;
    use nrz_cas::syntax::Declaration;
    let mut fixture = Fixture::new()?;
    let fresh = Declaration {
        key: "s".into(),
        display_name: "s".into(),
        sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
    };
    let header = admit(
        &mut fixture.cas,
        Draft::Header(HeaderDraft::Extend {
            parent: reference(&fixture.header),
            declarations: vec![fresh],
            spaces: Vec::new(),
        }),
    )?;
    let inherited = expression(
        &mut fixture.cas,
        &header,
        Field::Real,
        Operator::Free("r".into()),
        Vec::new(),
        Vec::new(),
    )?;
    assert_eq!(
        fixture.cas.scalar_ambient(&inherited)?,
        Some(fixture.real_space)
    );
    let changed = Declaration {
        key: "r".into(),
        display_name: "r".into(),
        sort: Sort::Value(AtomicSort::Scalar(Field::Complex)),
    };
    assert!(
        fixture
            .cas
            .admit(
                &[Draft::Header(HeaderDraft::Extend {
                    parent: reference(&header),
                    declarations: vec![changed],
                    spaces: Vec::new()
                })],
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    Ok(())
}

/// Renaming rebuilds the original family frames without changing any scalar role.
/// ´test:integration:scalar-family-renaming-preserves-roles´
#[test]
fn scalar_family_renaming_preserves_roles() -> TestResult {
    use nrz_cas::domain::HeaderDraft;
    use nrz_cas::source::AxisMap;
    use nrz_cas::syntax::{Branch, Declaration, Environment};
    let mut fixture = Fixture::new()?;
    let shape = fixture.family()?;
    let body = fixture.body(&shape)?;
    let declarations = [
        ("s", "r", Field::Real),
        ("d", "c", Field::Complex),
        ("w", "z", Field::Complex),
    ]
    .into_iter()
    .map(|(key, display_name, field)| Declaration {
        key: key.into(),
        display_name: display_name.into(),
        sort: Sort::Value(AtomicSort::Scalar(field)),
    })
    .collect();
    let header = admit(
        &mut fixture.cas,
        Draft::Header(HeaderDraft::Core(Environment {
            field: Field::Complex,
            branch: Branch::PrincipalComplex,
            declarations,
        })),
    )?;
    let map = AxisMap {
        coordinates: vec![
            ("r".into(), "s".into()),
            ("c".into(), "d".into()),
            ("z".into(), "w".into()),
        ],
        headers: vec![(fixture.header.clone(), header)],
        spaces: Vec::new(),
    };
    let renamed = fixture
        .cas
        .rename_axes(&body, &map, &mut Budget::new(Limits::default())?)?;
    let SolveView::FamilyExpression(value) = fixture.cas.solve_view(&renamed)? else {
        return Err("missing renamed body".into());
    };
    assert_eq!(value.ambient, fixture.complex_space);
    assert_eq!(value.arguments[0].role, FamilyArgumentRole::Base);
    assert_eq!(
        fixture.cas.scalar_ambient(&value.arguments[0].value)?,
        Some(fixture.real_space)
    );
    Ok(())
}

/// A nested body retains exactly the placement support of its original base operand.
/// ´test:integration:scalar-nested-family-keeps-placement-support´
#[test]
fn scalar_nested_family_keeps_placement_support() -> TestResult {
    use nrz_cas::solving::FamilyArgument;
    let mut fixture = Fixture::new()?;
    let shape = fixture.family()?;
    let body = fixture.body(&shape)?;
    let mut outer = fixture.body_data(&shape);
    outer.operator = Operator::Neg;
    outer.arguments = vec![FamilyArgument {
        role: FamilyArgumentRole::Family,
        value: reference(&body),
    }];
    let nested = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::FamilyExpression(outer)),
    )?;
    let piece = fixture.piece(&shape.region)?;
    let mut arm = CaseArmDraft {
        placements: Vec::new(),
        header: reference(&fixture.header),
        scope: Vec::new(),
        region: reference(&shape.region),
        value: reference(&nested),
    };
    assert!(
        fixture
            .cas
            .admit(
                &[Draft::CaseArm(arm.clone())],
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    arm.placements.push(PlacementUse {
        operand: reference(&fixture.real),
        piece: reference(&piece),
    });
    assert_eq!(
        admit(&mut fixture.cas, Draft::CaseArm(arm))?
            .logical_id()
            .kind,
        VertexKind::CaseArm
    );
    Ok(())
}

/// A Real component retains inner Complex placement and can itself be explicitly placed later.
/// ´test:integration:scalar-family-components-preserve-local-placements´
#[test]
fn scalar_family_components_preserve_local_placements() -> TestResult {
    use nrz_cas::solving::FamilyArgument;
    let mut fixture = Fixture::new()?;
    let shape = fixture.family()?;
    let body = fixture.body(&shape)?;
    let mut component = fixture.body_data(&shape);
    component.operator = Operator::RealPart;
    component.ambient = reference(&fixture.real_space);
    component.result = Sort::Value(AtomicSort::Scalar(Field::Real));
    component.arguments = vec![FamilyArgument {
        role: FamilyArgumentRole::Family,
        value: reference(&body),
    }];
    let real = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::FamilyExpression(component)),
    )?;
    let piece = fixture.piece(&shape.region)?;
    let base_use = PlacementUse {
        operand: reference(&fixture.real),
        piece: reference(&piece),
    };
    let arm = CaseArmDraft {
        placements: vec![base_use.clone()],
        header: reference(&fixture.header),
        scope: Vec::new(),
        region: reference(&shape.region),
        value: reference(&real),
    };
    admit(&mut fixture.cas, Draft::CaseArm(arm))?;
    let mut outer = fixture.body_data(&shape);
    outer.operator = Operator::Neg;
    outer.arguments = vec![FamilyArgument {
        role: FamilyArgumentRole::Family,
        value: reference(&real),
    }];
    let complex = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::FamilyExpression(outer)),
    )?;
    let arm = CaseArmDraft {
        placements: vec![
            PlacementUse {
                operand: reference(&real),
                piece: reference(&piece),
            },
            base_use,
        ],
        header: reference(&fixture.header),
        scope: Vec::new(),
        region: reference(&shape.region),
        value: reference(&complex),
    };
    admit(&mut fixture.cas, Draft::CaseArm(arm))?;
    Ok(())
}

/// Original source uses include every family value while global registry records remain interpretation dependencies.
/// ´test:integration:scalar-family-original-source-walk-is-complete´
#[test]
fn scalar_family_original_source_walk_is_complete() -> TestResult {
    use nrz_cas::proof::Statement;
    use nrz_cas::source::{SyntaxNode, WalkEvent};
    let (mut cas, witness) = crate::testing::sorts::family_witness()?;
    let Statement::PredicateFact { predicate, .. } = &witness.judgment().statement else {
        return Err("missing source predicate".into());
    };
    let predicate = predicate.clone();
    let source = admit(
        &mut cas,
        Draft::Source {
            key: "original-family".into(),
            text: "r+c".into(),
        },
    )?;
    let root = cas.materialize_original(
        &predicate,
        &source,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let walk = cas.walk_original(&root, &mut Budget::new(Limits::blind_cubic())?)?;
    let mut keys = std::collections::BTreeSet::new();
    for event in walk.events {
        if let WalkEvent::Visit(visit) = event
            && let SyntaxNode::Expression(expression) = cas
                .describe_syntax(&visit.expression, &mut Budget::new(Limits::blind_cubic())?)?
                .node
            && let Operator::Free(key) = expression.operator
        {
            keys.insert(key);
        }
    }
    assert_eq!(keys.into_iter().collect::<Vec<_>>(), ["c", "r"]);
    assert!(walk.receipt.is_original());
    Ok(())
}
