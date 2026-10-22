// SPDX-License-Identifier: AGPL-3.0-only
//! Refusal and selection boundaries remain separate from defining-ideal consequences.

use super::{
    Budget, Cas, CheckedApplication, Evaluation, Fixture, Handle, Limits, Rational, TestResult,
    Value, constant, fixture,
};
use crate::model::Field;
use crate::proof_model::Statement;
use crate::solving::{
    AdjunctionMode, CertificateRootAdjunction, SolveCertificate, SolveDraft, SolveSchema,
    SolveStatement, StatementRootExistence,
};
use crate::{Reference, syntax::Draft};

fn root_selection(cas: &Cas, root: &Handle) -> TestResult<Handle> {
    let expression = cas.state.expression(root.logical_id())?;
    let Some(crate::graph::ExpressionAttachment::SelectedRoot(selection)) = &expression.attachment
    else {
        return Err("selected root attachment".into());
    };
    Ok(cas.handle(*selection))
}

/// The replacement of the inner existence fact is pinned independently of traversal order.
/// ´test:crate:root-closure-missing-inner-existence´
#[test]
fn root_closure_missing_inner_existence() -> TestResult {
    let mut fixture = fixture()?;
    let target = fixture.target(&mut Budget::new(Limits::blind_cubic())?)?;
    let selected = root_selection(&fixture.cas, &fixture.roots[0])?;
    let index = fixture.premises.iter().position(|premise| {
        matches!(&premise.judgment.statement, Statement::Solve(statement)
            if matches!(statement.as_ref(), SolveStatement::RootExistence(value) if value.relation == selected))
    }).ok_or("inner existence")?;
    assert_eq!(fixture.premises.len(), 3);
    fixture
        .evaluation()
        .zero(&target, &mut Budget::new(Limits::blind_cubic())?)?;
    let removed = fixture.premises.remove(index);
    assert_ne!(fixture.premises.len(), 3);
    assert!(
        matches!(&removed.judgment.statement, Statement::Solve(statement)
        if matches!(statement.as_ref(), SolveStatement::RootExistence(value) if value.relation == selected))
    );
    assert!(
        fixture
            .evaluation()
            .root_relations(&target.numerator, &mut Budget::new(Limits::blind_cubic())?)
            .is_err()
    );
    Ok(())
}

/// Changing only the asserted constant leaves a nonzero remainder after the full relation closure.
/// ´test:crate:root-closure-false-equation´
#[test]
fn root_closure_false_equation() -> TestResult {
    let fixture = fixture()?;
    let evaluation = fixture.evaluation();
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let target = fixture.target(&mut budget)?;
    evaluation.zero(&target, &mut budget)?;
    let original = Rational::from_ratio(1, 27, &mut budget)?;
    let replacement = Rational::from_ratio(1, 26, &mut budget)?;
    assert_ne!(original, replacement);
    let correct = constant(&evaluation.engine, 1, 27, &mut budget)?;
    let wrong = constant(&evaluation.engine, 1, 26, &mut budget)?;
    let changed = target.sub(&correct, &evaluation.engine, &mut budget)?.add(
        &wrong,
        &evaluation.engine,
        &mut budget,
    )?;
    assert!(!changed.equals(&target, &evaluation.engine, &mut budget)?);
    assert!(evaluation.zero(&changed, &mut budget).is_err());
    Ok(())
}

/// Build the same four-node parent-map chain; its single changed image would close a cycle.
fn cyclic_drafts(fixture: &Fixture) -> TestResult<Vec<Draft>> {
    let selected = root_selection(&fixture.cas, &fixture.roots[1])?;
    let selection = super::selected::selection(&fixture.cas, &selected)?;
    let relation = super::selected::relation(&fixture.cas, &selection.relations[0])?;
    let SolveDraft::CoefficientMap(parent) =
        crate::solving::graph::object(&fixture.cas.state, relation.map.logical_id())?
    else {
        return Err("outer parent map".into());
    };
    let mapping = SolveDraft::CoefficientMap(parent.clone())
        .resolve(&mut |handle| Ok(Reference::Existing(handle.clone())))?;
    let SolveDraft::RootRelation(mut equation) = SolveDraft::RootRelation(relation.clone())
        .resolve(&mut |handle| Ok(Reference::Existing(handle.clone())))?
    else {
        return Err("root relation draft".into());
    };
    equation.map = Reference::Local(0);
    let SolveDraft::RootSelection(mut choice) = SolveDraft::RootSelection(selection.clone())
        .resolve(&mut |handle| Ok(Reference::Existing(handle.clone())))?
    else {
        return Err("root selection draft".into());
    };
    choice.relations = vec![Reference::Local(1)];
    Ok(vec![
        Draft::Solve(mapping),
        Draft::Solve(SolveDraft::RootRelation(equation)),
        Draft::Solve(SolveDraft::RootSelection(choice)),
        Draft::RootValue {
            header: Reference::Existing(selection.header.clone()),
            selection: Reference::Local(2),
            slot: 0,
        },
    ])
}

/// A parent map cannot point forward through its own relation and selection back to its root.
/// ´test:crate:root-closure-cyclic-relation-refused´
#[test]
fn root_closure_cyclic_relation_refused() -> TestResult {
    let mut fixture = fixture()?;
    let mut drafts = cyclic_drafts(&fixture)?;
    let admitted = fixture
        .cas
        .admit(&drafts, &mut Budget::new(Limits::blind_cubic())?)?;
    assert_eq!(admitted.last(), fixture.roots.get(1));
    let Some(Draft::Solve(SolveDraft::CoefficientMap(mapping))) = drafts.first_mut() else {
        return Err("cycle map".into());
    };
    assert_eq!(
        mapping.images,
        [Reference::Existing(fixture.roots[0].clone())]
    );
    let replacement = Reference::Local(3);
    assert_ne!(mapping.images[0], replacement);
    mapping.images[0] = replacement;
    let before = fixture.cas.counts();
    let fault = fixture
        .cas
        .admit(&drafts, &mut Budget::new(Limits::blind_cubic())?)
        .err()
        .ok_or("cyclic relation accepted")?;
    assert!(matches!(fault.kind, crate::FaultKind::Invalid(_)));
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Supply one equation and its independent existence theorem, without producer simplification.
fn supplied(coefficients: &[i64], real: bool) -> TestResult<Fixture> {
    let mut source = crate::testing::solve_coefficients::Fixture::new()?;
    let parameter = source.guard_expression(crate::model::Operator::Free("x".into()), &[])?;
    let [_, nonzero] = source.guard_cells(&parameter)?;
    source.on_region(&nonzero)?;
    let entries: Vec<_> = coefficients
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, coefficient)| **coefficient != 0)
        .map(|(power, coefficient)| Ok((vec![u32::try_from(power)?], *coefficient)))
        .collect::<TestResult<_>>()?;
    let polynomial = source.polynomial(1, &entries)?;
    let field = if real { Field::Real } else { Field::Complex };
    let (relation, selection) = source.root(&polynomial, &field)?;
    let premise = supplied_existence(&mut source, &relation, &selection, real)?;
    let root = crate::testing::solve_graph::admit(
        &mut source.cas,
        &Draft::RootValue {
            header: Reference::Existing(source.header.clone()),
            selection: Reference::Existing(selection),
            slot: 0,
        },
    )?;
    Ok(Fixture {
        cas: source.cas,
        domain: source.rational,
        region: source.region,
        roots: vec![root],
        premises: vec![premise],
    })
}

fn supplied_existence(
    source: &mut crate::testing::solve_coefficients::Fixture,
    relation: &Handle,
    selection: &Handle,
    real: bool,
) -> TestResult<CheckedApplication> {
    let mut construction = crate::solving::radicals::construction::Construction {
        header: source.header.clone(),
        context: source.context.clone(),
        region: source.region.clone(),
        steps: Vec::new(),
    };
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let unit = construction.unit(&mut source.cas, &source.rational, &mut budget)?;
    construction.push(
        SolveSchema::RootAdjunction,
        SolveStatement::RootExistence(StatementRootExistence {
            relation: selection.clone(),
            region: source.region.clone(),
        }),
        SolveCertificate::RootAdjunction(CertificateRootAdjunction {
            mode: if real {
                AdjunctionMode::RealOddRoot
            } else {
                AdjunctionMode::ComplexFTA
            },
            relation: relation.clone(),
            selection: Some(selection.clone()),
            leading_unit: unit,
            order: if real { Some(3) } else { None },
            splitting_chain: Vec::new(),
        }),
        &mut budget,
    )?;
    let witness = source
        .cas
        .check_proof(&source.context, &construction.steps, &mut budget)?;
    Ok(crate::proof_view::application(
        &source.cas,
        witness.proof.logical_id(),
        &mut budget,
    )?)
}

fn polynomial_value(
    evaluation: &Evaluation<'_>,
    root: &Value,
    coefficients: &[i64],
    budget: &mut Budget,
) -> TestResult<Value> {
    let mut result = Value::integer(0, &evaluation.engine, budget)?;
    for coefficient in coefficients.iter().rev() {
        result = result.mul(root, &evaluation.engine, budget)?.add(
            &constant(&evaluation.engine, *coefficient, 1, budget)?,
            &evaluation.engine,
            budget,
        )?;
    }
    Ok(result)
}

/// Reducible, irreducible, shifted and repeated equations imply themselves, never an unchecked minimal polynomial.
/// ´test:crate:root-closure-defining-polynomial-twins´
#[test]
fn root_closure_defining_polynomial_twins() -> TestResult {
    for (coefficients, real) in [
        (&[-8, 0, 0, 1][..], true),
        (&[-2, 0, 0, 1], true),
        (&[-8, 12, -6, 1], false),
        (&[4, -4, 1], false),
    ] {
        let fixture = supplied(coefficients, real)?;
        let evaluation = fixture.evaluation();
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let values = fixture.values(&mut budget)?;
        let root = &values[0];
        let equation = polynomial_value(&evaluation, root, coefficients, &mut budget)?;
        assert_eq!(
            evaluation
                .root_relations(&equation.numerator, &mut budget)?
                .len(),
            1
        );
        evaluation.zero(&equation, &mut budget)?;
        let linear = root.sub(
            &constant(&evaluation.engine, 2, 1, &mut budget)?,
            &evaluation.engine,
            &mut budget,
        )?;
        assert!(evaluation.zero(&linear, &mut budget).is_err());
    }
    Ok(())
}

/// Each pinned component equation proves only its own signed root, and the opposite component is refused.
/// ´test:crate:root-closure-selected-component-twins´
#[test]
fn root_closure_selected_component_twins() -> TestResult {
    for constant_term in [-1, 1] {
        let fixture = supplied(&[constant_term, 1], false)?;
        let evaluation = fixture.evaluation();
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let root = &fixture.values(&mut budget)?[0];
        let correct = polynomial_value(&evaluation, root, &[constant_term, 1], &mut budget)?;
        evaluation.zero(&correct, &mut budget)?;
        let replacement = -constant_term;
        assert_ne!(replacement, constant_term);
        let wrong = polynomial_value(&evaluation, root, &[replacement, 1], &mut budget)?;
        assert!(!wrong.equals(&correct, &evaluation.engine, &mut budget)?);
        assert!(evaluation.zero(&wrong, &mut budget).is_err());
    }
    Ok(())
}

/// A missing defining slot is rejected at admission, before any equation can enter the closure.
/// ´test:crate:root-closure-missing-defining-relation´
#[test]
fn root_closure_missing_defining_relation() -> TestResult {
    let mut fixture = supplied(&[-2, 0, 0, 1], true)?;
    let selected = root_selection(&fixture.cas, &fixture.roots[0])?;
    let original = super::selected::selection(&fixture.cas, &selected)?;
    let SolveDraft::RootSelection(mut changed) = SolveDraft::RootSelection(original.clone())
        .resolve(&mut |handle| Ok(Reference::Existing(handle.clone())))?
    else {
        return Err("selection draft".into());
    };
    assert_eq!(
        changed.relations,
        [Reference::Existing(original.relations[0].clone())]
    );
    let replacement: Vec<Reference> = Vec::new();
    assert_ne!(changed.relations, replacement);
    changed.relations = replacement;
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .admit(
                &[Draft::Solve(SolveDraft::RootSelection(changed))],
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Existence on x≠0 cannot justify the same equation on its sibling x=0 region.
/// ´test:crate:root-closure-sibling-region-refused´
#[test]
fn root_closure_sibling_region_refused() -> TestResult {
    use crate::domain_model::{RegionDraft, RegionExpr};
    let mut fixture = supplied(&[-2, 0, 0, 1], true)?;
    let original = fixture.cas.state.region(fixture.region.logical_id())?;
    let RegionExpr::GuardFalse { base, predicate } = original.expression else {
        return Err("original nonzero cell".into());
    };
    let draft = Draft::Region(RegionDraft {
        header: Reference::Existing(fixture.cas.handle(original.header)),
        scope: original.scope.clone(),
        ambient: original.ambient.clone(),
        expression: RegionExpr::GuardTrue {
            base: Reference::Existing(fixture.cas.handle(base)),
            predicate: Reference::Existing(fixture.cas.handle(predicate)),
        },
    });
    let sibling = crate::testing::solve_graph::admit(&mut fixture.cas, &draft)?;
    assert_ne!(fixture.region, sibling);
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let target = fixture.values(&mut budget)?.remove(0);
    let original_region = fixture.region.clone();
    assert_eq!(fixture.evaluation().region, &original_region);
    assert_eq!(
        fixture
            .evaluation()
            .root_relations(&target.numerator, &mut budget)?
            .len(),
        1
    );
    let changed = Evaluation {
        region: &sibling,
        ..fixture.evaluation()
    };
    assert!(
        changed
            .root_relations(&target.numerator, &mut budget)
            .is_err()
    );
    Ok(())
}

fn expression(
    fixture: &mut Fixture,
    operator: crate::model::Operator,
    operands: &[Handle],
) -> TestResult<Handle> {
    let root = fixture
        .cas
        .state
        .expression(fixture.roots[0].logical_id())?;
    let draft = crate::testing::solve_graph::expression(
        Reference::Existing(fixture.cas.handle(root.header)),
        operator,
        root.sort.clone(),
        operands.iter().cloned().map(Reference::Existing).collect(),
    );
    crate::testing::solve_graph::admit(&mut fixture.cas, &draft)
}

/// Even a zero established by transitive root closure retains the original divisor's strict domain.
/// ´test:crate:root-closure-strict-denominator-mutation´
#[test]
fn root_closure_strict_denominator_mutation() -> TestResult {
    use crate::model::Operator;
    let mut fixture = fixture()?;
    let roots = fixture.roots.clone();
    let left = expression(
        &mut fixture,
        Operator::IntegerPower(3),
        std::slice::from_ref(&roots[1]),
    )?;
    let right = expression(
        &mut fixture,
        Operator::IntegerPower(3),
        std::slice::from_ref(&roots[2]),
    )?;
    let product = expression(&mut fixture, Operator::Mul, &[left, right])?;
    let rational = Rational::from_ratio(1, 27, &mut Budget::new(Limits::blind_cubic())?)?;
    let offset = expression(&mut fixture, Operator::Rational(rational), &[])?;
    let numerator = expression(&mut fixture, Operator::Add, &[product, offset])?;
    let unit = expression(&mut fixture, Operator::Rational(Rational::one()), &[])?;
    let zero = expression(&mut fixture, Operator::Rational(Rational::zero()), &[])?;
    assert_ne!(unit, zero);
    let root = fixture.cas.state.expression(numerator.logical_id())?;
    let Draft::Expression(mut quotient) = crate::testing::solve_graph::expression(
        Reference::Existing(fixture.cas.handle(root.header)),
        Operator::Div,
        root.sort.clone(),
        vec![
            Reference::Existing(numerator),
            Reference::Existing(unit.clone()),
        ],
    ) else {
        return Err("quotient draft".into());
    };
    let original =
        crate::testing::solve_graph::admit(&mut fixture.cas, &Draft::Expression(quotient.clone()))?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let evaluation = fixture.evaluation();
    let value = evaluation.expression(original.logical_id(), true, &mut budget)?;
    evaluation.zero(&value, &mut budget)?;
    assert_eq!(quotient.operands[1], Reference::Existing(unit));
    let replacement = Reference::Existing(zero);
    assert_ne!(quotient.operands[1], replacement);
    quotient.operands[1] = replacement;
    let changed =
        crate::testing::solve_graph::admit(&mut fixture.cas, &Draft::Expression(quotient))?;
    assert!(
        fixture
            .evaluation()
            .expression(changed.logical_id(), true, &mut budget)
            .is_err()
    );
    Ok(())
}
