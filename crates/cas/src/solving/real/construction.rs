// SPDX-License-Identifier: AGPL-3.0-only
//! Real scalar construction over the shared unsealed application DAG.

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference};
use crate::model::{AtomicSort, Field, Operator, PredicateKind, Sort};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::Element;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{invalid, vector};
use crate::solving::towers::{arithmetic, dense::Dense, materialize};
use crate::solving::{
    AdjunctionMode, CertificateExtensionUnit, CertificateRootAdjunction, CoefficientDomainData,
    CoefficientMapData, ExtensionLevelData, MapMode, MapTarget, RootRelationData,
    RootSelectionData, SelectionMode, SolveCertificate, SolveDraft, SolveSchema, SolveStatement,
    SquareFreeRequest, StatementRootExistence, StatementUnit,
};

use crate::solving::radicals::construction::{Construction, Extension};

/// Preserve the shared proof scheduler while every constructed scalar remains Real.
pub(crate) struct Frame {
    /// Unsealed applications and the exact original semantic frame.
    pub(crate) inner: Construction,
}

impl std::ops::Deref for Frame {
    type Target = Construction;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for Frame {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Admit one syntax object inside the caller's transaction.
pub(crate) fn admit(cas: &mut Cas, draft: &Draft, budget: &mut Budget) -> Result<Handle, Fault> {
    cas.admit(std::slice::from_ref(draft), budget)?
        .into_iter()
        .next()
        .ok_or_else(invalid)
}

impl Frame {
    /// Ordinary scalar arithmetic remains visible to the independent checker.
    pub(crate) fn expression(
        &self,
        cas: &mut Cas,
        operator: Operator,
        operands: &[Handle],
        bound: bool,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let mut arguments = vector(operands.len(), budget)?;
        arguments.extend(operands.iter().cloned().map(Reference::Existing));
        let scope = if bound {
            vec![vec![AtomicSort::Scalar(Field::Real)]]
        } else {
            Vec::new()
        };
        admit(
            cas,
            &Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.header.clone()),
                operator,
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                operands: arguments,
                scope,
                space: None,
                binding: None,
            }),
            budget,
        )
    }

    /// A scalar map retains the exact algebra and region; quotient images are explicit root values.
    pub(crate) fn mapping(
        &self,
        cas: &mut Cas,
        domain: &Handle,
        images: &[Handle],
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let mut values = vector(images.len(), budget)?;
        values.extend(images.iter().cloned().map(Reference::Existing));

        materialize::admit(
            cas,
            SolveDraft::CoefficientMap(CoefficientMapData {
                source: Reference::Existing(domain.clone()),
                target: MapTarget::ScalarAmbient {
                    header: Reference::Existing(self.header.clone()),
                    field: Field::Real,
                },
                mode: MapMode::Specialization,
                region: Reference::Existing(self.region.clone()),
                images: values,
                denominators: Vec::new(),
            }),
            budget,
        )
    }

    /// Exact coefficients retain their finite quotient and selected scalar map.
    pub(crate) fn literal(
        &self,
        cas: &mut Cas,
        value: &Element,
        map: &Handle,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let value = materialize::value(cas, value, budget)?;
        admit(
            cas,
            &Draft::ExactCoefficient {
                header: Reference::Existing(self.header.clone()),
                value: Reference::Existing(value),
                map: Reference::Existing(map.clone()),
            },
            budget,
        )
    }

    /// Root syntax has an explicit one-slot tautology; existence is supplied separately.
    fn selection(
        &self,
        cas: &mut Cas,
        polynomial: &Handle,
        map: &Handle,
        positive: bool,
        budget: &mut Budget,
    ) -> Result<(Handle, Handle, Handle), Fault> {
        let relation = materialize::admit(
            cas,
            SolveDraft::RootRelation(RootRelationData {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                ambient: Field::Real,
                polynomial: Reference::Existing(polynomial.clone()),
                binder: vec![AtomicSort::Scalar(Field::Real)],
                map: Reference::Existing(map.clone()),
                region: Reference::Existing(self.region.clone()),
                multiplicity: None,
            }),
            budget,
        )?;
        let bound = self.expression(
            cas,
            Operator::Bound { depth: 0, slot: 0 },
            &[],
            true,
            budget,
        )?;
        let predicate = admit(
            cas,
            &Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: vec![vec![AtomicSort::Scalar(Field::Real)]],
                kind: if positive {
                    PredicateKind::Positive
                } else {
                    PredicateKind::Equal
                },
                arguments: if positive {
                    vec![Reference::Existing(bound)]
                } else {
                    vec![
                        Reference::Existing(bound.clone()),
                        Reference::Existing(bound),
                    ]
                },
            },
            budget,
        )?;
        let selection = materialize::admit(
            cas,
            SolveDraft::RootSelection(RootSelectionData {
                cell: None,
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                relations: vec![Reference::Existing(relation.clone())],
                predicate: Reference::Existing(predicate),
                mode: if positive {
                    SelectionMode::Constrained
                } else {
                    SelectionMode::Any
                },
                convention: None,
                interval: None,
            }),
            budget,
        )?;
        let root = admit(
            cas,
            &Draft::RootValue {
                header: Reference::Existing(self.header.clone()),
                selection: Reference::Existing(selection.clone()),
                slot: 0,
            },
            budget,
        )?;
        Ok((relation, selection, root))
    }

    /// A monic quotient is syntax only until its selected map has an existence theorem.
    fn quotient(
        cas: &mut Cas,
        parent: &Handle,
        polynomial: &Handle,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let level = materialize::admit(
            cas,
            SolveDraft::ExtensionLevel(ExtensionLevelData {
                parent: Reference::Existing(parent.clone()),
                polynomial: Reference::Existing(polynomial.clone()),
                generator: vec![AtomicSort::Scalar(Field::Real)],
                square_free: SquareFreeRequest::NotRequested,
            }),
            budget,
        )?;
        materialize::admit(
            cas,
            SolveDraft::CoefficientDomain(CoefficientDomainData::Quotient {
                level: Reference::Existing(level),
            }),
            budget,
        )
    }

    /// The one-unit theorem reconstructs the complete Bezout identity in the actual parent algebra.
    pub(crate) fn unit(
        &mut self,
        cas: &mut Cas,
        domain: &Handle,
        budget: &mut Budget,
    ) -> Result<u32, Fault> {
        let engine = Engine {
            state: &cas.state,
            domain,
        };
        let one = engine.constant(true, budget).map_err(|e| fault(&e))?;
        let value = materialize::value(cas, &one, budget)?;
        let engine = Engine {
            state: &cas.state,
            domain,
        };
        let (parent, modulus) =
            match crate::solving::coefficients::storage::domain(&cas.state, domain)? {
                CoefficientDomainData::Quotient { .. } => {
                    let (_, level) = arithmetic::level(&engine).map_err(|e| fault(&e))?;
                    (level.parent.clone(), Some(level.polynomial.clone()))
                }
                _ => (domain.clone(), None),
            };
        let engine = Engine {
            state: &cas.state,
            domain: &parent,
        };
        let constant = Dense::one(&engine, budget).map_err(|e| fault(&e))?;
        let empty = Dense::zero(&engine, budget).map_err(|e| fault(&e))?;
        let modulus = if let Some(modulus) = modulus {
            modulus
        } else {
            let variable = constant
                .scaled(
                    &engine.constant(true, budget).map_err(|e| fault(&e))?,
                    1,
                    &cas.state,
                    budget,
                )
                .map_err(|e| fault(&e))?;
            materialize::polynomial(cas, &variable, &[Field::Real], budget)?
        };
        let constant = materialize::polynomial(cas, &constant, &[Field::Real], budget)?;
        let empty = materialize::polynomial(cas, &empty, &[Field::Real], budget)?;
        let region = self.region.clone();
        self.push(
            SolveSchema::ExtensionUnit,
            SolveStatement::Unit(StatementUnit {
                value: value.clone(),
                inverse: value.clone(),
                domain: domain.clone(),
                region: region.clone(),
            }),
            SolveCertificate::ExtensionUnit(CertificateExtensionUnit {
                element: value,
                modulus: modulus.clone(),
                gcd: constant.clone(),
                quotients: vec![constant.clone(), modulus],
                bezout: vec![constant, empty],
                region,
            }),
            budget,
        )
    }

    /// A named Real existence theorem checks the exact binomial and its selection predicate.
    fn root_choice(
        &mut self,
        cas: &mut Cas,
        polynomial: &Dense,
        interpretation: [&Handle; 2],
        order: Option<u32>,
        budget: &mut Budget,
    ) -> Result<(Handle, Handle), Fault> {
        let (relation, selection, root) = self.selection(
            cas,
            interpretation[0],
            interpretation[1],
            order == Some(2),
            budget,
        )?;
        let unit = self.unit(cas, &polynomial.domain, budget)?;
        let region = self.region.clone();
        self.push(
            SolveSchema::RootAdjunction,
            SolveStatement::RootExistence(StatementRootExistence {
                relation: selection.clone(),
                region,
            }),
            SolveCertificate::RootAdjunction(CertificateRootAdjunction {
                mode: if order == Some(2) {
                    AdjunctionMode::PositiveRealSquareRoot
                } else {
                    AdjunctionMode::RealOddRoot
                },
                relation,
                selection: Some(selection.clone()),
                leading_unit: unit,
                order,
                splitting_chain: Vec::new(),
            }),
            budget,
        )?;
        Ok((selection, root))
    }

    /// Select an odd root without constructing an unnecessary algebraic compositum.
    pub(crate) fn named(
        &mut self,
        cas: &mut Cas,
        polynomial: &Dense,
        map: &Handle,
        order: u32,
        budget: &mut Budget,
    ) -> Result<(Handle, Handle), Fault> {
        let stored = materialize::polynomial(cas, polynomial, &[Field::Real], budget)?;
        self.root_choice(cas, polynomial, [&stored, map], Some(order), budget)
    }

    /// Adjoin a root of a computed monic polynomial and retain its exact generator evaluation.
    pub(crate) fn adjoin(
        &mut self,
        cas: &mut Cas,
        polynomial: &Dense,
        map: &Handle,
        order: Option<u32>,
        budget: &mut Budget,
    ) -> Result<Extension, Fault> {
        let stored = materialize::polynomial(cas, polynomial, &[Field::Real], budget)?;
        let (selection, root) = self.root_choice(cas, polynomial, [&stored, map], order, budget)?;
        let domain = Self::quotient(cas, &polynomial.domain, &stored, budget)?;
        let engine = Engine {
            state: &cas.state,
            domain: &polynomial.domain,
        };
        let mut coefficients = vector(2, budget)?;
        coefficients.push(engine.constant(false, budget).map_err(|e| fault(&e))?);
        coefficients.push(engine.constant(true, budget).map_err(|e| fault(&e))?);
        let variable = Dense::new(&engine, coefficients, budget).map_err(|e| fault(&e))?;
        let engine = Engine {
            state: &cas.state,
            domain: &domain,
        };
        let generator = arithmetic::reduced(&engine, variable, budget).map_err(|e| fault(&e))?;
        let map = self.mapping(cas, &domain, std::slice::from_ref(&root), budget)?;
        Ok(Extension {
            domain,
            map,
            generator,
            root,
            selection,
        })
    }
}
