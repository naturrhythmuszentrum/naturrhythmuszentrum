// SPDX-License-Identifier: AGPL-3.0-only
//! Exact formal-root context with complete ordered Vieta hypotheses and independent existence.

use super::construction::{Construction, admit};
use super::formal_render::Rendering;
use crate::budget::Budget;
use crate::domain_model::{ContextDraft, HypothesisRef};
use crate::error::Fault;
use crate::graph::{AssumptionDraft, Cas, Draft, Handle, Reference};
use crate::model::{AtomicSort, Field, Operator, PredicateKind};
use crate::proof_model::{ApplicationDraft, Certificate, Statement};
use crate::rule::Primitive;
use crate::solving::coefficients::storage::{copied, invalid, reserve, vector};
use crate::solving::symmetric::{algebra::Polynomial, reduction::Reduction};
use crate::solving::towers::{dense::Dense, materialize};
use crate::solving::{
    AdjunctionMode, CertificateCoefficientIdentity, CertificateRootAdjunction, IdentityMode,
    RelationMultiple, RootRelationData, ScopedCoefficientRelation, SolveCertificate, SolveDraft,
    SolveSchema, SolveStatement, StatementCoefficientEquality, StatementRootExistence,
};

/// Bound roots, displayed hypotheses and their independent source anchor.
pub(crate) struct Formal {
    /// Scoped rendering retains the exact lifted scalar map.
    pub(crate) rendering: Rendering,
    /// Complete ordered innermost root frame.
    pub(crate) roots: Vec<Handle>,
    /// Exact polynomial endpoints of each scalar equality.
    endpoints: Vec<(Handle, Handle)>,
    /// Ordered ideal generators, with unchanged polynomial-slot semantics.
    relations: Vec<Handle>,
    /// Actual local scalar equality predicates.
    predicates: Vec<Handle>,
    /// Scoped region until introduction installs the root relation.
    anchor: Handle,
}

impl Formal {
    /// The local region is structurally the original region with one unused complex frame.
    pub(crate) fn new(
        cas: &mut Cas,
        construction: &Construction,
        reduction: &Reduction,
        images: &[Polynomial],
        input_map: &Handle,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let scope = vec![vec![
            AtomicSort::Scalar(Field::Complex);
            reduction.basis.len()
        ]];
        let region = super::formal_render::lift(cas, &construction.region, &scope, budget)?;
        let map = super::formal_render::mapping(cas, input_map, &region, &scope, budget)?;
        let rendering = Rendering {
            header: construction.header.clone(),
            map,
            scope,
        };
        let mut roots = vector(reduction.basis.len(), budget)?;
        for index in 0..reduction.basis.len() {
            roots.push(rendering.operation(
                cas,
                Operator::Bound {
                    depth: 0,
                    slot: u32::try_from(index).map_err(|_| invalid())?,
                },
                &[],
                budget,
            )?);
        }
        let mut result = Self {
            rendering,
            roots,
            endpoints: Vec::new(),
            relations: Vec::new(),
            predicates: Vec::new(),
            anchor: region,
        };
        for (basis, value) in reduction.basis.iter().zip(images) {
            result.equation(cas, basis, value, budget)?;
        }
        Ok(result)
    }

    /// A predicate and its polynomial endpoints are built independently in distinct namespaces.
    fn equation(
        &mut self,
        cas: &mut Cas,
        basis: &Polynomial,
        value: &Polynomial,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let left = self.rendering.polynomial(cas, basis, &self.roots, budget)?;
        let right = self.rendering.polynomial(cas, value, &self.roots, budget)?;
        let predicate = admit(
            cas,
            &Draft::Condition {
                header: Reference::Existing(self.rendering.header.clone()),
                scope: self.rendering.scope.clone(),
                kind: PredicateKind::Equal,
                arguments: vec![Reference::Existing(left), Reference::Existing(right)],
            },
            budget,
        )?;
        let endpoints = (basis.stage(cas, budget)?, value.stage(cas, budget)?);
        let relation = basis
            .add(&value.neg(&cas.state, budget)?, &cas.state, budget)?
            .stage(cas, budget)?;
        reserve(&mut self.endpoints, 1, budget)?;
        reserve(&mut self.relations, 1, budget)?;
        reserve(&mut self.predicates, 1, budget)?;
        self.endpoints.push(endpoints);
        self.relations.push(relation);
        self.predicates.push(predicate);
        Ok(())
    }

    /// Each derivation gets a distinct local frame so its ordered premises remain exact.
    pub(crate) fn introduce(
        &mut self,
        cas: &mut Cas,
        construction: &mut Construction,
        source: &Dense,
        reciprocal: &crate::solving::coefficients::model::Element,
        budget: &mut Budget,
    ) -> Result<(), crate::solving::coefficients::model::CoefficientFailure> {
        let mut hypotheses = vector(self.predicates.len(), budget)?;
        for (index, predicate) in self.predicates.iter().enumerate() {
            hypotheses.push(AssumptionDraft {
                key: format!("vieta-{}-{index}", construction.steps.len()),
                condition: Reference::Existing(predicate.clone()),
            });
        }
        construction.context = admit(
            cas,
            &Draft::Context(ContextDraft::Local {
                parent: Reference::Existing(construction.context.clone()),
                region: Reference::Existing(self.anchor.clone()),
                hypotheses,
            }),
            budget,
        )?;
        construction.region.clone_from(&self.anchor);
        let polynomial = materialize::polynomial(cas, source, &[Field::Complex], budget)?;
        self.anchor = materialize::admit(
            cas,
            SolveDraft::RootRelation(RootRelationData {
                header: Reference::Existing(construction.header.clone()),
                scope: self.rendering.scope.clone(),
                ambient: Field::Complex,
                polynomial: Reference::Existing(polynomial),
                binder: vec![AtomicSort::Scalar(Field::Complex)],
                map: Reference::Existing(self.rendering.map.clone()),
                region: Reference::Existing(construction.region.clone()),
                multiplicity: None,
            }),
            budget,
        )?;
        let unit = super::formal_base::leading(cas, construction, source, reciprocal, budget)?;
        construction.push(
            SolveSchema::RootAdjunction,
            SolveStatement::RootExistence(StatementRootExistence {
                relation: self.anchor.clone(),
                region: construction.region.clone(),
            }),
            SolveCertificate::RootAdjunction(CertificateRootAdjunction {
                mode: AdjunctionMode::ComplexFTA,
                relation: self.anchor.clone(),
                selection: None,
                leading_unit: unit,
                order: None,
                splitting_chain: Vec::new(),
            }),
            budget,
        )?;
        Ok(self.hypotheses(construction, budget)?)
    }

    /// Hypothesis applications carry no premises, so the existence proof cannot inherit them.
    fn hypotheses(
        &self,
        construction: &mut Construction,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        for (index, predicate) in self.predicates.iter().enumerate() {
            reserve(&mut construction.steps, 1, budget)?;
            construction.steps.push(ApplicationDraft {
                rule: Primitive::ContextHypothesis,
                context: construction.context.clone(),
                conclusion: Statement::PredicateFact {
                    predicate: predicate.clone(),
                    region: construction.region.clone(),
                },
                premises: Vec::new(),
                substitutions: Vec::new(),
                certificate: Certificate::Hypothesis(HypothesisRef {
                    context: construction.context.clone(),
                    slot: u32::try_from(index + 1).map_err(|_| invalid())?,
                }),
            });
        }
        Ok(())
    }

    /// Every anchored row cites the same source and all elementary equations exactly once.
    pub(crate) fn identity(
        &self,
        cas: &Cas,
        construction: &mut Construction,
        endpoints: [&Handle; 2],
        multipliers: &[Handle],
        anchored: bool,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        if multipliers.len() != self.relations.len() {
            return Err(invalid());
        }
        let mut combinations = vector(multipliers.len(), budget)?;
        for (index, multiplier) in multipliers.iter().enumerate() {
            let (left, right) = self.endpoints.get(index).ok_or_else(invalid)?;
            combinations.push(RelationMultiple {
                multiplier: multiplier.clone(),
                relation: self.relations.get(index).ok_or_else(invalid)?.clone(),
                hypothesis: Some(ScopedCoefficientRelation {
                    premise: u32::try_from(index + 2).map_err(|_| invalid())?,
                    left: left.clone(),
                    right: right.clone(),
                    roots: copied(&self.roots, budget)?,
                    vieta: anchored.then(|| self.anchor.clone()),
                }),
            });
        }
        let SolveDraft::CoefficientMap(map) =
            crate::solving::graph::object(&cas.state, cas.checked(&self.rendering.map)?)?
        else {
            return Err(invalid());
        };
        let denominators = copied(&map.denominators, budget)?;
        let [left, right] = endpoints;
        construction.push(
            SolveSchema::CoefficientIdentity,
            SolveStatement::CoefficientEquality(StatementCoefficientEquality {
                left: left.clone(),
                right: right.clone(),
                map: self.rendering.map.clone(),
                region: construction.region.clone(),
            }),
            SolveCertificate::CoefficientIdentity(CertificateCoefficientIdentity {
                mode: IdentityMode::RelationCombination,
                left: left.clone(),
                right: right.clone(),
                map: self.rendering.map.clone(),
                region: construction.region.clone(),
                combinations,
                denominators,
            }),
            budget,
        )?;
        Ok(())
    }
}
