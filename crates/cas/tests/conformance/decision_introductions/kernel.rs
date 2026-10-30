// SPDX-License-Identifier: AGPL-3.0-only
//! The scalar origin survives portable certified-membership replay in a fresh store.

use super::super::decision_baseline::Example;
use super::{frame_draft, membership, prepared_introduction};
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use nrz_cas::decision::{
    EnclosureOrigin, ScalarKernelCertificate, ScalarKernelLaw, ScalarKernelRecord,
};
use nrz_cas::proof::Certificate;

/// Complete scalar records remain attached to the same target and assumptions after wire replay.
/// ´test:integration:decision-kernel-membership-wire´
/// ´claim:cas:decision-kernel-membership-wire´
#[test]
fn decision_kernel_membership_wire() -> TestResult {
    let mut fixture = prepared_introduction(Example::Integer, true)?;
    let Certificate::Decision(evidence) =
        &mut fixture.steps.last_mut().ok_or("introduction")?.certificate
    else {
        return Err("missing introduction".into());
    };
    let request = &mut evidence.enclosures[0];
    request.origin = EnclosureOrigin::ScalarKernel(Box::new(ScalarKernelCertificate {
        version: 1,
        expression: request.target.clone(),
        context: request.context.clone(),
        input_family: request.input_family.clone(),
        records: vec![ScalarKernelRecord {
            law: ScalarKernelLaw::Rational(super::super::decision_partitions::rational(51, 16)?),
            lower: request.lower.clone(),
            upper: request.upper.clone(),
        }],
    }));
    let source = &mut fixture.partition.source;
    let proof = source
        .cas
        .check_proof(&source.context, &fixture.steps, &mut ledger()?)?;
    let draft = frame_draft(&fixture, &proof);
    let source = &mut fixture.partition.source;
    let frame = admit(&mut source.cas, draft, &mut ledger()?)?;
    let step = membership(&source.cas, &frame, &fixture.option)?;
    let proof = source.cas.check_proof(&frame, &[step], &mut ledger()?)?;
    let bytes = source.cas.export_witness(&proof, &mut ledger()?)?;
    let mut receiver = nrz_cas::Cas::new(ledger()?.limits().clone())?;
    let replayed = receiver.inspect(&bytes, &mut ledger()?)?;
    assert_eq!(receiver.export_witness(&replayed, &mut ledger()?)?, bytes);
    Ok(())
}
