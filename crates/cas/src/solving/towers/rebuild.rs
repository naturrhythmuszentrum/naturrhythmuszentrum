// SPDX-License-Identifier: AGPL-3.0-only
//! Parent-first restriction of dependent coefficient objects into a staged child algebra.

use super::dense::Dense;
use super::materialize;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle, Reference};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::{Element, Value};
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{AT, copied, domain, invalid, reserve, vector};
use crate::solving::{
    CoefficientDomainData, ExtensionLevelData, PolynomialData, PolynomialTerm, SolveDraft,
};

/// One component's shared substitutions, including every rebuilt ancestor before its children.
#[derive(Debug)]
pub(crate) struct Rebuilder {
    /// Shared source/image identities accumulated in parent-first order.
    replacements: Vec<(Handle, Handle)>,
}

impl Rebuilder {
    /// Seed the proper component projection without changing unrelated graph identities.
    pub(crate) fn new(
        source: &Handle,
        target: &Handle,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let mut replacements = vector(1, budget)?;
        replacements.push((source.clone(), target.clone()));
        Ok(Self { replacements })
    }

    /// Cache lookup charges all compared graph identities and never depends on display names.
    fn existing(&self, source: &Handle, budget: &mut Budget) -> Result<Option<Handle>, Fault> {
        for (before, after) in &self.replacements {
            budget.work(1, AT)?;
            if before == source {
                return Ok(Some(after.clone()));
            }
        }
        Ok(None)
    }

    /// Rebuild a defining relation only after all of its parent coefficients have been restricted.
    fn domain(
        &mut self,
        cas: &mut Cas,
        source: &Handle,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        if let Some(target) = self.existing(source, budget)? {
            return Ok(target);
        }
        let CoefficientDomainData::Quotient { level } = domain(&cas.state, source)? else {
            return Ok(source.clone());
        };
        let SolveDraft::ExtensionLevel(extension) =
            crate::solving::graph::object(&cas.state, level.logical_id())?
        else {
            return Err(invalid());
        };
        let original_parent = extension.parent.clone();
        let defining = extension.polynomial.clone();
        let generator = materialize::generator(&extension.generator, budget)?;
        let square_free = extension.square_free;
        let parent = self.domain(cas, &original_parent, budget)?;
        if parent == original_parent {
            return Ok(source.clone());
        }
        let polynomial = self.polynomial(cas, &defining, &parent, budget)?;
        let level = materialize::admit(
            cas,
            SolveDraft::ExtensionLevel(ExtensionLevelData {
                parent: Reference::Existing(parent),
                polynomial: Reference::Existing(polynomial),
                generator,
                square_free,
            }),
            budget,
        )?;
        let target = materialize::admit(
            cas,
            SolveDraft::CoefficientDomain(CoefficientDomainData::Quotient {
                level: Reference::Existing(level),
            }),
            budget,
        )?;
        reserve(&mut self.replacements, 1, budget)?;
        self.replacements.push((source.clone(), target.clone()));
        Ok(target)
    }

    /// Canonical generator projections recursively reduce every parent entry before this child.
    fn element(
        cas: &Cas,
        element: &Element,
        target: &Handle,
        budget: &mut Budget,
    ) -> Result<Element, Fault> {
        let engine = Engine {
            state: &cas.state,
            domain: target,
        };
        if element.domain == *target {
            return engine.copy(element, budget).map_err(|error| fault(&error));
        }
        let Value::Tower(entries) = &element.value else {
            return Err(invalid());
        };
        let (_, extension) = super::arithmetic::level(&engine).map_err(|error| fault(&error))?;
        let parent = Engine {
            state: &cas.state,
            domain: &extension.parent,
        };
        let mut coefficients = vector(entries.len(), budget)?;
        for entry in entries {
            coefficients.push(Self::element(cas, entry, &extension.parent, budget)?);
        }
        let polynomial =
            Dense::new(&parent, coefficients, budget).map_err(|error| fault(&error))?;
        super::arithmetic::reduced(&engine, polynomial, budget).map_err(|error| fault(&error))
    }

    /// Reuse unchanged coefficient nodes and admit only reduced images in changed domains.
    fn coefficient(
        &mut self,
        cas: &mut Cas,
        source: &Handle,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        if let Some(target) = self.existing(source, budget)? {
            return Ok(target);
        }
        let original = crate::solving::coefficients::storage::value(&cas.state, source)?
            .domain
            .clone();
        let target = self.domain(cas, &original, budget)?;
        if target == original {
            return Ok(source.clone());
        }
        let engine = Engine {
            state: &cas.state,
            domain: &original,
        };
        let element = engine.read(source, budget).map_err(|error| fault(&error))?;
        let reduced = Self::element(cas, &element, &target, budget)?;
        let result = materialize::value(cas, &reduced, budget)?;
        reserve(&mut self.replacements, 1, budget)?;
        self.replacements.push((source.clone(), result.clone()));
        Ok(result)
    }

    /// Preserve a polynomial's exact formal variable frame while discarding newly zero coefficients.
    fn polynomial(
        &mut self,
        cas: &mut Cas,
        source: &Handle,
        target: &Handle,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let stored = crate::solving::coefficients::storage::polynomial(&cas.state, source)?;
        let variables = materialize::fields(&stored.variables, budget)?;
        let mut inputs = vector(stored.terms.len(), budget)?;
        for term in &stored.terms {
            inputs.push((copied(&term.exponents, budget)?, term.coefficient.clone()));
        }
        let mut terms = vector(inputs.len(), budget)?;
        for (exponents, coefficient) in inputs {
            let image = self.coefficient(cas, &coefficient, budget)?;
            let engine = Engine {
                state: &cas.state,
                domain: target,
            };
            if engine
                .formal_zero(
                    &engine.read(&image, budget).map_err(|error| fault(&error))?,
                    budget,
                )
                .map_err(|error| fault(&error))?
            {
                continue;
            }
            terms.push(PolynomialTerm {
                exponents,
                coefficient: Reference::Existing(image),
            });
        }
        materialize::admit(
            cas,
            SolveDraft::Polynomial(PolynomialData {
                domain: Reference::Existing(target.clone()),
                variables,
                terms,
            }),
            budget,
        )
    }

    /// Restrict explicitly supplied coefficient values, polynomials and dependent domains.
    pub(crate) fn object(
        &mut self,
        cas: &mut Cas,
        source: &Handle,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        budget.work(1, AT)?;
        match crate::solving::graph::object(&cas.state, cas.checked(source)?)? {
            SolveDraft::CoefficientDomain(_) => self.domain(cas, source, budget),
            SolveDraft::CoefficientValue(_) => self.coefficient(cas, source, budget),
            SolveDraft::Polynomial(stored) => {
                let original = stored.domain.clone();
                let target = self.domain(cas, &original, budget)?;
                if target == original {
                    Ok(source.clone())
                } else {
                    self.polynomial(cas, source, &target, budget)
                }
            }
            _ => Err(invalid()),
        }
    }
}
