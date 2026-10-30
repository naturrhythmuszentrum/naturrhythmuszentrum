// SPDX-License-Identifier: AGPL-3.0-only
//! Selected matrix entries require the new grammar while old free-scalar receipts retain their scope.

use super::{TestResult, ledger, mapped_fixture};
use nrz_bindings::readings::algebra::{
    FaultKind, InvalidCode, Location, Rational,
    decision::{
        EnclosureOrigin, RationalEnclosure, ScalarKernelCertificate, ScalarKernelFamilyInput,
        ScalarKernelLaw, ScalarKernelRecord,
    },
};

/// Version three still refuses selected matrix entries before coverage or affine replay.
/// ´test:integration:kernel-mapped-family-version-refusal´
/// ´claim:matrix:kernel-mapped-family-version-refusal´
#[test]
fn kernel_mapped_family_version_refusal() -> TestResult {
    let (mut source, family, _) = mapped_fixture()?;
    let target = family.source.clone();
    let domain = source.frame.cas.admissible_region(&source.context)?;
    let record = ScalarKernelRecord {
        lower: family.lower.clone(),
        upper: family.upper.clone(),
        law: ScalarKernelLaw::Family(Box::new(ScalarKernelFamilyInput {
            center: Rational::from_ratio(1, 100, &mut ledger()?)?,
            radius: Rational::from_ratio(1, 1_000_000, &mut ledger()?)?,
            family,
        })),
    };
    let request = RationalEnclosure {
        target: target.clone(),
        context: source.context.clone(),
        input_family: domain.clone(),
        lower: record.lower.clone(),
        upper: record.upper.clone(),
        origin: EnclosureOrigin::ScalarKernel(Box::new(ScalarKernelCertificate {
            version: 3,
            expression: target,
            context: source.context.clone(),
            input_family: domain,
            records: vec![record],
        })),
    };
    let before = source.frame.cas.counts();
    let mut budget = ledger()?;
    budget.work(17, Location::new("prior old source grammar"))?;
    let Err(fault) = source
        .frame
        .cas
        .check_rational_enclosure(&request, &mut budget)
    else {
        panic!("version three must retain its free-scalar source grammar");
    };
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(
        fault.location,
        Location::new("scalar kernel family premise")
    );
    assert_eq!(source.frame.cas.counts(), before);
    assert!(budget.usage().work >= 17);
    Ok(())
}
