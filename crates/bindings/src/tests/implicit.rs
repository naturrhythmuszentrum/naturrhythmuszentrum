// SPDX-License-Identifier: AGPL-3.0-only
//! Implicit lowering differentiates checked polynomial coordinates, not named root calls.

use nrz_cas::limits::{Limits, LimitsOverride};
use nrz_cas::proof::Statement;
use nrz_cas::syntax::{AtomicSort, Declaration as Mathematical, Environment, Field, Sort};

use crate::{Context, Declaration, Descriptor, Evidence, ExponentPolicy, Registry};

/// Every inspected partial retains the checked polynomial coordinate while the original root survives.
/// ´test:crate:implicit-root-coordinate-association´
#[test]
fn implicit_root_coordinate_association() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = Context::new(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        Environment {
            declarations: ["x", "t"]
                .map(|key| Mathematical {
                    key: key.into(),
                    display_name: key.into(),
                    sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                })
                .to_vec(),
            ..Environment::default()
        },
    )?;
    for key in ["x", "t"] {
        builder.declare(
            key.into(),
            Declaration::Input {
                descriptor: Descriptor::ratio(),
                nonzero: false,
                evidence: Evidence::default(),
            },
        )?;
    }
    let original = builder.parse("root", "sqrt(x)")?;
    let checked = builder.check_root_sensitivity(
        &[original],
        &["x".into()],
        &["t".into()],
        &LimitsOverride::default(),
    )?;
    let relation = checked
        .jacobian()
        .originals()
        .first()
        .ok_or("missing relation")?;
    let value = checked.originals().first().ok_or("missing root original")?;
    for (entry, key) in checked
        .jacobian()
        .entries()
        .iter()
        .zip(checked.jacobian().coordinates())
    {
        let witness = checked
            .jacobian()
            .witnesses()
            .get(entry.witness_index())
            .ok_or("missing partial proof")?;
        let Statement::DerivativeOn {
            function,
            coordinate,
            derivative,
            ..
        } = &witness.judgment().statement
        else {
            return Err("missing polynomial derivative".into());
        };
        assert_eq!(function, &relation.coordinates);
        assert_eq!(coordinate, key);
        assert_eq!(derivative, entry.expression());
    }
    assert_ne!(relation.coordinates, value.coordinates);
    assert_eq!(relation.snapshot_id(), checked.snapshot_id());
    Ok(())
}
