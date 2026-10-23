// SPDX-License-Identifier: AGPL-3.0-only
//! Sealed witness caches must agree with the authenticated retained proof root.

mod expanded;

use crate::budget::Limits;
use crate::domain_model::{
    AmbientDomain, ContextDraft, HeaderDraft, HypothesisRef, RegionDraft, RegionExpr,
};
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference};
use crate::model::{Declaration, Environment, Operator};
use crate::proof_model::{DomainContract, Statement, VerifiedWitness};
use crate::readings::AffineMap;
use crate::testing::common::{TestResult, default_budget, real};
use crate::{FaultKind, InvalidCode};

fn declaration(key: &str) -> Draft {
    Draft::Expression(ExpressionDraft {
        operator: Operator::Free(key.into()),
        sort: real(),
        header: Reference::Local(0),
        scope: Vec::new(),
        binding: None,
        space: None,
        operands: Vec::new(),
    })
}

fn fixture() -> TestResult<(Cas, Handle, VerifiedWitness)> {
    let mut cas = Cas::new(Limits::default())?;
    let environment = Environment {
        declarations: ["r", "z"]
            .map(|key| Declaration {
                key: key.into(),
                display_name: key.into(),
                sort: real(),
            })
            .into(),
        ..Environment::default()
    };
    let values = cas.admit(
        &[
            Draft::Header(HeaderDraft::Core(environment)),
            Draft::Region(RegionDraft {
                header: Reference::Local(0),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Ambient,
            }),
            Draft::Context(ContextDraft::Root {
                header: Reference::Local(0),
                admissible: Reference::Local(1),
                assumptions: Vec::new(),
            }),
            declaration("z"),
            declaration("r"),
        ],
        &mut default_budget()?,
    )?;
    let context = values.get(2).ok_or("context")?;
    let source = values.get(3).ok_or("source")?;
    let symbol = values.get(4).ok_or("symbol")?;
    let reading = cas.define_reading(
        symbol,
        source,
        &AffineMap::identity(1, &mut default_budget()?)?,
        &mut default_budget()?,
    )?;
    let witness =
        cas.prove_reading_equality(context, &reading, &reading, &mut default_budget()?)?;
    Ok((cas, reading, witness))
}

/// Cached bytes, statement strength and hypothesis provenance cannot be substituted independently.
/// ´test:crate:dependency-witness-tampering-refuses´
#[test]
fn dependency_witness_tampering_refuses() -> TestResult {
    let (cas, _, witness) = fixture()?;
    let before = cas.counts();
    for mutation in 0..3 {
        let mut changed = witness.clone();
        match mutation {
            0 => *changed.bytes.last_mut().ok_or("witness byte")? ^= 1,
            1 => {
                let Statement::Equality {
                    domain_contract, ..
                } = &mut changed.judgment.statement
                else {
                    return Err("equality".into());
                };
                *domain_contract = DomainContract::SameDefinedDomain;
            }
            2 => changed.hypotheses.push(HypothesisRef {
                context: changed.judgment.context.clone(),
                slot: 0,
            }),
            _ => return Err("mutation".into()),
        }
        let result = cas.witness_dependencies(&changed, &mut default_budget()?);
        assert_eq!(
            result.err().map(|fault| fault.kind),
            Some(FaultKind::Invalid(InvalidCode::DependencyWitness))
        );
        assert_eq!(cas.counts(), before);
    }
    Ok(())
}

/// A nonproof root and a foreign proof root cannot authorize an inventory through cached bytes.
/// ´test:crate:dependency-witness-root-substitution-refuses´
#[test]
fn dependency_witness_root_substitution_refuses() -> TestResult {
    let (cas, reading, witness) = fixture()?;
    let mut changed = witness;
    changed.proof = reading;
    let result = cas.witness_dependencies(&changed, &mut default_budget()?);
    assert_eq!(
        result.err().map(|fault| fault.kind),
        Some(FaultKind::Invalid(InvalidCode::UnsupportedFragment))
    );
    let (_, _, foreign) = fixture()?;
    changed.proof = foreign.proof;
    let result = cas.witness_dependencies(&changed, &mut default_budget()?);
    assert_eq!(
        result.err().map(|fault| fault.kind),
        Some(FaultKind::Invalid(InvalidCode::ForeignHandle))
    );
    Ok(())
}
