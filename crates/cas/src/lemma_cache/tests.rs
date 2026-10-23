// SPDX-License-Identifier: AGPL-3.0-only
//! Each component of an anonymous lemma key has its own staleness regression.

use super::AnonymousLemmaCache;
use crate::domain_model::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::graph::{AssumptionDraft, Cas, Draft, Handle, Reference};
use crate::model::{Declaration, Environment, Operator, PredicateKind};
use crate::proof_model::{
    ApplicationDraft, Certificate, DomainContract, Statement, VerifiedWitness,
};
use crate::rule::Primitive;
use crate::testing::common::{TestResult, real};
use crate::testing::native::{admit, budget, predicate, term};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Change {
    None,
    Relation,
    Declarations,
    Context,
    Stratum,
    Premises,
    Representation,
}

fn signature(cas: &mut Cas, change: Change) -> TestResult<Handle> {
    let mut declarations = vec![Declaration {
        key: "x".into(),
        display_name: "x".into(),
        sort: real(),
    }];
    if change == Change::Declarations {
        declarations.push(Declaration {
            key: "y".into(),
            display_name: "y".into(),
            sort: real(),
        });
    }
    let header = admit(
        cas,
        Draft::Header(HeaderDraft::Core(Environment {
            declarations,
            ..Environment::default()
        })),
    )?;
    Ok(header)
}

fn frame(cas: &mut Cas, change: Change) -> TestResult<(Handle, Handle, Handle)> {
    let header = signature(cas, change)?;
    let subject = term(cas, &header, Operator::Free("x".into()), &[])?;
    let positive = predicate(
        cas,
        &header,
        PredicateKind::Positive,
        std::slice::from_ref(&subject),
    )?;
    let mut region = admit(
        cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Real("x".into()),
            expression: RegionExpr::Ambient,
        }),
    )?;
    if change == Change::Stratum {
        region = admit(
            cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Real("x".into()),
                expression: RegionExpr::GuardTrue {
                    base: Reference::Existing(region),
                    predicate: Reference::Existing(positive.clone()),
                },
            }),
        )?;
    }
    let assumptions = if change == Change::Context {
        vec![AssumptionDraft {
            key: "positive".into(),
            condition: Reference::Existing(positive),
        }]
    } else {
        Vec::new()
    };
    let context = admit(
        cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header.clone()),
            admissible: Reference::Existing(region.clone()),
            assumptions,
        }),
    )?;
    let subject = if change == Change::Representation {
        let zero = term(
            cas,
            &header,
            Operator::Rational(crate::Rational::zero()),
            &[],
        )?;
        term(cas, &header, Operator::Add, &[subject, zero])?
    } else {
        subject
    };
    Ok((context, region, subject))
}

fn proof(cas: &mut Cas, change: Change) -> TestResult<VerifiedWitness> {
    let (context, region, subject) = frame(cas, change)?;
    let conclusion = if change == Change::Relation {
        Statement::Subset {
            subset: region.clone(),
            superset: region,
        }
    } else {
        Statement::Equality {
            left: subject.clone(),
            right: subject,
            domain: region,
            domain_contract: DomainContract::DefinedAndEqual,
        }
    };
    let mut steps = vec![ApplicationDraft {
        rule: if change == Change::Relation {
            Primitive::RegionConstruct
        } else {
            Primitive::Reflexivity
        },
        context: context.clone(),
        conclusion,
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    }];
    if change == Change::Premises {
        let mut last = steps.first().ok_or("reflexivity")?.clone();
        last.rule = Primitive::Symmetry;
        last.premises.push(0);
        steps.push(last);
    }
    Ok(cas.check_proof(&context, &steps, &mut budget()?)?)
}

fn distinct(change: Change) -> TestResult {
    let mut cas = Cas::new(budget()?.limits().clone())?;
    let first = proof(&mut cas, Change::None)?;
    let changed = proof(&mut cas, change)?;
    let mut cache = AnonymousLemmaCache::new();
    cache.replay(&mut cas, &first.bytes, first.judgment(), &mut budget()?)?;
    let mut ledger = budget()?;
    let (result, rows) = crate::testing::work_trace::measured(|| {
        cache.replay(&mut cas, &changed.bytes, changed.judgment(), &mut ledger)
    })?;
    assert_eq!(result?.judgment(), changed.judgment());
    assert!(rows.iter().any(|row| row.rule != "replay"));
    assert_eq!(cache.len(), 2);
    Ok(())
}

/// Changed relations cannot reuse a theorem with equal source syntax.
/// ´test:unit:lemma-cache-relation-is-stale´
#[test]
fn lemma_cache_relation_is_stale() -> TestResult {
    distinct(Change::Relation)
}

/// The complete immutable declaration header participates in the key.
/// ´test:unit:lemma-cache-declarations-are-stale´
#[test]
fn lemma_cache_declarations_are_stale() -> TestResult {
    distinct(Change::Declarations)
}

/// Even unused root assumptions preserve the complete original context identity.
/// ´test:unit:lemma-cache-context-is-stale´
#[test]
fn lemma_cache_context_is_stale() -> TestResult {
    distinct(Change::Context)
}

/// A true guard stratum cannot inherit an ambient lemma cache hit.
/// ´test:unit:lemma-cache-stratum-is-stale´
#[test]
fn lemma_cache_stratum_is_stale() -> TestResult {
    distinct(Change::Stratum)
}

/// A new ordered proof premise is a new key even for the identical final judgment.
/// ´test:unit:lemma-cache-premises-are-stale´
#[test]
fn lemma_cache_premises_are_stale() -> TestResult {
    distinct(Change::Premises)
}

/// Equal-valued expressions with different exact representations have different keys.
/// ´test:unit:lemma-cache-representation-is-stale´
#[test]
fn lemma_cache_representation_is_stale() -> TestResult {
    distinct(Change::Representation)
}

/// An altered primitive revision misses the cache and reaches the ordinary version refusal.
/// ´test:unit:lemma-cache-revision-is-stale´
#[test]
fn lemma_cache_revision_is_stale() -> TestResult {
    let mut cas = Cas::new(budget()?.limits().clone())?;
    let witness = proof(&mut cas, Change::None)?;
    let mut cache = AnonymousLemmaCache::new();
    cache.replay(&mut cas, &witness.bytes, witness.judgment(), &mut budget()?)?;
    let mut bytes = witness.bytes.clone();
    let name_length = u32::from_le_bytes(
        bytes
            .get(12..16)
            .ok_or("manifest name length")?
            .try_into()?,
    );
    let offset = 16 + usize::try_from(name_length)?;
    bytes
        .get_mut(offset..offset + 4)
        .ok_or("manifest revision")?
        .copy_from_slice(&99_u32.to_le_bytes());
    let before = cas.counts();
    let error = cache
        .replay(&mut cas, &bytes, witness.judgment(), &mut budget()?)
        .err()
        .ok_or("stale revision hit")?;
    assert_eq!(
        error.kind,
        crate::FaultKind::Invalid(crate::InvalidCode::UnsupportedVersion)
    );
    assert_eq!(cache.len(), 1);
    assert_eq!(cas.counts(), before);
    Ok(())
}

/// The same key is checked once across independent ledgers and publishes nothing on a hit.
/// ´test:unit:lemma-cache-checks-once-across-retries´
#[test]
fn lemma_cache_checks_once_across_retries() -> TestResult {
    let mut cas = Cas::new(budget()?.limits().clone())?;
    let witness = proof(&mut cas, Change::None)?;
    let mut cache = AnonymousLemmaCache::new();
    for warm in [false, true, true] {
        let before = cas.counts();
        let mut ledger = budget()?;
        let (result, rows) = crate::testing::work_trace::measured(|| {
            cache.replay(&mut cas, &witness.bytes, witness.judgment(), &mut ledger)
        })?;
        assert_eq!(result?.judgment(), witness.judgment());
        assert_eq!(rows.iter().all(|row| row.rule == "replay"), warm);
        assert_eq!(cache.len(), 1);
        if warm {
            assert_eq!(cas.counts(), before);
        }
    }
    Ok(())
}

/// Portable key equality cannot return another store's local proof handle.
/// ´test:unit:lemma-cache-authenticates-receiving-store´
#[test]
fn lemma_cache_authenticates_receiving_store() -> TestResult {
    let mut first = Cas::new(budget()?.limits().clone())?;
    let mut second = Cas::new(budget()?.limits().clone())?;
    let original = proof(&mut first, Change::None)?;
    let imported = proof(&mut second, Change::None)?;
    assert_eq!(original.bytes, imported.bytes);
    let mut cache = AnonymousLemmaCache::new();
    cache.replay(
        &mut first,
        &original.bytes,
        original.judgment(),
        &mut budget()?,
    )?;
    let result = cache.replay(
        &mut second,
        &original.bytes,
        imported.judgment(),
        &mut budget()?,
    )?;
    second.checked(result.proof())?;
    assert!(first.checked(result.proof()).is_err());
    assert_eq!(cache.len(), 2);
    Ok(())
}

/// The named cache schema and its full premise shape occur exactly once in both guide sources.
/// ´test:unit:lemma-cache-schema-is-documented´
#[test]
fn lemma_cache_schema_is_documented() {
    let schema = super::ANONYMOUS_LEMMA_SCHEMA;
    let expected = format!(
        "Formal schema `{}`, version {}. {} Required premises: {}",
        schema.name, schema.version, schema.statement, schema.premises
    );
    let head = format!("**Theorem (Anonymous lemma reuse)** · `{}`", schema.label);
    for document in [
        include_str!("../../README.md"),
        include_str!("../../../../templates/crates/cas/README.md.j2"),
    ] {
        assert_eq!(document.lines().filter(|line| *line == expected).count(), 1);
        assert_eq!(document.lines().filter(|line| *line == head).count(), 1);
    }
}

mod limits;
