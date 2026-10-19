// SPDX-License-Identifier: AGPL-3.0-only
//! Sole-current bounded codecs for scalar-space and finite proof syntax.

use super::model::{
    CoefficientEvaluationData, EvaluationBinding, FamilyArgument, FamilyExpressionData,
    FibrePieceData, ParameterBaseData, RealCellData, ScalarPieceData, ScalarSpaceData,
    SpaceFamilyData, SpacePlacementData,
};
use super::model::{EvaluationTarget, FamilyArgumentRole, RealEndpoint};
use super::proofs::{
    CertificateCoefficientEvaluation, CertificateRealCellExists, CertificateRealRootCount,
    CertificateRealSign, RealSignStep, StatementCoefficientEvaluation, StatementRealCellExists,
    StatementRealRootCount, StatementRealSign,
};
use super::proofs::{RealSignConclusion, RealSignRule};
use crate::budget::BudgetKind;
use crate::encoding::{self, Encoder};
use crate::error::{Fault, Location};
use crate::graph::Handle;
use crate::wire_decode::{Decoded, object};
use crate::wire_read::{Reader, invalid};

/// Check the count and retained bytes before allocating any vector.
pub(super) fn vector<T>(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
    mut read: impl FnMut(&mut Reader<'_, '_>, &[Decoded]) -> Result<T, Fault>,
) -> Result<Vec<T>, Fault> {
    let count = reader.count(BudgetKind::Edges, 4, 256)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(Location::new("scalar-space decoding")))?;
    for _ in 0..count {
        values.push(read(reader, records)?);
    }
    Ok(values)
}

/// Booleans have exactly two portable spellings.
pub(super) fn boolean(reader: &mut Reader<'_, '_>) -> Result<bool, Fault> {
    match reader.byte()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid()),
    }
}

impl ScalarSpaceData {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode<R>(
        &self,
        out: &mut Encoder<'_>,
        _write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.name(&self.specification)?;
        crate::wire::index(self.version, out)?;
        Ok(())
    }
}

impl ScalarSpaceData {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, _records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            specification: reader.name()?,
            version: reader.index()?,
        })
    }
}

impl<R> SpacePlacementData<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.source, out)?;
        write(&self.ambient, out)?;
        out.name(&self.specification)?;
        crate::wire::index(self.version, out)?;
        Ok(())
    }
}

impl SpacePlacementData<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            source: object(reader, records)?,
            ambient: object(reader, records)?,
            specification: reader.name()?,
            version: reader.index()?,
        })
    }
}

impl<R> ScalarPieceData<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.placement, out)?;
        crate::wire::index(self.piece, out)?;
        write(&self.region, out)?;
        write(&self.local_frame, out)?;
        Ok(())
    }
}

impl ScalarPieceData<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            placement: object(reader, records)?,
            piece: reader.index()?,
            region: object(reader, records)?,
            local_frame: object(reader, records)?,
        })
    }
}

impl<R> ParameterBaseData<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.header, out)?;
        write(&self.coordinates, out)?;
        write(&self.domain, out)?;
        Ok(())
    }
}

impl ParameterBaseData<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            header: object(reader, records)?,
            coordinates: object(reader, records)?,
            domain: object(reader, records)?,
        })
    }
}

impl<R> SpaceFamilyData<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.base, out)?;
        write(&self.fibre_space, out)?;
        write(&self.coefficients, out)?;
        write(&self.unknowns, out)?;
        Ok(())
    }
}

impl SpaceFamilyData<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            base: object(reader, records)?,
            fibre_space: object(reader, records)?,
            coefficients: object(reader, records)?,
            unknowns: object(reader, records)?,
        })
    }
}

impl<R> FamilyArgument<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.byte(self.role.tag())?;
        write(&self.value, out)?;
        Ok(())
    }
}

impl FamilyArgument<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            role: FamilyArgumentRole::decode(reader)?,
            value: object(reader, records)?,
        })
    }
}

impl<R> FamilyExpressionData<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.family, out)?;
        write(&self.ambient, out)?;
        write(&self.region, out)?;
        encoding::scope(&self.scope, out)?;
        encoding::operator(&self.operator, out)?;
        encoding::sort(&self.result, out)?;
        out.count(self.arguments.len())?;
        for value in &self.arguments {
            value.encode(out, write)?;
        }
        Ok(())
    }
}

impl FamilyExpressionData<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            family: object(reader, records)?,
            ambient: object(reader, records)?,
            region: object(reader, records)?,
            scope: reader.scope()?,
            operator: reader.operator()?,
            result: reader.sort()?,
            arguments: vector(reader, records, FamilyArgument::decode)?,
        })
    }
}

impl<R> FibrePieceData<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.family, out)?;
        write(&self.stratum, out)?;
        write(&self.region, out)?;
        write(&self.problem, out)?;
        write(&self.set, out)?;
        Ok(())
    }
}

impl FibrePieceData<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            family: object(reader, records)?,
            stratum: object(reader, records)?,
            region: object(reader, records)?,
            problem: object(reader, records)?,
            set: object(reader, records)?,
        })
    }
}

impl<R> RealCellData<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.header, out)?;
        encoding::scope(&self.scope, out)?;
        write(&self.region, out)?;
        self.left.encode(out, write)?;
        self.right.encode(out, write)?;
        out.byte(u8::from(self.left_open))?;
        out.byte(u8::from(self.right_open))?;
        Ok(())
    }
}

impl RealCellData<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            header: object(reader, records)?,
            scope: reader.scope()?,
            region: object(reader, records)?,
            left: RealEndpoint::decode(reader, records)?,
            right: RealEndpoint::decode(reader, records)?,
            left_open: boolean(reader)?,
            right_open: boolean(reader)?,
        })
    }
}

impl<R> EvaluationBinding<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        crate::wire::index(self.slot, out)?;
        write(&self.image, out)?;
        Ok(())
    }
}

impl EvaluationBinding<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            slot: reader.index()?,
            image: object(reader, records)?,
        })
    }
}

impl<R> CoefficientEvaluationData<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.source, out)?;
        self.target.encode(out, write)?;
        write(&self.region, out)?;
        out.count(self.bindings.len())?;
        for value in &self.bindings {
            value.encode(out, write)?;
        }
        out.count(self.localization.len())?;
        for value in &self.localization {
            write(value, out)?;
        }
        Ok(())
    }
}

impl CoefficientEvaluationData<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            source: object(reader, records)?,
            target: EvaluationTarget::decode(reader, records)?,
            region: object(reader, records)?,
            bindings: vector(reader, records, EvaluationBinding::decode)?,
            localization: vector(reader, records, object)?,
        })
    }
}

impl<R> RealSignStep<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        out.byte(self.rule.tag())?;
        write(&self.subject, out)?;
        out.byte(self.conclusion.tag())?;
        out.count(self.arguments.len())?;
        for value in &self.arguments {
            crate::wire::index(*value, out)?;
        }
        out.count(self.premises.len())?;
        for value in &self.premises {
            crate::wire::index(*value, out)?;
        }
        crate::wire::index(self.exponent, out)?;
        Ok(())
    }
}

impl RealSignStep<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            rule: RealSignRule::decode(reader)?,
            subject: object(reader, records)?,
            conclusion: RealSignConclusion::decode(reader)?,
            arguments: vector(reader, records, |reader, _records| reader.index())?,
            premises: vector(reader, records, |reader, _records| reader.index())?,
            exponent: reader.index()?,
        })
    }
}

impl<R> StatementCoefficientEvaluation<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.evaluation, out)?;
        write(&self.left, out)?;
        write(&self.right, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl StatementCoefficientEvaluation<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            evaluation: object(reader, records)?,
            left: object(reader, records)?,
            right: object(reader, records)?,
            region: object(reader, records)?,
        })
    }
}

impl<R> CertificateCoefficientEvaluation<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.evaluation, out)?;
        write(&self.left, out)?;
        write(&self.right, out)?;
        out.count(self.support.len())?;
        for value in &self.support {
            write(value, out)?;
        }
        out.count(self.definedness.len())?;
        for value in &self.definedness {
            crate::wire::index(*value, out)?;
        }
        out.count(self.units.len())?;
        for value in &self.units {
            crate::wire::index(*value, out)?;
        }
        out.count(self.existence.len())?;
        for value in &self.existence {
            crate::wire::index(*value, out)?;
        }
        write(&self.region, out)?;
        Ok(())
    }
}

impl CertificateCoefficientEvaluation<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            evaluation: object(reader, records)?,
            left: object(reader, records)?,
            right: object(reader, records)?,
            support: vector(reader, records, object)?,
            definedness: vector(reader, records, |reader, _records| reader.index())?,
            units: vector(reader, records, |reader, _records| reader.index())?,
            existence: vector(reader, records, |reader, _records| reader.index())?,
            region: object(reader, records)?,
        })
    }
}

impl<R> StatementRealSign<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.subject, out)?;
        write(&self.region, out)?;
        out.byte(self.conclusion.tag())?;
        Ok(())
    }
}

impl StatementRealSign<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            subject: object(reader, records)?,
            region: object(reader, records)?,
            conclusion: RealSignConclusion::decode(reader)?,
        })
    }
}

impl<R> CertificateRealSign<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.subject, out)?;
        write(&self.region, out)?;
        out.count(self.steps.len())?;
        for value in &self.steps {
            value.encode(out, write)?;
        }
        crate::wire::index(self.root, out)?;
        Ok(())
    }
}

impl CertificateRealSign<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            subject: object(reader, records)?,
            region: object(reader, records)?,
            steps: vector(reader, records, RealSignStep::decode)?,
            root: reader.index()?,
        })
    }
}

impl<R> StatementRealRootCount<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.relation, out)?;
        write(&self.region, out)?;
        out.count(self.cells.len())?;
        for value in &self.cells {
            write(value, out)?;
        }
        out.count(self.counts.len())?;
        for value in &self.counts {
            crate::wire::index(*value, out)?;
        }
        crate::wire::index(self.exterior_left, out)?;
        crate::wire::index(self.exterior_right, out)?;
        Ok(())
    }
}

impl StatementRealRootCount<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            relation: object(reader, records)?,
            region: object(reader, records)?,
            cells: vector(reader, records, object)?,
            counts: vector(reader, records, |reader, _records| reader.index())?,
            exterior_left: reader.index()?,
            exterior_right: reader.index()?,
        })
    }
}

impl<R> CertificateRealRootCount<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.relation, out)?;
        write(&self.region, out)?;
        out.count(self.cells.len())?;
        for value in &self.cells {
            write(value, out)?;
        }
        write(&self.square_free, out)?;
        write(&self.derivative, out)?;
        write(&self.gcd, out)?;
        out.count(self.chain.len())?;
        for value in &self.chain {
            write(value, out)?;
        }
        out.count(self.quotients.len())?;
        for value in &self.quotients {
            write(value, out)?;
        }
        out.count(self.rescalings.len())?;
        for value in &self.rescalings {
            write(value, out)?;
        }
        out.count(self.identities.len())?;
        for value in &self.identities {
            crate::wire::index(*value, out)?;
        }
        out.count(self.units.len())?;
        for value in &self.units {
            crate::wire::index(*value, out)?;
        }
        out.count(self.signs.len())?;
        for value in &self.signs {
            crate::wire::index(*value, out)?;
        }
        out.count(self.counts.len())?;
        for value in &self.counts {
            crate::wire::index(*value, out)?;
        }
        crate::wire::index(self.exterior_left, out)?;
        crate::wire::index(self.exterior_right, out)?;
        Ok(())
    }
}

impl CertificateRealRootCount<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            relation: object(reader, records)?,
            region: object(reader, records)?,
            cells: vector(reader, records, object)?,
            square_free: object(reader, records)?,
            derivative: object(reader, records)?,
            gcd: object(reader, records)?,
            chain: vector(reader, records, object)?,
            quotients: vector(reader, records, object)?,
            rescalings: vector(reader, records, object)?,
            identities: vector(reader, records, |reader, _records| reader.index())?,
            units: vector(reader, records, |reader, _records| reader.index())?,
            signs: vector(reader, records, |reader, _records| reader.index())?,
            counts: vector(reader, records, |reader, _records| reader.index())?,
            exterior_left: reader.index()?,
            exterior_right: reader.index()?,
        })
    }
}

impl<R> StatementRealCellExists<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.relation, out)?;
        write(&self.cell, out)?;
        write(&self.predicate, out)?;
        write(&self.region, out)?;
        Ok(())
    }
}

impl StatementRealCellExists<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            relation: object(reader, records)?,
            cell: object(reader, records)?,
            predicate: object(reader, records)?,
            region: object(reader, records)?,
        })
    }
}

impl<R> CertificateRealCellExists<R> {
    /// Encode the complete current payload in semantic field order.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.relation, out)?;
        write(&self.cell, out)?;
        write(&self.predicate, out)?;
        write(&self.region, out)?;
        crate::wire::index(self.count, out)?;
        out.count(self.definedness.len())?;
        for value in &self.definedness {
            crate::wire::index(*value, out)?;
        }
        out.count(self.order.len())?;
        for value in &self.order {
            crate::wire::index(*value, out)?;
        }
        Ok(())
    }
}

impl CertificateRealCellExists<Handle> {
    /// Decode only earlier references and bounded finite payloads.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<Self, Fault> {
        Ok(Self {
            relation: object(reader, records)?,
            cell: object(reader, records)?,
            predicate: object(reader, records)?,
            region: object(reader, records)?,
            count: reader.index()?,
            definedness: vector(reader, records, |reader, _records| reader.index())?,
            order: vector(reader, records, |reader, _records| reader.index())?,
        })
    }
}
