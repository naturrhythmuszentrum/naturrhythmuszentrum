// SPDX-License-Identifier: AGPL-3.0-only
//! Graph-owned radical construction with unsealed, explicitly ordered proof applications.

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference};
use crate::model::{AtomicSort, Field, Operator, PredicateKind, Sort};
use crate::proof_model::{ApplicationDraft, Certificate, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::Element;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{invalid, reserve, vector};
use crate::solving::towers::{arithmetic, dense::Dense, materialize};
use crate::solving::{
    AdjunctionMode, CertificateExtensionUnit, CertificateRootAdjunction, CoefficientDomainData,
    CoefficientMapData, ExtensionLevelData, MapMode, MapTarget, RootRelationData,
    RootSelectionData, SelectionMode, SolveCertificate, SolveDraft, SolveSchema, SolveStatement,
    SquareFreeRequest, StatementRootExistence, StatementUnit,
};

/// A selected generator and its explicit scalar interpretation, without any implicit field claim.
#[derive(Debug)]
pub(crate) struct Extension {
    /// Exact coefficient algebra shared by the returned values.
    pub(crate) domain: Handle,
    /// Explicit scalar interpretation with its selected-root dependencies.
    pub(crate) map: Handle,
    /// Canonical residue of the defining generator.
    pub(crate) generator: Element,
    /// Scalar root value whose exact selection has an existence application.
    pub(crate) root: Handle,
    /// Graph-owned root selection with an explicit binder.
    pub(crate) selection: Handle,
}

/// One exact coefficient context and a bounded proposal DAG awaiting independent replay.
#[derive(Debug)]
pub(crate) struct Construction {
    /// Original scalar declaration header.
    pub(crate) header: Handle,
    /// Complete hypothesis context of every application.
    pub(crate) context: Handle,
    /// Exact coefficient region of every application.
    pub(crate) region: Handle,
    /// Ordered unsealed proof proposals, independently replayed before publication.
    pub(crate) steps: Vec<ApplicationDraft>,
}

/// Admit one syntax object inside the caller's transaction.
pub(crate) fn admit(cas: &mut Cas, draft: &Draft, budget: &mut Budget) -> Result<Handle, Fault> {
    cas.admit(std::slice::from_ref(draft), budget)?
        .into_iter()
        .next()
        .ok_or_else(invalid)
}

impl Construction {
    /// Earlier applications in this exact context are explicit premises; sibling frames never leak.
    pub(crate) fn push(
        &mut self,
        rule: SolveSchema,
        conclusion: SolveStatement,
        certificate: SolveCertificate,
        budget: &mut Budget,
    ) -> Result<u32, Fault> {
        self.append(
            crate::rule::Primitive::Solve(rule),
            Statement::Solve(Box::new(conclusion)),
            Certificate::Solve(Box::new(certificate)),
            budget,
        )
    }

    /// A shared scoped step builder also supports ordinary cover judgments.
    pub(crate) fn append(
        &mut self,
        rule: crate::rule::Primitive,
        conclusion: Statement,
        certificate: Certificate,
        budget: &mut Budget,
    ) -> Result<u32, Fault> {
        let mut premises = vector(self.steps.len(), budget)?;
        for (index, step) in self.steps.iter().enumerate() {
            budget.work(1, super::AT)?;
            if step.context == self.context {
                premises.push(u32::try_from(index).map_err(|_| invalid())?);
            }
        }
        let index = u32::try_from(premises.len()).map_err(|_| invalid())?;
        reserve(&mut self.steps, 1, budget)?;
        self.steps.push(ApplicationDraft {
            rule,
            context: self.context.clone(),
            conclusion,
            premises,
            substitutions: Vec::new(),
            certificate,
        });
        Ok(index)
    }

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
            vec![vec![AtomicSort::Scalar(Field::Complex)]]
        } else {
            Vec::new()
        };
        admit(
            cas,
            &Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.header.clone()),
                operator,
                sort: Sort::Value(AtomicSort::Scalar(Field::Complex)),
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
                    field: Field::Complex,
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
        budget: &mut Budget,
    ) -> Result<(Handle, Handle, Handle), Fault> {
        let relation = materialize::admit(
            cas,
            SolveDraft::RootRelation(RootRelationData {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                ambient: Field::Complex,
                polynomial: Reference::Existing(polynomial.clone()),
                binder: vec![AtomicSort::Scalar(Field::Complex)],
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
                scope: vec![vec![AtomicSort::Scalar(Field::Complex)]],
                kind: PredicateKind::Equal,
                arguments: vec![
                    Reference::Existing(bound.clone()),
                    Reference::Existing(bound),
                ],
            },
            budget,
        )?;
        let selection = materialize::admit(
            cas,
            SolveDraft::RootSelection(RootSelectionData {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                relations: vec![Reference::Existing(relation.clone())],
                predicate: Reference::Existing(predicate),
                mode: SelectionMode::Any,
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
                generator: vec![AtomicSort::Scalar(Field::Complex)],
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
            materialize::polynomial(cas, &variable, &[Field::Complex], budget)?
        };
        let constant = materialize::polynomial(cas, &constant, &[Field::Complex], budget)?;
        let empty = materialize::polynomial(cas, &empty, &[Field::Complex], budget)?;
        self.push(
            SolveSchema::ExtensionUnit,
            SolveStatement::Unit(StatementUnit {
                value: value.clone(),
                inverse: value.clone(),
                domain: domain.clone(),
                region: self.region.clone(),
            }),
            SolveCertificate::ExtensionUnit(CertificateExtensionUnit {
                element: value,
                modulus: modulus.clone(),
                gcd: constant.clone(),
                quotients: vec![constant.clone(), modulus],
                bezout: vec![constant, empty],
                region: self.region.clone(),
            }),
            budget,
        )
    }

    /// Constructive real witnesses precede the generic complex existence fallback.
    fn root_choice(
        &mut self,
        cas: &mut Cas,
        polynomial: &Dense,
        interpretation: [&Handle; 2],
        order: Option<u32>,
        budget: &mut Budget,
    ) -> Result<(Handle, Handle), Fault> {
        let (relation, selection, root) =
            self.selection(cas, interpretation[0], interpretation[1], budget)?;
        if order.is_none()
            && let Some(real) =
                super::real_choice::choose(cas, self, polynomial, &relation, budget)?
        {
            return Ok(real);
        }
        let unit = self.unit(cas, &polynomial.domain, budget)?;
        self.push(
            SolveSchema::RootAdjunction,
            SolveStatement::RootExistence(StatementRootExistence {
                relation: selection.clone(),
                region: self.region.clone(),
            }),
            SolveCertificate::RootAdjunction(CertificateRootAdjunction {
                mode: if order.is_some() {
                    AdjunctionMode::PrimitiveUnity
                } else {
                    AdjunctionMode::ComplexFTA
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

    /// Adjoin a root of a computed monic polynomial and retain its exact generator evaluation.
    pub(crate) fn adjoin(
        &mut self,
        cas: &mut Cas,
        polynomial: &Dense,
        map: &Handle,
        order: Option<u32>,
        budget: &mut Budget,
    ) -> Result<Extension, Fault> {
        let stored = materialize::polynomial(cas, polynomial, &[Field::Complex], budget)?;
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
