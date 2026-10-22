// SPDX-License-Identifier: AGPL-3.0-only
//! Original-domain finite sets and independently replayable full factor products.

use super::leaf::Leaf;
use crate::graph::{Handle, Reference};
use crate::model::Operator;
use crate::solving::coefficients::{
    arithmetic::Engine,
    model::{CoefficientFailure, Element, EvaluationScope},
    storage::{invalid, vector},
};
use crate::solving::towers::{dense::Dense, materialize};
use crate::solving::{
    CertificateProductCompleteness, RootFactor, SolutionBranchData, SolutionBranchKind,
    SolutionSetData, SolveCertificate, SolveDraft, SolveSchema, SolveStatement, StatementCoverage,
    TupleData,
};

impl Leaf<'_> {
    /// The actual original unknown and equation remain the subjects of every coverage identity.
    pub(crate) fn source(&mut self) -> Result<(Handle, Handle), CoefficientFailure> {
        let original = crate::solving::elementary::proof::problem(self.cas, self.problem)?;
        let coordinates =
            crate::solving::elementary::proof::coordinates(self.cas, &original.unknowns)?;
        let [coordinate] = coordinates.coordinates.as_slice() else {
            return Err(invalid().into());
        };
        self.budget.work(
            crate::graph::size(coordinate.key.len(), super::AT)?,
            super::AT,
        )?;
        let key = coordinate.key.clone();
        let (crate::model::PredicateKind::Equal, [left, right]) =
            crate::solving::elementary::evidence::predicate(
                self.cas,
                original.predicate.logical_id(),
            )?
        else {
            return Err(invalid().into());
        };
        let endpoints = [self.cas.handle(*left), self.cas.handle(*right)];
        let expression = self.expression(Operator::Sub, &endpoints)?;
        Ok((self.expression(Operator::Free(key), &[])?, expression))
    }

    /// Horner rendering exposes the computed complete polynomial, including all original parameters.
    pub(crate) fn polynomial(
        &mut self,
        polynomial: &Dense,
        argument: &Handle,
    ) -> Result<Handle, CoefficientFailure> {
        let mut result = self.number(0, 1)?;
        for coefficient in polynomial.coefficients.iter().rev() {
            let value = self
                .frame
                .literal(self.cas, coefficient, &self.map, self.budget)?;
            let product = self.expression(Operator::Mul, &[result, argument.clone()])?;
            result = self.expression(Operator::Add, &[product, value])?;
        }
        Ok(result)
    }

    /// The source equality fixes the exact scale before root counting is allowed to imply coverage.
    pub(crate) fn source_identity(
        &mut self,
        polynomial: &Dense,
        scale: &Element,
    ) -> Result<u32, CoefficientFailure> {
        let (unknown, source) = self.source()?;
        let value = self.polynomial(polynomial, &unknown)?;
        let scale = self
            .frame
            .literal(self.cas, scale, &self.map, self.budget)?;
        let right = self.expression(Operator::Mul, &[scale, value])?;
        Ok(self.equality(&source, &right)?)
    }

    /// One finite branch retains the original solution domain and exact tuple coordinate frame.
    pub(crate) fn set(&mut self, entries: &[Handle]) -> Result<Handle, CoefficientFailure> {
        let original = crate::solving::elementary::proof::problem(self.cas, self.problem)?;
        let (coordinates, domain) = (original.unknowns.clone(), original.domain.clone());
        let mut tuples = vector(entries.len(), self.budget)?;
        for entry in entries {
            tuples.push(Reference::Existing(materialize::admit(
                self.cas,
                SolveDraft::Tuple(TupleData {
                    header: Reference::Existing(self.frame.header.clone()),
                    scope: Vec::new(),
                    coordinates: Reference::Existing(coordinates.clone()),
                    entries: vec![Reference::Existing(entry.clone())],
                }),
                self.budget,
            )?));
        }
        let kind = if tuples.is_empty() {
            SolutionBranchKind::Empty
        } else {
            SolutionBranchKind::Finite { tuples }
        };
        let branch = materialize::admit(
            self.cas,
            SolveDraft::SolutionBranch(SolutionBranchData {
                problem: Reference::Existing(self.problem.clone()),
                region: Reference::Existing(self.frame.region.clone()),
                domain: Reference::Existing(domain),
                kind,
            }),
            self.budget,
        )?;
        Ok(materialize::admit(
            self.cas,
            SolveDraft::SolutionSet(SolutionSetData {
                problem: Reference::Existing(self.problem.clone()),
                branches: vec![Reference::Existing(branch)],
            }),
            self.budget,
        )?)
    }

    /// Full factor reconstruction certifies all roots and their retained positive multiplicities.
    pub(crate) fn product(
        &mut self,
        scale: &Element,
        factors: &[(Handle, u32)],
    ) -> Result<Handle, CoefficientFailure> {
        self.mark("real-factor-coverage")?;
        let mut entries = vector(factors.len(), self.budget)?;
        entries.extend(factors.iter().map(|(root, _)| root.clone()));
        let set = self.set(&entries)?;
        let (unknown, source) = self.source()?;
        let scalar = self
            .frame
            .literal(self.cas, scale, &self.map, self.budget)?;
        let mut product = scalar.clone();
        let mut roots = vector(factors.len(), self.budget)?;
        for (root, multiplicity) in factors {
            let difference = self.expression(Operator::Sub, &[unknown.clone(), root.clone()])?;
            let power = self.expression(
                Operator::IntegerPower(i64::from(*multiplicity)),
                &[difference],
            )?;
            product = self.expression(Operator::Mul, &[product, power])?;
            roots.push(RootFactor {
                root: root.clone(),
                multiplicity: *multiplicity,
            });
        }
        let identity = self.equality(&source, &product)?;
        let region = self.frame.region.clone();
        self.frame.push(
            SolveSchema::ProductCompleteness,
            SolveStatement::Coverage(StatementCoverage {
                problem: self.problem.clone(),
                set: set.clone(),
                region: region.clone(),
            }),
            SolveCertificate::ProductCompleteness(CertificateProductCompleteness {
                problem: self.problem.clone(),
                set: set.clone(),
                scalar,
                factors: roots,
                product: identity,
                existence: Vec::new(),
                region,
            }),
            self.budget,
        )?;
        Ok(set)
    }

    /// The degree ladder ends in an exact empty set, one linear root or the full identity family.
    pub(crate) fn lower(&mut self, polynomial: &Dense) -> Result<Handle, CoefficientFailure> {
        if polynomial.coefficients.is_empty() {
            return Ok(super::free::coverage(
                self.cas,
                self.frame,
                self.problem,
                self.budget,
            )?);
        }
        let engine = Engine {
            state: &self.cas.state,
            domain: &polynomial.domain,
        };
        let scale = engine.copy(
            polynomial.coefficients.last().ok_or_else(invalid)?,
            self.budget,
        )?;
        if polynomial.coefficients.len() == 1 {
            return self.product(&scale, &[]);
        }
        let scope = EvaluationScope {
            context: &self.frame.context,
            region: &self.frame.region,
            premises: &[],
            localization: &[],
        };
        let reciprocal = super::derive::inverse(&engine, &scale, &scope, self.budget)?;
        let root = engine.neg(
            &engine.mul(
                polynomial.coefficients.first().ok_or_else(invalid)?,
                &reciprocal,
                self.budget,
            )?,
            self.budget,
        )?;
        let root = self
            .frame
            .literal(self.cas, &root, &self.map, self.budget)?;
        self.product(&scale, &[(root, 1)])
    }
}
