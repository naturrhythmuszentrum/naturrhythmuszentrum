// SPDX-License-Identifier: AGPL-3.0-only
//! Raw schema round trips are distinct from admission and proof availability.

use crate::budget::{Budget, Limits};
use crate::encoding::Encoder;
use crate::error::{Fault, Location};
use crate::graph::{Cas, Draft, Handle};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::solving::{
    CoefficientEvaluationData, EvaluationBinding, EvaluationTarget, FamilyArgument,
    FamilyArgumentRole, FamilyExpressionData, FibrePieceData, ParameterBaseData, RealCellData,
    RealEndpoint, ScalarPieceData, ScalarSpaceData, SolveDraft, SpaceFamilyData,
    SpacePlacementData,
};
use crate::testing::common::TestResult;
use crate::wire_decode::Decoded;
use crate::wire_read::Reader;

/// Location for independent current-schema encoding tests.
const AT: Location = Location::new("current scalar schema test");

/// Build the handle payload for independent structural tests.
fn handle() -> TestResult<Handle> {
    let mut cas = Cas::new(Limits::default())?;
    let draft = Draft::Solve(SolveDraft::ScalarSpace(ScalarSpaceData {
        specification: "scalar.real".into(),
        version: 1,
    }));
    cas.admit(&[draft], &mut Budget::new(Limits::default())?)?
        .pop()
        .ok_or_else(|| "missing scalar node".into())
}

/// Build the reference payload for independent structural tests.
fn reference(_: &Handle, out: &mut Encoder<'_>) -> Result<(), Fault> {
    crate::wire::index(0, out)
}

/// Build the objects payload for independent structural tests.
fn objects(value: &Handle) -> Vec<SolveDraft<Handle>> {
    vec![
        objects_parameter_base(value),
        objects_space_family(value),
        objects_family_expression(value),
        objects_fibre_piece(value),
        objects_real_cell(value),
        objects_coefficient_evaluation(value),
        objects_scalar_space(),
        objects_space_placement(value),
        objects_scalar_piece(value),
    ]
}

/// Build the encode payload for independent structural tests.
fn encode(value: &SolveDraft<Handle>) -> TestResult<Vec<u8>> {
    let mut budget = Budget::new(Limits::default())?;
    let mut out = Encoder::new(&mut budget, AT);
    value.encode(&mut out, &mut reference)?;
    Ok(out.finish())
}

/// Build the decode payload for independent structural tests.
fn decode(tag: u8, bytes: &[u8], records: &[Decoded]) -> TestResult<SolveDraft<Handle>> {
    let mut budget = Budget::new(Limits::default())?;
    let mut reader = Reader::new(bytes, &mut budget)?;
    let result = SolveDraft::decode(tag, &mut reader, records)?;
    reader.finished()?;
    Ok(result)
}

/// Every new record has its specified distinct tag and deterministic complete payload.
/// ´test:crate:scalar-record-inventory-round-trips´
#[test]
fn scalar_record_inventory_round_trips() -> TestResult {
    let value = handle()?;
    let records = [Decoded::Object(value.clone())];
    for (position, object) in objects(&value).iter().enumerate() {
        assert_eq!(
            object.record_tag(),
            u8::try_from(position)?
                .checked_add(28)
                .ok_or("tag overflow")?
        );
        let bytes = encode(object)?;
        assert_eq!(decode(object.record_tag(), &bytes, &records)?, *object);
        assert_eq!(
            encode(&decode(object.record_tag(), &bytes, &records)?)?,
            bytes
        );
        for cut in 0..bytes.len() {
            assert!(decode(object.record_tag(), &bytes[..cut], &records).is_err());
        }
    }
    Ok(())
}

/// References remain earlier-record references; an absent dependency cannot be decoded.
/// ´test:crate:scalar-record-forward-reference-refuses´
#[test]
fn scalar_record_forward_reference_refuses() -> TestResult {
    let value = handle()?;
    for object in objects(&value) {
        if object.objects().is_empty() {
            continue;
        }
        assert!(decode(object.record_tag(), &encode(&object)?, &[]).is_err());
    }
    Ok(())
}

/// Both target forms and every endpoint tag are part of the one current schema.
/// ´test:crate:scalar-target-and-endpoint-variants-round-trip´
#[test]
fn scalar_target_and_endpoint_variants_round_trip() -> TestResult {
    let value = handle()?;
    let records = [Decoded::Object(value.clone())];
    let evaluation = SolveDraft::CoefficientEvaluation(CoefficientEvaluationData {
        source: value.clone(),
        target: EvaluationTarget::Family {
            family: value.clone(),
        },
        region: value.clone(),
        bindings: Vec::new(),
        localization: Vec::new(),
    });
    assert_eq!(
        decode(evaluation.record_tag(), &encode(&evaluation)?, &records)?,
        evaluation
    );
    let cell = SolveDraft::RealCell(RealCellData {
        header: value.clone(),
        scope: Vec::new(),
        region: value,
        left: RealEndpoint::NegativeInfinity,
        right: RealEndpoint::PositiveInfinity,
        left_open: true,
        right_open: true,
    });
    assert_eq!(decode(cell.record_tag(), &encode(&cell)?, &records)?, cell);
    Ok(())
}

use crate::solving::{
    CertificateCoefficientEvaluation, CertificateRealCellExists, CertificateRealRootCount,
    CertificateRealSign, RealSignConclusion, RealSignRule, RealSignStep, SolveCertificate,
    SolveStatement, StatementCoefficientEvaluation, StatementRealCellExists,
    StatementRealRootCount, StatementRealSign,
};

/// Build the statements payload for independent structural tests.
fn statements(value: &Handle) -> Vec<SolveStatement<Handle>> {
    vec![
        statements_coefficient_evaluation(value),
        statements_real_sign(value),
        statements_real_root_count(value),
        statements_real_cell_exists(value),
    ]
}

/// Build the certificates payload for independent structural tests.
fn certificates(value: &Handle) -> Vec<SolveCertificate<Handle>> {
    vec![
        certificates_coefficient_evaluation(value),
        certificates_real_sign(value),
        certificates_real_root_count(value),
        certificates_real_cell_exists(value),
    ]
}

/// Build the sign steps payload for independent structural tests.
fn sign_steps(value: &Handle) -> Vec<RealSignStep<Handle>> {
    [
        RealSignRule::Rational,
        RealSignRule::Identity,
        RealSignRule::Negation,
        RealSignRule::Sum,
        RealSignRule::Product,
        RealSignRule::EvenPower,
        RealSignRule::OddPowerReflection,
        RealSignRule::Hypothesis,
        RealSignRule::Root,
    ]
    .into_iter()
    .map(|rule| RealSignStep {
        rule,
        subject: value.clone(),
        conclusion: RealSignConclusion::Positive,
        arguments: vec![0],
        premises: vec![1],
        exponent: 3,
    })
    .collect()
}

/// New proof payload syntax is complete independently of checker availability.
/// ´test:crate:scalar-statement-inventory-round-trips´
#[test]
fn scalar_statement_inventory_round_trips() -> TestResult {
    let value = handle()?;
    let records = [Decoded::Object(value.clone())];
    for (position, object) in statements(&value).iter().enumerate() {
        let mut budget = Budget::new(Limits::default())?;
        let mut out = Encoder::new(&mut budget, AT);
        object.encode(&mut out, &mut reference)?;
        let bytes = out.finish();
        assert_eq!(
            bytes.first(),
            Some(
                &u8::try_from(position)?
                    .checked_add(20)
                    .ok_or("tag overflow")?
            )
        );
        let mut reader = Reader::new(&bytes, &mut budget)?;
        assert_eq!(SolveStatement::decode(&mut reader, &records)?, *object);
        reader.finished()?;
        for cut in 0..bytes.len() {
            let mut budget = Budget::new(Limits::default())?;
            let mut reader = Reader::new(&bytes[..cut], &mut budget)?;
            assert!(SolveStatement::decode(&mut reader, &records).is_err());
        }
    }
    Ok(())
}

/// New proof payload syntax is complete independently of checker availability.
/// ´test:crate:scalar-certificate-inventory-round-trips´
#[test]
fn scalar_certificate_inventory_round_trips() -> TestResult {
    let value = handle()?;
    let records = [Decoded::Object(value.clone())];
    for (position, object) in certificates(&value).iter().enumerate() {
        let mut budget = Budget::new(Limits::default())?;
        let mut out = Encoder::new(&mut budget, AT);
        object.encode(&mut out, &mut reference)?;
        let bytes = out.finish();
        assert_eq!(
            bytes.first(),
            Some(
                &u8::try_from(position)?
                    .checked_add(18)
                    .ok_or("tag overflow")?
            )
        );
        let mut reader = Reader::new(&bytes, &mut budget)?;
        assert_eq!(SolveCertificate::decode(&mut reader, &records)?, *object);
        reader.finished()?;
        for cut in 0..bytes.len() {
            let mut budget = Budget::new(Limits::default())?;
            let mut reader = Reader::new(&bytes[..cut], &mut budget)?;
            assert!(SolveCertificate::decode(&mut reader, &records).is_err());
        }
    }
    Ok(())
}

/// Withdrawn certificate tag thirteen cannot be reinterpreted under the current magic.
/// ´test:crate:scalar-withdrawn-certificate-tag-refuses´
#[test]
fn scalar_withdrawn_certificate_tag_refuses() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let mut reader = Reader::new(&[13, 0, 0, 0, 0], &mut budget)?;
    assert!(SolveCertificate::decode(&mut reader, &[]).is_err());
    Ok(())
}

/// Build the objects parameter base payload for independent structural tests.
fn objects_parameter_base(value: &Handle) -> SolveDraft<Handle> {
    SolveDraft::ParameterBase(ParameterBaseData {
        header: value.clone(),
        coordinates: value.clone(),
        domain: value.clone(),
    })
}

/// Build the objects space family payload for independent structural tests.
fn objects_space_family(value: &Handle) -> SolveDraft<Handle> {
    SolveDraft::SpaceFamily(SpaceFamilyData {
        base: value.clone(),
        fibre_space: value.clone(),
        coefficients: value.clone(),
        unknowns: value.clone(),
    })
}

/// Build the objects family expression payload for independent structural tests.
fn objects_family_expression(value: &Handle) -> SolveDraft<Handle> {
    SolveDraft::FamilyExpression(FamilyExpressionData {
        family: value.clone(),
        ambient: value.clone(),
        region: value.clone(),
        scope: Vec::new(),
        operator: Operator::Add,
        result: Sort::Value(AtomicSort::Scalar(Field::Complex)),
        arguments: vec![
            FamilyArgument {
                role: FamilyArgumentRole::Base,
                value: value.clone(),
            },
            FamilyArgument {
                role: FamilyArgumentRole::Fibre,
                value: value.clone(),
            },
            FamilyArgument {
                role: FamilyArgumentRole::Family,
                value: value.clone(),
            },
        ],
    })
}

/// Build the objects fibre piece payload for independent structural tests.
fn objects_fibre_piece(value: &Handle) -> SolveDraft<Handle> {
    SolveDraft::FibrePiece(FibrePieceData {
        family: value.clone(),
        stratum: value.clone(),
        region: value.clone(),
        problem: value.clone(),
        set: value.clone(),
    })
}

/// Build the objects real cell payload for independent structural tests.
fn objects_real_cell(value: &Handle) -> SolveDraft<Handle> {
    SolveDraft::RealCell(RealCellData {
        header: value.clone(),
        scope: Vec::new(),
        region: value.clone(),
        left: RealEndpoint::RealExpression(value.clone()),
        right: RealEndpoint::PositiveInfinity,
        left_open: false,
        right_open: true,
    })
}

/// Build the objects coefficient evaluation payload for independent structural tests.
fn objects_coefficient_evaluation(value: &Handle) -> SolveDraft<Handle> {
    SolveDraft::CoefficientEvaluation(CoefficientEvaluationData {
        source: value.clone(),
        target: EvaluationTarget::Scalar {
            header: value.clone(),
            space: value.clone(),
        },
        region: value.clone(),
        bindings: vec![EvaluationBinding {
            slot: 0,
            image: value.clone(),
        }],
        localization: vec![value.clone()],
    })
}

/// Build the objects scalar space payload for independent structural tests.
fn objects_scalar_space() -> SolveDraft<Handle> {
    SolveDraft::ScalarSpace(ScalarSpaceData {
        specification: "scalar.real".into(),
        version: 1,
    })
}

/// Build the objects space placement payload for independent structural tests.
fn objects_space_placement(value: &Handle) -> SolveDraft<Handle> {
    SolveDraft::SpacePlacement(SpacePlacementData {
        source: value.clone(),
        ambient: value.clone(),
        specification: "placement.real-in-complex".into(),
        version: 1,
    })
}

/// Build the objects scalar piece payload for independent structural tests.
fn objects_scalar_piece(value: &Handle) -> SolveDraft<Handle> {
    SolveDraft::ScalarPiece(ScalarPieceData {
        placement: value.clone(),
        piece: 0,
        region: value.clone(),
        local_frame: value.clone(),
    })
}

/// Build the statements coefficient evaluation payload for independent structural tests.
fn statements_coefficient_evaluation(value: &Handle) -> SolveStatement<Handle> {
    SolveStatement::CoefficientEvaluation(StatementCoefficientEvaluation {
        evaluation: value.clone(),
        left: value.clone(),
        right: value.clone(),
        region: value.clone(),
    })
}

/// Build the statements real sign payload for independent structural tests.
fn statements_real_sign(value: &Handle) -> SolveStatement<Handle> {
    SolveStatement::RealSign(StatementRealSign {
        subject: value.clone(),
        region: value.clone(),
        conclusion: RealSignConclusion::Positive,
    })
}

/// Build the statements real root count payload for independent structural tests.
fn statements_real_root_count(value: &Handle) -> SolveStatement<Handle> {
    SolveStatement::RealRootCount(StatementRealRootCount {
        relation: value.clone(),
        region: value.clone(),
        cells: vec![value.clone()],
        counts: vec![0, 1],
        exterior_left: 1,
        exterior_right: 1,
    })
}

/// Build the statements real cell exists payload for independent structural tests.
fn statements_real_cell_exists(value: &Handle) -> SolveStatement<Handle> {
    SolveStatement::RealCellExists(StatementRealCellExists {
        relation: value.clone(),
        cell: value.clone(),
        predicate: value.clone(),
        region: value.clone(),
    })
}

/// Build the certificates coefficient evaluation payload for independent structural tests.
fn certificates_coefficient_evaluation(value: &Handle) -> SolveCertificate<Handle> {
    SolveCertificate::CoefficientEvaluation(CertificateCoefficientEvaluation {
        evaluation: value.clone(),
        left: value.clone(),
        right: value.clone(),
        support: vec![value.clone()],
        definedness: vec![0, 1],
        units: vec![0, 1],
        existence: vec![0, 1],
        region: value.clone(),
    })
}

/// Build the certificates real sign payload for independent structural tests.
fn certificates_real_sign(value: &Handle) -> SolveCertificate<Handle> {
    SolveCertificate::RealSign(CertificateRealSign {
        subject: value.clone(),
        region: value.clone(),
        steps: sign_steps(value),
        root: 1,
    })
}

/// Build the certificates real root count payload for independent structural tests.
fn certificates_real_root_count(value: &Handle) -> SolveCertificate<Handle> {
    SolveCertificate::RealRootCount(CertificateRealRootCount {
        relation: value.clone(),
        region: value.clone(),
        cells: vec![value.clone()],
        square_free: value.clone(),
        derivative: value.clone(),
        gcd: value.clone(),
        chain: vec![value.clone()],
        quotients: vec![value.clone()],
        rescalings: vec![value.clone()],
        identities: vec![0, 1],
        units: vec![0, 1],
        signs: vec![0, 1],
        counts: vec![0, 1],
        exterior_left: 1,
        exterior_right: 1,
    })
}

/// Build the certificates real cell exists payload for independent structural tests.
fn certificates_real_cell_exists(value: &Handle) -> SolveCertificate<Handle> {
    SolveCertificate::RealCellExists(CertificateRealCellExists {
        relation: value.clone(),
        cell: value.clone(),
        predicate: value.clone(),
        region: value.clone(),
        count: 1,
        definedness: vec![0, 1],
        order: vec![0, 1],
    })
}
