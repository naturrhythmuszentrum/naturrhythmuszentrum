// SPDX-License-Identifier: AGPL-3.0-only
//! Exact dependency traversal and child-before-parent reference resolution.

use super::model::{
    CoefficientEvaluationData, EvaluationBinding, FamilyArgument, FamilyExpressionData,
    FibrePieceData, ParameterBaseData, PlacementUse, RealCellData, ScalarPieceData,
    SpaceFamilyData, SpacePlacementData,
};
use super::proofs::{
    CertificateCoefficientEvaluation, CertificateRealCellExists, CertificateRealRootCount,
    CertificateRealSign, RealSignStep, StatementCoefficientEvaluation, StatementRealCellExists,
    StatementRealRootCount, StatementRealSign,
};
use crate::error::{Fault, Location};

/// Preserve ordered slots while reporting allocation failure as a value.
fn map_vector<R, S>(
    values: &[R],
    write: &mut impl FnMut(&R) -> Result<S, Fault>,
) -> Result<Vec<S>, Fault> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(values.len())
        .map_err(|_| Fault::allocation(Location::new("scalar-space references")))?;
    for value in values {
        result.push(write(value)?);
    }
    Ok(result)
}

impl<R> SpacePlacementData<R> {
    /// Resolve all references without changing their roles or slot order.
    pub(crate) fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<SpacePlacementData<S>, Fault> {
        Ok(SpacePlacementData {
            source: write(&self.source)?,
            ambient: write(&self.ambient)?,
            specification: self.specification.clone(),
            version: self.version,
        })
    }
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.source);
        result.push(&self.ambient);
    }
}

impl<R> ScalarPieceData<R> {
    /// Resolve all references without changing their roles or slot order.
    pub(crate) fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<ScalarPieceData<S>, Fault> {
        Ok(ScalarPieceData {
            placement: write(&self.placement)?,
            piece: self.piece,
            region: write(&self.region)?,
            local_frame: write(&self.local_frame)?,
        })
    }
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.placement);
        result.push(&self.region);
        result.push(&self.local_frame);
    }
}

impl<R> ParameterBaseData<R> {
    /// Resolve all references without changing their roles or slot order.
    pub(crate) fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<ParameterBaseData<S>, Fault> {
        Ok(ParameterBaseData {
            header: write(&self.header)?,
            coordinates: write(&self.coordinates)?,
            domain: write(&self.domain)?,
        })
    }
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.header);
        result.push(&self.coordinates);
        result.push(&self.domain);
    }
}

impl<R> SpaceFamilyData<R> {
    /// Resolve all references without changing their roles or slot order.
    pub(crate) fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<SpaceFamilyData<S>, Fault> {
        Ok(SpaceFamilyData {
            base: write(&self.base)?,
            fibre_space: write(&self.fibre_space)?,
            coefficients: write(&self.coefficients)?,
            unknowns: write(&self.unknowns)?,
        })
    }
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.base);
        result.push(&self.fibre_space);
        result.push(&self.coefficients);
        result.push(&self.unknowns);
    }
}

impl<R> FamilyArgument<R> {
    /// Resolve all references without changing their roles or slot order.
    pub(crate) fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<FamilyArgument<S>, Fault> {
        Ok(FamilyArgument {
            role: self.role,
            value: write(&self.value)?,
        })
    }
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.value);
    }
}

impl<R> FamilyExpressionData<R> {
    /// Resolve all references without changing their roles or slot order.
    pub(crate) fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<FamilyExpressionData<S>, Fault> {
        Ok(FamilyExpressionData {
            family: write(&self.family)?,
            ambient: write(&self.ambient)?,
            region: write(&self.region)?,
            scope: self.scope.clone(),
            operator: self.operator.clone(),
            result: self.result.clone(),
            arguments: map_vector(&self.arguments, &mut |value| value.resolve(write))?,
        })
    }
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.family);
        result.push(&self.ambient);
        result.push(&self.region);
        for value in &self.arguments {
            value.references(result);
        }
    }
}

impl<R> FibrePieceData<R> {
    /// Resolve all references without changing their roles or slot order.
    pub(crate) fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<FibrePieceData<S>, Fault> {
        Ok(FibrePieceData {
            family: write(&self.family)?,
            stratum: write(&self.stratum)?,
            region: write(&self.region)?,
            problem: write(&self.problem)?,
            set: write(&self.set)?,
        })
    }
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.family);
        result.push(&self.stratum);
        result.push(&self.region);
        result.push(&self.problem);
        result.push(&self.set);
    }
}

impl<R> RealCellData<R> {
    /// Resolve all references without changing their roles or slot order.
    pub(crate) fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<RealCellData<S>, Fault> {
        Ok(RealCellData {
            header: write(&self.header)?,
            scope: self.scope.clone(),
            region: write(&self.region)?,
            left: self.left.resolve(write)?,
            right: self.right.resolve(write)?,
            left_open: self.left_open,
            right_open: self.right_open,
        })
    }
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.header);
        result.push(&self.region);
        self.left.references(result);
        self.right.references(result);
    }
}

impl<R> EvaluationBinding<R> {
    /// Resolve all references without changing their roles or slot order.
    pub(crate) fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<EvaluationBinding<S>, Fault> {
        Ok(EvaluationBinding {
            slot: self.slot,
            image: write(&self.image)?,
        })
    }
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.image);
    }
}

impl<R> CoefficientEvaluationData<R> {
    /// Resolve all references without changing their roles or slot order.
    pub(crate) fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<CoefficientEvaluationData<S>, Fault> {
        Ok(CoefficientEvaluationData {
            source: write(&self.source)?,
            target: self.target.resolve(write)?,
            region: write(&self.region)?,
            bindings: map_vector(&self.bindings, &mut |value| value.resolve(write))?,
            localization: map_vector(&self.localization, write)?,
        })
    }
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.source);
        self.target.references(result);
        result.push(&self.region);
        for value in &self.bindings {
            value.references(result);
        }
        result.extend(&self.localization);
    }
}

impl<R> PlacementUse<R> {
    /// Resolve all references without changing their roles or slot order.
    pub(crate) fn resolve<S>(
        &self,
        write: &mut impl FnMut(&R) -> Result<S, Fault>,
    ) -> Result<PlacementUse<S>, Fault> {
        Ok(PlacementUse {
            operand: write(&self.operand)?,
            piece: write(&self.piece)?,
        })
    }
}

impl<R> RealSignStep<R> {
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.subject);
    }
}

impl<R> StatementCoefficientEvaluation<R> {
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.evaluation);
        result.push(&self.left);
        result.push(&self.right);
        result.push(&self.region);
    }
}

impl<R> CertificateCoefficientEvaluation<R> {
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.evaluation);
        result.push(&self.left);
        result.push(&self.right);
        result.extend(&self.support);
        result.push(&self.region);
    }
}

impl<R> StatementRealSign<R> {
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.subject);
        result.push(&self.region);
    }
}

impl<R> CertificateRealSign<R> {
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.subject);
        result.push(&self.region);
        for value in &self.steps {
            value.references(result);
        }
    }
}

impl<R> StatementRealRootCount<R> {
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.relation);
        result.push(&self.region);
        result.extend(&self.cells);
    }
}

impl<R> CertificateRealRootCount<R> {
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.relation);
        result.push(&self.region);
        result.extend(&self.cells);
        result.push(&self.square_free);
        result.push(&self.derivative);
        result.push(&self.gcd);
        result.extend(&self.chain);
        result.extend(&self.quotients);
        result.extend(&self.rescalings);
    }
}

impl<R> StatementRealCellExists<R> {
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.relation);
        result.push(&self.cell);
        result.push(&self.predicate);
        result.push(&self.region);
    }
}

impl<R> CertificateRealCellExists<R> {
    /// Enumerate every semantic dependency in payload order.
    pub(crate) fn references<'a>(&'a self, result: &mut Vec<&'a R>) {
        result.push(&self.relation);
        result.push(&self.cell);
        result.push(&self.predicate);
        result.push(&self.region);
    }
}
