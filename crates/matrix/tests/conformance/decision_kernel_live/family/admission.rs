// SPDX-License-Identifier: AGPL-3.0-only
//! Physical-family admission preserves the complete original declaration and immutable attachment.

use super::{TestResult, family_fixture, ledger, physical, source};
use nrz_bindings::readings::algebra::Location;
use nrz_bindings::{DecisionError, DecisionExpression, IntegerDecisionCase};

/// Missing or renamed physical inputs cannot acquire an association from a valid CAS box.
/// ´test:integration:kernel-physical-family-inventory-refuses´
/// ´claim:matrix:kernel-physical-family-inventory-refuses´
#[test]
fn kernel_physical_family_inventory_refuses() -> TestResult {
    for renamed in [false, true] {
        let (source, family) = family_fixture()?;
        let owner = physical()?;
        let before = source.frame.cas.counts();
        let physical_before = owner.cas()?.counts();
        let inputs = if renamed {
            vec![("period".into(), family)]
        } else {
            Vec::new()
        };
        let mut budget = ledger()?;
        budget.work(17, Location::new("prior physical inventory"))?;
        let Err(fault) = DecisionExpression::family(
            &source.frame.cas,
            owner.clone(),
            "quotient",
            &source.context,
            inputs,
            &mut budget,
        ) else {
            panic!("incomplete original physical inventory must refuse");
        };
        assert!(matches!(fault, DecisionError::Source));
        assert_eq!(source.frame.cas.counts(), before);
        assert_eq!(owner.cas()?.counts(), physical_before);
        assert!(budget.usage().work >= 17);
    }
    Ok(())
}

/// Equal family boxes and source values do not authenticate an independently declared attachment.
/// ´test:integration:kernel-physical-family-attachment-refuses´
/// ´claim:matrix:kernel-physical-family-attachment-refuses´
#[test]
fn kernel_physical_family_attachment_refuses() -> TestResult {
    let (association, source) = source(&mut ledger()?)?;
    let original = association.case();
    let other = IntegerDecisionCase::declare_family(
        &source.frame.cas,
        original.source().clone(),
        original.specification().clone(),
        &mut ledger()?,
    )?;
    let before = source.frame.cas.counts();
    let mut budget = ledger()?;
    budget.work(17, Location::new("prior physical attachment"))?;
    let Err(fault) = other.validate_attachment(&original.attachment(), &mut budget) else {
        panic!("another family declaration must refuse the original attachment");
    };
    assert!(matches!(fault, DecisionError::Attachment));
    assert_eq!(source.frame.cas.counts(), before);
    assert!(budget.usage().work >= 17);
    Ok(())
}

/// A different original operand cannot inherit the same physical source-box declaration.
/// ´test:integration:kernel-physical-family-target-refuses´
/// ´claim:matrix:kernel-physical-family-target-refuses´
#[test]
fn kernel_physical_family_target_refuses() -> TestResult {
    let (association, source) = source(&mut ledger()?)?;
    let original = association.case();
    let mut specification = original.specification().clone();
    specification.target = original.source().input_families()[0].1.source.clone();
    let before = source.frame.cas.counts();
    let mut budget = ledger()?;
    budget.work(17, Location::new("prior physical target"))?;
    let Err(fault) = IntegerDecisionCase::declare_family(
        &source.frame.cas,
        original.source().clone(),
        specification,
        &mut budget,
    ) else {
        panic!("a different original quotient must refuse");
    };
    assert!(matches!(fault, DecisionError::Target));
    assert_eq!(source.frame.cas.counts(), before);
    assert!(budget.usage().work >= 17);
    Ok(())
}
