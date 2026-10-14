// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit typed space links preserve exact declaration identity.

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::case_spaces::{axis, space};
use crate::testing::common::TestResult;
use nrz_cas::syntax::{Draft, ExpressionDraft, Operator, Sort};
use nrz_cas::{Cas, Reference};

/// A supplied space is checked against a leaf declaration, not inferred from matching field names.
/// ´test:integration:explicit-expression-space-identity´
#[test]
fn explicit_expression_space_identity() -> TestResult {
    let mut cas = Cas::new(budget()?.limits().clone())?;
    let (header, domain) = axis(&mut cas, "x")?;
    let (expected, extended, variable) = space(&mut cas, &header, &domain, "x")?;
    let (other_header, other_domain) = axis(&mut cas, "y")?;
    let (other, _, _) = space(&mut cas, &other_header, &other_domain, "y")?;
    let mut draft = ExpressionDraft {
        space: Some(Reference::Existing(expected)),
        operator: Operator::Free("phi".into()),
        sort: Sort::Test,
        header: Reference::Existing(extended),
        scope: Vec::new(),
        binding: None,
        operands: Vec::new(),
    };
    let same = crate::testing::builders::admit(
        &mut cas,
        Draft::Expression(draft.clone()),
        &mut budget()?,
    )?;
    assert_eq!(same, variable);
    draft.space = Some(Reference::Existing(other));
    assert!(
        cas.admit(&[Draft::Expression(draft)], &mut budget()?)
            .is_err()
    );
    Ok(())
}
