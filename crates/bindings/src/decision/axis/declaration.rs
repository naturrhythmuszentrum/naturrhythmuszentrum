// SPDX-License-Identifier: AGPL-3.0-only
//! Original physical correspondence and every continuation are checked before numerical access.

use super::{
    AxisPhaseCase, AxisPhaseSpec, Data, DecisionError, DecisionExpression, storage, syntax::Builder,
};
use crate::{BindingSymbols, Budget, Descriptor, Kind, Measure, Role};
use nrz_cas::{
    Cas, ExactConstant,
    proof::{ApplicationDraft, Certificate, Primitive, Statement},
    syntax::{Operator, PredicateKind},
};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

impl AxisPhaseCase {
    /// Declare a closed original normalized reactance, all sign regions and both consumers.
    ///
    /// The physical source must be a checked ratio with a rational exact target. The
    /// declared source context supplies its explicit coordinate-to-target hypothesis.
    /// This initial adapter does not compose uncertain L/C into a new reactance source.
    /// # Errors
    /// Refuses open physical sources, changed targets, non-Real/free coordinates,
    /// unsupported domains, wrong correspondence slots and cumulative resource limits.
    pub fn declare(
        graph: Arc<Mutex<Cas>>,
        source: DecisionExpression,
        spec: AxisPhaseSpec,
        budget: &mut Budget,
    ) -> Result<Self, DecisionError> {
        let mut cas = graph.lock().map_err(|_| DecisionError::GraphPoisoned)?;
        check(&cas, &source, &spec, budget)?;
        let mut builder = Builder::new(&mut cas, &spec, budget)?;
        let predicate = builder.predicate(
            PredicateKind::Equal,
            &[spec.coordinate.clone(), spec.target.clone()],
            budget,
        )?;
        let correspondence = ApplicationDraft {
            context: spec.context.clone(),
            rule: Primitive::ContextHypothesis,
            conclusion: Statement::PredicateFact {
                region: builder.cas.admissible_region(&spec.context, budget)?,
                predicate,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Hypothesis(spec.correspondence.clone()),
        };
        let witness = builder.cas.check_proof(
            &spec.context,
            std::slice::from_ref(&correspondence),
            budget,
        )?;
        let (partition, branches) = builder.partition(&spec.coordinate, budget)?;
        builder.cas.check_case_partition(&partition, budget)?;
        let (phases, reciprocal) = builder.continuations(&spec.coordinate, budget)?;
        drop(cas);
        let angle = Descriptor::new(Kind::Angle, Measure::coherent(), Role::Difference)?;
        let symbols = BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [true, true], budget)?;
        let bytes = symbols
            .value()
            .cas()?
            .counts()
            .payload_bytes
            .checked_add(storage::witness_bytes(&witness)?)
            .and_then(|bytes| bytes.checked_add(4096))
            .ok_or(DecisionError::Capacity)?;
        storage::retained(bytes, budget)?;
        Ok(Self(Rc::new(Data {
            graph,
            source,
            spec,
            partition,
            branches,
            phases,
            reciprocal,
            correspondence,
            angle,
            symbols,
            bytes,
        })))
    }
}

/// Same exact value is necessary, but never replaces the retained physical source identity.
fn check(
    cas: &Cas,
    source: &DecisionExpression,
    spec: &AxisPhaseSpec,
    budget: &mut Budget,
) -> Result<(), DecisionError> {
    let expected = source.exact().ok_or(DecisionError::Source)?;
    if expected.as_rational().is_none() {
        return Err(DecisionError::Source);
    }
    let actual = ExactConstant::from_expression(cas, &spec.target, budget)?;
    budget.work(
        storage::constant_bytes(&actual).max(storage::constant_bytes(expected)),
        storage::AT,
    )?;
    if actual != *expected {
        return Err(DecisionError::Target);
    }
    let header = cas.context_header(&spec.context, budget)?;
    for term in [&spec.coordinate, &spec.target] {
        let syntax = cas.describe_syntax(term, budget)?;
        let (_, sort) = cas.expression(term, budget)?;
        if syntax.header != header
            || !syntax.scope.is_empty()
            || syntax.binding.is_some()
            || sort != &super::syntax::real()
        {
            return Err(DecisionError::Case);
        }
    }
    if !matches!(
        cas.expression(&spec.coordinate, budget)?.0,
        Operator::Free(_)
    ) {
        return Err(DecisionError::Case);
    }
    Ok(())
}
