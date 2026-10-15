// SPDX-License-Identifier: AGPL-3.0-only
//! Structural coefficient, region and selection dependencies of a header transport.

use super::Correspondence;
use crate::budget::Budget;
use crate::domain_model::{AmbientDomain, RegionExpr};
use crate::error::Fault;
use crate::graph::{Handle, LogicalId};
use crate::solving::coefficients::storage::reserve;
use crate::solving::elementary::proof::{AT, invalid};
use crate::solving::{CoefficientDomainData, CoefficientValueData, MapTarget, SolveDraft};

impl Correspondence<'_> {
    /// Coordinate products retain their declared positions and roles.
    fn coordinates(
        &self,
        real: &Handle,
        complex: &Handle,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let left = crate::solving::elementary::proof::coordinates(self.cas, real)?;
        let right = crate::solving::elementary::proof::coordinates(self.cas, complex)?;
        if (
            left.header.logical_id(),
            right.header.logical_id(),
            left.coordinates.len(),
        ) != (
            self.real.header.logical_id(),
            self.complex.header.logical_id(),
            right.coordinates.len(),
        ) {
            return Err(invalid());
        }
        for (a, b) in left.coordinates.iter().zip(&right.coordinates) {
            budget.work(1, AT)?;
            if a.role != b.role || !Self::scalar(&a.field, &b.field) {
                return Err(invalid());
            }
            if a.role != crate::solving::CoordinateRole::Family {
                self.declaration(&a.key, &b.key, budget)?;
            }
        }
        Ok(())
    }

    /// Region constructors preserve every guard, including false guards and exclusions.
    pub(crate) fn region(
        &self,
        real: LogicalId,
        complex: LogicalId,
        pending: &mut Vec<(LogicalId, LogicalId)>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let left = self.cas.state.region(real)?;
        let right = self.cas.state.region(complex)?;
        if (left.header, right.header)
            != (
                self.real.header.logical_id(),
                self.complex.header.logical_id(),
            )
            || !Self::scope(&left.scope, &right.scope)
        {
            return Err(invalid());
        }
        match (&left.ambient, &right.ambient) {
            (
                AmbientDomain::Product { coordinates: a },
                AmbientDomain::Product { coordinates: b },
            ) => self.coordinates(a, b, budget)?,
            (a, b) if a == b => {}
            _ => return Err(invalid()),
        }
        reserve(pending, 2, budget)?;
        match (&left.expression, &right.expression) {
            (RegionExpr::Ambient, RegionExpr::Ambient) | (RegionExpr::Empty, RegionExpr::Empty) => {
            }
            (
                RegionExpr::GuardTrue {
                    base: a,
                    predicate: p,
                },
                RegionExpr::GuardTrue {
                    base: b,
                    predicate: q,
                },
            )
            | (
                RegionExpr::GuardFalse {
                    base: a,
                    predicate: p,
                },
                RegionExpr::GuardFalse {
                    base: b,
                    predicate: q,
                },
            ) => pending.extend([(*a, *b), (*p, *q)]),
            (RegionExpr::Meet(a), RegionExpr::Meet(b))
            | (RegionExpr::Union(a), RegionExpr::Union(b))
                if a.len() == b.len() =>
            {
                reserve(pending, a.len(), budget)?;
                pending.extend(a.iter().zip(b).map(|(x, y)| (*x, *y)));
            }
            (a, b) if a == b => {}
            _ => return Err(invalid()),
        }
        Ok(())
    }

    /// Formal domains are paired structurally; quotient generators keep their defining relations.
    fn domain(
        &self,
        real: &CoefficientDomainData<Handle>,
        complex: &CoefficientDomainData<Handle>,
        pending: &mut Vec<(LogicalId, LogicalId)>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        reserve(pending, 2, budget)?;
        match (real, complex) {
            (
                CoefficientDomainData::Rational,
                CoefficientDomainData::Rational | CoefficientDomainData::Gaussian,
            )
            | (CoefficientDomainData::Gaussian, CoefficientDomainData::Gaussian) => {}
            (
                CoefficientDomainData::Fraction {
                    base: a,
                    coordinates: p,
                },
                CoefficientDomainData::Fraction {
                    base: b,
                    coordinates: q,
                },
            ) => {
                self.coordinates(p, q, budget)?;
                pending.push((a.logical_id(), b.logical_id()));
            }
            (
                CoefficientDomainData::Quotient { level: a },
                CoefficientDomainData::Quotient { level: b },
            ) => pending.push((a.logical_id(), b.logical_id())),
            _ => return Err(invalid()),
        }
        Ok(())
    }

    /// Literal normal forms keep all numerator and denominator dependencies.
    fn coefficient(
        real: &CoefficientValueData<Handle>,
        complex: &CoefficientValueData<Handle>,
        pending: &mut Vec<(LogicalId, LogicalId)>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        reserve(pending, 2, budget)?;
        match (real, complex) {
            (
                CoefficientValueData::Rational { value: a },
                CoefficientValueData::Rational { value: b },
            ) if a == b => {}
            (
                CoefficientValueData::Rational { value },
                CoefficientValueData::Gaussian { real, imaginary },
            ) if value == real && imaginary.is_zero() => {}
            (
                CoefficientValueData::Gaussian {
                    real: a,
                    imaginary: p,
                },
                CoefficientValueData::Gaussian {
                    real: b,
                    imaginary: q,
                },
            ) if (a, p) == (b, q) => {}
            (
                CoefficientValueData::Fraction {
                    numerator: a,
                    denominator: p,
                },
                CoefficientValueData::Fraction {
                    numerator: b,
                    denominator: q,
                },
            ) => pending.extend([
                (a.logical_id(), b.logical_id()),
                (p.logical_id(), q.logical_id()),
            ]),
            (
                CoefficientValueData::Tower { coefficients: a },
                CoefficientValueData::Tower { coefficients: b },
            ) => Self::pairs(pending, a, b, budget)?,
            _ => return Err(invalid()),
        }
        Ok(())
    }

    /// Scalar maps preserve the exact region, localization, ordered images and target field.
    fn mapping(
        &self,
        real: &crate::solving::CoefficientMapData<Handle>,
        complex: &crate::solving::CoefficientMapData<Handle>,
        pending: &mut Vec<(LogicalId, LogicalId)>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        if real.mode != complex.mode {
            return Err(invalid());
        }
        match (&real.target, &complex.target) {
            (
                MapTarget::ScalarAmbient {
                    header: a,
                    field: x,
                },
                MapTarget::ScalarAmbient {
                    header: b,
                    field: y,
                },
            ) if (a, b) == (&self.real.header, &self.complex.header) && Self::scalar(x, y) => {}
            (
                MapTarget::CoefficientDomain { domain: a },
                MapTarget::CoefficientDomain { domain: b },
            ) => {
                reserve(pending, 1, budget)?;
                pending.push((a.logical_id(), b.logical_id()));
            }
            _ => return Err(invalid()),
        }
        reserve(pending, 2, budget)?;
        pending.extend([
            (real.source.logical_id(), complex.source.logical_id()),
            (real.region.logical_id(), complex.region.logical_id()),
        ]);
        Self::pairs(pending, &real.images, &complex.images, budget)?;
        Self::pairs(pending, &real.denominators, &complex.denominators, budget)
    }

    /// Root relations and joint constraints transport together, never just the displayed root name.
    fn roots(
        real: &SolveDraft<Handle>,
        complex: &SolveDraft<Handle>,
        pending: &mut Vec<(LogicalId, LogicalId)>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        reserve(pending, 3, budget)?;
        match (real, complex) {
            (SolveDraft::RootSelection(a), SolveDraft::RootSelection(b))
                if (a.mode, &a.convention, &a.interval) == (b.mode, &b.convention, &b.interval)
                    && Self::scope(&a.scope, &b.scope) =>
            {
                Self::pairs(pending, &a.relations, &b.relations, budget)?;
                pending.push((a.predicate.logical_id(), b.predicate.logical_id()));
            }
            (SolveDraft::RootRelation(a), SolveDraft::RootRelation(b))
                if Self::scalar(&a.ambient, &b.ambient)
                    && a.multiplicity == b.multiplicity
                    && Self::scope(&a.scope, &b.scope) =>
            {
                pending.extend([
                    (a.polynomial.logical_id(), b.polynomial.logical_id()),
                    (a.map.logical_id(), b.map.logical_id()),
                    (a.region.logical_id(), b.region.logical_id()),
                ]);
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }

    /// Dispatch exact object dependencies without traversing unrelated graph content.
    pub(crate) fn object(
        &self,
        real: &SolveDraft<Handle>,
        complex: &SolveDraft<Handle>,
        pending: &mut Vec<(LogicalId, LogicalId)>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        reserve(pending, 2, budget)?;
        match (real, complex) {
            (SolveDraft::CoefficientDomain(a), SolveDraft::CoefficientDomain(b)) => {
                self.domain(a, b, pending, budget)?;
            }
            (SolveDraft::CoefficientValue(a), SolveDraft::CoefficientValue(b)) => {
                pending.push((a.domain.logical_id(), b.domain.logical_id()));
                Self::coefficient(&a.value, &b.value, pending, budget)?;
            }
            (SolveDraft::CoefficientMap(a), SolveDraft::CoefficientMap(b)) => {
                self.mapping(a, b, pending, budget)?;
            }
            (SolveDraft::Polynomial(a), SolveDraft::Polynomial(b))
                if a.terms.len() == b.terms.len()
                    && a.variables.len() == b.variables.len()
                    && a.variables
                        .iter()
                        .zip(&b.variables)
                        .all(|(x, y)| Self::scalar(x, y)) =>
            {
                pending.push((a.domain.logical_id(), b.domain.logical_id()));
                reserve(pending, a.terms.len(), budget)?;
                for (x, y) in a.terms.iter().zip(&b.terms) {
                    if x.exponents != y.exponents {
                        return Err(invalid());
                    }
                    pending.push((x.coefficient.logical_id(), y.coefficient.logical_id()));
                }
            }
            (SolveDraft::ExtensionLevel(a), SolveDraft::ExtensionLevel(b))
                if a.square_free == b.square_free =>
            {
                pending.extend([
                    (a.parent.logical_id(), b.parent.logical_id()),
                    (a.polynomial.logical_id(), b.polynomial.logical_id()),
                ]);
            }
            _ => Self::roots(real, complex, pending, budget)?,
        }
        Ok(())
    }
}
