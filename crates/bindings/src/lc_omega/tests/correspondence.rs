// SPDX-License-Identifier: AGPL-3.0-only
//! Fresh receiving declarations and portable evidence never share checked store authority.
use super::*;
mod keys;
mod oracle;
mod resources;

/// Independent fixtures preserve ordered repeated inputs across fresh-store replay.
/// ´test:unit:lc-omega-correspondence-construct-replay´
#[test]
fn lc_omega_correspondence_construct_replay() -> TestResult {
    receiving_policy()?;
    let mut budget = Budget::new(Limits::default())?;
    let source = fixture(&mut budget)?;
    let declaration = owning::admit(source.request(), &mut budget)?;
    let checked = LcOmegaCorrespondence::construct(&declaration, &mut budget)?;
    checked.check_declaration(&declaration, &mut budget)?;
    let certificate = checked.certificate(&mut budget)?;
    let receiver = fixture(&mut budget)?;
    let receiving = owning::admit(receiver.request(), &mut budget)?;
    let replayed = LcOmegaCorrespondence::replay(&receiving, &certificate, &mut budget)?;
    replayed.check_declaration(&receiving, &mut budget)?;
    assert_eq!(
        replayed.check_declaration(&declaration, &mut budget),
        Err(AdmissionError::ValueMismatch)
    );
    let exported = replayed.certificate(&mut budget)?;
    assert_eq!(
        certificate.to_bytes(&mut budget)?,
        exported.to_bytes(&mut budget)?
    );
    Ok(())
}

/// Declaration identity and untrusted transcript categories remain independent authorities.
/// ´test:unit:lc-omega-correspondence-identity-refusals´
#[test]
fn lc_omega_correspondence_identity_refusals() -> TestResult {
    owning::identities()?;
    let mut budget = Budget::new(Limits::default())?;
    let f = fixture(&mut budget)?;
    let declaration = owning::admit(f.request(), &mut budget)?;
    let separate = owning::admit(f.request(), &mut budget)?;
    let checked = LcOmegaCorrespondence::construct(&declaration, &mut budget)?;
    assert_eq!(
        checked.check_declaration(&separate, &mut budget),
        Err(AdmissionError::ValueMismatch)
    );
    let certificate = checked.certificate(&mut budget)?;
    let bytes = certificate.to_bytes(&mut budget)?;
    oracle::wire(&bytes)?;
    for index in [0, 8, bytes.len() - 1] {
        let mut budget = Budget::new(Limits::default())?;
        let mut changed = bytes.clone();
        changed[index] ^= 1;
        let changed = nrz_cas::enclosure::lc_root::Certificate::from_bytes(&changed, &mut budget)?;
        assert!(matches!(
            LcOmegaCorrespondence::replay(&declaration, &changed, &mut budget),
            Err(AdmissionError::Algebra(_))
        ));
    }
    let mut alternate = f.request();
    alternate.positions = [3, 1];
    let alternate = owning::admit(alternate, &mut budget)?;
    assert!(matches!(
        LcOmegaCorrespondence::replay(&alternate, &certificate, &mut budget),
        Err(AdmissionError::Algebra(_))
    ));
    for intervals in [
        [["1", "9"], ["1", "1"], ["0", "0"], ["1", "16"], ["0", "1"]],
        [["1", "9"], ["1", "1"], ["0", "0"], ["1", "4"], ["0", "2"]],
    ] {
        let mut budget = Budget::new(Limits::default())?;
        let changed = fixture_with_intervals(intervals, &mut budget)?;
        let changed = owning::admit(changed.request(), &mut budget)?;
        assert!(matches!(
            LcOmegaCorrespondence::replay(&changed, &certificate, &mut budget),
            Err(AdmissionError::Algebra(_))
        ));
    }
    crate::lc_omega::correspondence::controls::identity(
        &declaration,
        &mut Budget::new(Limits::default())?,
    )
}

/// A restrictive mapped store contributes; a stricter private output store does not.
fn receiving_policy() -> TestResult {
    let mut preparation = Budget::new(Limits::default())?;
    let mut f = fixture(&mut preparation)?;
    let mut input_policy = Budget::new(Limits {
        name_bytes: 64,
        ..Limits::default()
    })?;
    f.inputs[4][1] = BindingSymbols::declare(
        &LcQuantity::Energy.descriptor()?,
        [1, 1],
        [true; 2],
        &mut input_policy,
    )?;
    let mut output_policy = Budget::new(Limits {
        name_bytes: 32,
        ..Limits::default()
    })?;
    f.output = BindingSymbols::declare(
        &LcQuantity::AngularFrequency.descriptor()?,
        [1, 1],
        [true; 2],
        &mut output_policy,
    )?;
    let declaration = owning::admit(f.request(), &mut preparation)?;
    for (active, expected) in [(128, 64), (48, 48)] {
        let mut budget = Budget::new(Limits {
            name_bytes: active,
            ..Limits::default()
        })?;
        crate::lc_omega::correspondence::controls::policy(&declaration, expected, &mut budget)?;
    }
    Ok(())
}
