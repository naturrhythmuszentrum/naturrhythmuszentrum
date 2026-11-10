// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical integer admission and typed dependency roles of the half-Pi statement.

use crate::encoding::Encoder;
use crate::proof_model::Statement;
use crate::testing::common::{TestResult, default_budget};
use crate::testing::half_pi::prepared;
use crate::wire_decode::Decoded;
use crate::wire_read::Reader;
use crate::{FaultKind, InvalidCode, Location};

/// A changed manifest receives a new logical identity even with unchanged weights.
/// ´test:crate:half-pi-policy-version´
#[test]
fn half_pi_policy_version() -> TestResult {
    let policy = crate::cost::CostPolicy::default();
    assert_eq!(policy.version, 20);
    assert_eq!(policy.weights.len(), 100);
    assert_eq!(
        crate::rule::PRIMITIVES
            .iter()
            .filter(|primitive| !matches!(primitive, crate::rule::Primitive::Solve(_)))
            .count(),
        79
    );
    policy.validate(&mut default_budget()?)?;
    let mut old = policy.clone();
    old.version = 17;
    let fault = old
        .validate(&mut default_budget()?)
        .err()
        .ok_or("old logical identity accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Policy));
    let mut changed = policy;
    *changed.weights.first_mut().ok_or("weight")? += 1;
    let fault = changed
        .validate(&mut default_budget()?)
        .err()
        .ok_or("changed logical table accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Policy));
    Ok(())
}

/// Statement value bytes have no graph identity; original operand, context and domain do.
/// ´test:crate:half-pi-codec´
#[test]
fn half_pi_codec() -> TestResult {
    let f = prepared("3", "3/2", false, false)?;
    let statement = &f.expected.statement;
    let objects = crate::statement::objects(statement);
    let records: Vec<_> = objects
        .iter()
        .map(|value| Decoded::Object((*value).clone()))
        .collect();
    let mut budget = default_budget()?;
    let mut out = Encoder::new(&mut budget, Location::new("half-Pi codec fixture"));
    crate::statement::encode(statement, &mut out, |handle, out| {
        let index = objects
            .iter()
            .position(|value| *value == handle)
            .ok_or_else(crate::wire_read::invalid)?;
        crate::wire::index(
            u32::try_from(index).map_err(|_| crate::wire_read::invalid())?,
            out,
        )
    })?;
    let bytes = out.finish();
    assert_eq!(
        bytes,
        [
            31, 0, 0, 0, 0, 0, 1, 0, 0, 0, 3, 1, 0, 0, 0, 1, 1, 0, 0, 0, 2, 0, 0, 0
        ]
    );
    let mut budget = default_budget()?;
    let mut reader = Reader::new(&bytes, &mut budget)?;
    assert_eq!(
        crate::wire_statement::statement(&mut reader, &records)?,
        *statement
    );
    reader.finished()?;
    assert_eq!(objects.len(), 3);
    assert!(matches!(
        crate::statement::edge_role(statement, 0),
        crate::graph::Edge::OfFunction
    ));
    assert!(matches!(
        crate::statement::edge_role(statement, 1),
        crate::graph::Edge::InContext
    ));
    assert!(matches!(
        crate::statement::edge_role(statement, 2),
        crate::graph::Edge::ArgumentRegion(0)
    ));
    let counts = f.cas.counts();
    let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
    for (offset, value) in [(5, 2), (10, 0), (15, 2), (15, 3)] {
        let mut changed = bytes.clone();
        *changed.get_mut(offset).ok_or("integer byte")? = value;
        let mut budget = default_budget()?;
        let mut reader = Reader::new(&changed, &mut budget)?;
        let fault = crate::wire_statement::statement(&mut reader, &records)
            .err()
            .ok_or("noncanonical integer")?;
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Encoding));
        assert_eq!(f.cas.counts(), counts);
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    assert!(matches!(statement, Statement::HalfPiWitness { .. }));
    Ok(())
}
