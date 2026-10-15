// SPDX-License-Identifier: AGPL-3.0-only
//! Check results, original occurrences and coordinates share the frozen owning CAS.

use nrz_cas::{limits::Limits, syntax::Environment};

use crate::{Context, ExponentPolicy, Registry};

/// Private coherent coordinates remain available beside their original occurrence evidence.
/// ´test:crate:frozen-check-coordinate-ownership´
#[test]
fn frozen_check_coordinate_ownership() -> Result<(), Box<dyn std::error::Error>> {
    let builder = Context::new(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        Environment::default(),
    )?;
    let original = builder.parse("repeated", "(1-1)+(1-1)")?;
    let checked = builder.check(&original)?;
    let coordinate = checked.coordinates.clone();
    let frozen = builder.freeze()?;
    frozen.validate_checked(&checked)?;
    let cas = frozen.cas()?;
    let (operation, _) = cas.expression(&coordinate)?;
    assert_eq!(operation, &nrz_cas::syntax::Operator::Add);
    let operands = cas.operands(&coordinate, &mut crate::exact::budget()?)?;
    assert_eq!(operands.len(), 2);
    assert_eq!(operands.first(), operands.get(1));
    Ok(())
}
