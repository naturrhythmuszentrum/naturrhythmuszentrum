// SPDX-License-Identifier: AGPL-3.0-only
//! Compact emptiness certificates are checked through the public proof and fresh replay APIs.

mod limits;

use crate::testing::budgets::proof_budget as budget;
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::proofs::Fixture;
use nrz_cas::domain::{AmbientDomain, Layer, RegionDraft, RegionExpr};
use nrz_cas::proof::{
    ApplicationDraft, Certificate, EmptinessDag, EmptyEvidence, EmptyFact, EmptyFactor,
    EmptyFactorIdentity, EmptyLeaf, EmptyNode, Judgment, Primitive, Statement,
};
use nrz_cas::syntax::{Draft, Operator, PredicateKind};
use nrz_cas::{FaultKind, Handle, InvalidCode, Reference};

/// One independently reconstructed source and its untrusted proof proposal.
type Case = (Fixture, Vec<ApplicationDraft>);

fn region(fixture: &mut Fixture, expression: RegionExpr<Reference>) -> TestResult<Handle> {
    let header = fixture.cas.context_header(&fixture.context)?;
    admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression,
        }),
        &mut budget()?,
    )
}

fn guard(
    fixture: &mut Fixture,
    base: &Handle,
    kind: PredicateKind,
    operands: &[Handle],
) -> TestResult<Handle> {
    let header = fixture.cas.context_header(&fixture.context)?;
    let predicate = admit(
        &mut fixture.cas,
        Draft::Condition {
            header: Reference::Existing(header),
            scope: Vec::new(),
            kind,
            arguments: operands.iter().cloned().map(Reference::Existing).collect(),
        },
        &mut budget()?,
    )?;
    region(
        fixture,
        RegionExpr::GuardTrue {
            base: Reference::Existing(base.clone()),
            predicate: Reference::Existing(predicate),
        },
    )
}

fn sign(fixture: &mut Fixture, subject: &Handle, kind: PredicateKind) -> TestResult<EmptyNode> {
    let domain = fixture.domain.clone();
    let zero = fixture.zero.clone();
    let positive = guard(fixture, &domain, kind, std::slice::from_ref(subject))?;
    let empty = guard(
        fixture,
        &positive,
        PredicateKind::Equal,
        &[subject.clone(), zero],
    )?;
    Ok(EmptyNode {
        region: empty,
        evidence: EmptyEvidence::Leaf(Box::new(EmptyLeaf::Sign {
            subject: subject.clone(),
            nonzero: EmptyFact::Guard(positive),
        })),
    })
}

fn application(
    fixture: &Fixture,
    dag: EmptinessDag,
    premises: Vec<u32>,
) -> TestResult<ApplicationDraft> {
    Ok(ApplicationDraft {
        rule: Primitive::RegionDecide,
        context: fixture.context.clone(),
        conclusion: Statement::Empty {
            region: dag.nodes.last().ok_or("empty DAG")?.region.clone(),
        },
        certificate: Certificate::Emptiness(Box::new(dag)),
        premises,
        substitutions: Vec::new(),
    })
}

fn sign_case() -> TestResult<Case> {
    let mut fixture = Fixture::new()?;
    let subject = fixture.x.clone();
    let node = sign(&mut fixture, &subject, PredicateKind::Positive)?;
    let step = application(&fixture, EmptinessDag { nodes: vec![node] }, Vec::new())?;
    Ok((fixture, vec![step]))
}

fn premise_case() -> TestResult<Case> {
    let mut fixture = Fixture::new()?;
    let subject = fixture.x.clone();
    fixture.assume("positive x", PredicateKind::Positive, &subject)?;
    let header = fixture.cas.context_header(&fixture.context)?;
    let predicate = admit(
        &mut fixture.cas,
        Draft::Condition {
            header: Reference::Existing(header),
            scope: Vec::new(),
            kind: PredicateKind::Positive,
            arguments: vec![Reference::Existing(subject.clone())],
        },
        &mut budget()?,
    )?;
    let fact = ApplicationDraft {
        rule: Primitive::ContextHypothesis,
        context: fixture.context.clone(),
        conclusion: Statement::PredicateFact {
            region: fixture.domain.clone(),
            predicate,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Hypothesis(nrz_cas::domain::HypothesisRef {
            context: fixture.context.clone(),
            slot: 0,
        }),
    };
    let base = fixture.domain.clone();
    let zero = fixture.zero.clone();
    let region = guard(
        &mut fixture,
        &base,
        PredicateKind::Equal,
        &[subject.clone(), zero],
    )?;
    let node = EmptyNode {
        region,
        evidence: EmptyEvidence::Leaf(Box::new(EmptyLeaf::Sign {
            subject,
            nonzero: EmptyFact::Premise(0),
        })),
    };
    let root = application(&fixture, EmptinessDag { nodes: vec![node] }, vec![0])?;
    Ok((fixture, vec![fact, root]))
}

fn split_case() -> TestResult<Case> {
    let mut fixture = Fixture::new()?;
    let x = fixture.x.clone();
    let y = fixture.y.clone();
    let first = sign(&mut fixture, &x, PredicateKind::Positive)?;
    let second = sign(&mut fixture, &y, PredicateKind::NonZero)?;
    let members = vec![first.region.clone(), second.region.clone()];
    let domain = region(
        &mut fixture,
        RegionExpr::Union(members.iter().cloned().map(Reference::Existing).collect()),
    )?;
    let cover = ApplicationDraft {
        rule: Primitive::RegionConstruct,
        context: fixture.context.clone(),
        conclusion: Statement::Cover {
            domain: domain.clone(),
            members,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    let root = EmptyNode {
        region: domain,
        evidence: EmptyEvidence::Split {
            cover: 0,
            children: vec![0, 1],
        },
    };
    let step = application(
        &fixture,
        EmptinessDag {
            nodes: vec![first, second, root],
        },
        vec![0],
    )?;
    Ok((fixture, vec![cover, step]))
}

fn lemma_case() -> TestResult<Case> {
    let (fixture, mut steps) = sign_case()?;
    let Statement::Empty { region } = &steps.first().ok_or("sign step")?.conclusion else {
        return Err("empty claim".into());
    };
    let node = EmptyNode {
        region: region.clone(),
        evidence: EmptyEvidence::Lemma { premise: 0 },
    };
    steps.push(application(
        &fixture,
        EmptinessDag { nodes: vec![node] },
        vec![0],
    )?);
    Ok((fixture, steps))
}

fn factor_case() -> TestResult<Case> {
    let mut fixture = Fixture::new()?;
    let subject = fixture.term(Operator::Mul, &[fixture.x.clone(), fixture.y.clone()])?;
    let first = fixture.term(Operator::Mul, &[fixture.one.clone(), fixture.x.clone()])?;
    let product = fixture.term(Operator::Mul, &[first, fixture.y.clone()])?;
    let identity = crate::testing::witness::equality(
        &fixture.context,
        &fixture.domain,
        Primitive::RationalIdentity,
        &subject,
        &product,
    );
    let domain = fixture.domain.clone();
    let operands = [
        fixture.x.clone(),
        fixture.y.clone(),
        fixture.one.clone(),
        fixture.zero.clone(),
    ];
    let x = guard(
        &mut fixture,
        &domain,
        PredicateKind::Positive,
        std::slice::from_ref(operands.first().ok_or("x")?),
    )?;
    let y = guard(
        &mut fixture,
        &x,
        PredicateKind::NonZero,
        std::slice::from_ref(operands.get(1).ok_or("y")?),
    )?;
    let scalar = guard(
        &mut fixture,
        &y,
        PredicateKind::NonZero,
        std::slice::from_ref(operands.get(2).ok_or("one")?),
    )?;
    let empty = guard(
        &mut fixture,
        &scalar,
        PredicateKind::Equal,
        &[subject.clone(), operands.get(3).ok_or("zero")?.clone()],
    )?;
    let leaf = EmptyFactorIdentity {
        subject,
        product,
        scalar: fixture.one.clone(),
        scalar_nonzero: EmptyFact::Guard(scalar),
        identity: 0,
        factors: vec![
            EmptyFactor {
                subject: fixture.x.clone(),
                multiplicity: 1,
                nonzero: EmptyFact::Guard(x),
            },
            EmptyFactor {
                subject: fixture.y.clone(),
                multiplicity: 1,
                nonzero: EmptyFact::Guard(y),
            },
        ],
    };
    let node = EmptyNode {
        region: empty,
        evidence: EmptyEvidence::Leaf(Box::new(EmptyLeaf::FactorIdentity(Box::new(leaf)))),
    };
    let step = application(&fixture, EmptinessDag { nodes: vec![node] }, vec![0])?;
    Ok((fixture, vec![identity, step]))
}

fn certificate(steps: &mut [ApplicationDraft]) -> TestResult<&mut EmptinessDag> {
    let Certificate::Emptiness(dag) = &mut steps.last_mut().ok_or("last step")?.certificate else {
        return Err("empty certificate".into());
    };
    Ok(dag)
}

fn refused(fixture: &mut Fixture, steps: &[ApplicationDraft]) -> TestResult {
    let before = fixture.cas.counts();
    let fault = fixture
        .cas
        .check_proof(&fixture.context, steps, &mut budget()?)
        .err()
        .ok_or("false emptiness accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Sign, factor, split and lemma records round-trip through fresh independently reconstructed judgments.
/// ´test:integration:emptiness-node-wire-roundtrips´
#[test]
fn emptiness_node_wire_roundtrips() -> TestResult {
    let cases: [fn() -> TestResult<Case>; 5] =
        [sign_case, premise_case, factor_case, split_case, lemma_case];
    for build in cases {
        let (mut source, steps) = build()?;
        let witness = source
            .cas
            .check_proof(&source.context, &steps, &mut budget()?)?;
        let bytes = source.cas.export_witness(&witness, &mut budget()?)?;
        assert_eq!(bytes.get(..8), Some(b"NRZCAS05".as_slice()));
        let (mut target, proposed) = build()?;
        let expected = Judgment {
            context: target.context.clone(),
            layer: Layer::Logic,
            space: None,
            statement: proposed.last().ok_or("expected root")?.conclusion.clone(),
            residuals: Vec::new(),
        };
        let replayed = target.cas.replay(&bytes, &expected, &mut budget()?)?;
        assert!(replayed.is_discharged());
        assert_eq!(target.cas.export_witness(&replayed, &mut budget()?)?, bytes);
        let mut old = bytes;
        old.get_mut(..8)
            .ok_or("magic")?
            .copy_from_slice(b"NRZCAS04");
        let fault = target
            .cas
            .replay(&old, &expected, &mut budget()?)
            .err()
            .ok_or("legacy accepted")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::UnsupportedVersion)
        );
    }
    Ok(())
}

/// Nonnegative signs, another subject and a missing zero guard cannot exclude zero.
/// ´test:integration:emptiness-sign-mutations-refuse´
#[test]
fn emptiness_sign_mutations_refuse() -> TestResult {
    let mut fixture = Fixture::new()?;
    let subject = fixture.x.clone();
    let weak = sign(&mut fixture, &subject, PredicateKind::NonNegative)?;
    let step = application(&fixture, EmptinessDag { nodes: vec![weak] }, Vec::new())?;
    refused(&mut fixture, &[step])?;
    let (mut fixture, steps) = sign_case()?;
    for change in [0, 1] {
        let mut altered = steps.clone();
        let node = certificate(&mut altered)?
            .nodes
            .first_mut()
            .ok_or("sign node")?;
        let EmptyEvidence::Leaf(leaf) = &mut node.evidence else {
            return Err("leaf".into());
        };
        let EmptyLeaf::Sign { subject, nonzero } = leaf.as_mut() else {
            return Err("sign".into());
        };
        if change == 0 {
            subject.clone_from(&fixture.y);
        } else {
            *nonzero = EmptyFact::Guard(fixture.domain.clone());
        }
        refused(&mut fixture, &altered)?;
    }
    Ok(())
}

/// Every full product factor, nonzero scalar and positive multiplicity is independently mandatory.
/// ´test:integration:emptiness-factor-mutations-refuse´
#[test]
fn emptiness_factor_mutations_refuse() -> TestResult {
    let (mut fixture, steps) = factor_case()?;
    for change in 0..5 {
        let mut altered = steps.clone();
        let EmptyEvidence::Leaf(leaf) = &mut certificate(&mut altered)?
            .nodes
            .first_mut()
            .ok_or("factor node")?
            .evidence
        else {
            return Err("leaf".into());
        };
        let EmptyLeaf::FactorIdentity(value) = leaf.as_mut() else {
            return Err("factor identity".into());
        };
        match change {
            0 => {
                value.factors.pop().ok_or("factor")?;
            }
            1 => value.factors.first_mut().ok_or("factor")?.multiplicity = 0,
            2 => value
                .factors
                .first_mut()
                .ok_or("factor")?
                .subject
                .clone_from(&fixture.y),
            3 => value.scalar.clone_from(&fixture.zero),
            _ => value.identity = 1,
        }
        refused(&mut fixture, &altered)?;
    }
    Ok(())
}

/// Missing cells, forward edges and dead nodes refuse; child order is immaterial to a cover.
/// ´test:integration:emptiness-split-mutations-refuse´
#[test]
fn emptiness_split_mutations_refuse() -> TestResult {
    let (mut fixture, steps) = split_case()?;
    for change in 0..3 {
        let mut altered = steps.clone();
        let dag = certificate(&mut altered)?;
        let EmptyEvidence::Split { children, .. } =
            &mut dag.nodes.last_mut().ok_or("root")?.evidence
        else {
            return Err("split".into());
        };
        match change {
            0 => {
                children.pop().ok_or("child")?;
            }
            1 => *children.first_mut().ok_or("child")? = 2,
            _ => {
                *children = vec![0, 0];
            }
        }
        refused(&mut fixture, &altered)?;
    }
    let mut permuted = steps;
    let EmptyEvidence::Split { children, .. } = &mut certificate(&mut permuted)?
        .nodes
        .last_mut()
        .ok_or("root")?
        .evidence
    else {
        return Err("split".into());
    };
    children.reverse();
    assert!(
        fixture
            .cas
            .check_proof(&fixture.context, &permuted, &mut budget()?)?
            .is_discharged()
    );
    Ok(())
}

/// A lemma cannot replace an empty-region theorem for a different exact region.
/// ´test:integration:emptiness-lemma-region-mutation-refuses´
#[test]
fn emptiness_lemma_region_mutation_refuses() -> TestResult {
    let (mut fixture, mut steps) = lemma_case()?;
    let dag = certificate(&mut steps)?;
    dag.nodes
        .first_mut()
        .ok_or("lemma")?
        .region
        .clone_from(&fixture.domain);
    steps.last_mut().ok_or("root")?.conclusion = Statement::Empty {
        region: fixture.domain.clone(),
    };
    refused(&mut fixture, &steps)
}

fn cover(fixture: &Fixture, domain: &Handle, members: Vec<Handle>) -> ApplicationDraft {
    ApplicationDraft {
        rule: Primitive::RegionConstruct,
        context: fixture.context.clone(),
        conclusion: Statement::Cover {
            domain: domain.clone(),
            members,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    }
}

fn trimming_case() -> TestResult<(Fixture, Vec<ApplicationDraft>, EmptinessDag)> {
    let (mut fixture, mut steps) = sign_case()?;
    let leaf = certificate(&mut steps)?
        .nodes
        .first()
        .ok_or("leaf")?
        .clone();
    let inner = region(
        &mut fixture,
        RegionExpr::Union(vec![Reference::Existing(leaf.region.clone())]),
    )?;
    let outer = region(
        &mut fixture,
        RegionExpr::Union(vec![
            Reference::Existing(leaf.region.clone()),
            Reference::Existing(inner.clone()),
        ]),
    )?;
    let covers = vec![
        cover(&fixture, &inner, vec![leaf.region.clone()]),
        cover(&fixture, &outer, vec![leaf.region.clone(), inner.clone()]),
    ];
    let nodes = vec![
        EmptyNode {
            region: fixture.domain.clone(),
            evidence: EmptyEvidence::Lemma { premise: 999 },
        },
        leaf.clone(),
        leaf,
        EmptyNode {
            region: inner,
            evidence: EmptyEvidence::Split {
                cover: 0,
                children: vec![2],
            },
        },
        EmptyNode {
            region: outer,
            evidence: EmptyEvidence::Split {
                cover: 1,
                children: vec![1, 3],
            },
        },
    ];
    Ok((fixture, covers, EmptinessDag { nodes }))
}

/// Dead exploration vanishes and two distinct cover branches cite one shared exact subproblem.
/// ´test:integration:emptiness-trimming-shares-a-graph´
#[test]
fn emptiness_trimming_shares_a_graph() -> TestResult {
    let (mut fixture, mut steps, input) = trimming_case()?;
    let before = fixture.cas.counts();
    let trimmed = fixture.cas.trim_emptiness(&input, &mut budget()?)?;
    assert_eq!(fixture.cas.counts(), before);
    assert_eq!(trimmed.nodes.len(), 3);
    assert!(
        matches!(&trimmed.nodes.get(1).ok_or("inner")?.evidence, EmptyEvidence::Split { children, .. } if children == &[0])
    );
    assert!(
        matches!(&trimmed.nodes.last().ok_or("root")?.evidence, EmptyEvidence::Split { children, .. } if children == &[0, 1])
    );
    let mut untrimmed = steps.clone();
    untrimmed.push(application(&fixture, input, vec![0, 1])?);
    refused(&mut fixture, &untrimmed)?;
    steps.push(application(&fixture, trimmed, vec![0, 1])?);
    assert!(
        fixture
            .cas
            .check_proof(&fixture.context, &steps, &mut budget()?)?
            .is_discharged()
    );
    Ok(())
}

/// Construction, checking, replay, export and retained payload have separate measured ledgers.
/// ´test:integration:emptiness-resource-measurements´
#[test]
fn emptiness_resource_measurements() -> TestResult {
    let (mut fixture, mut steps, input) = trimming_case()?;
    let mut generation = budget()?;
    let trimmed = fixture.cas.trim_emptiness(&input, &mut generation)?;
    steps.push(application(&fixture, trimmed, vec![0, 1])?);
    let before = fixture.cas.counts().payload_bytes;
    let mut checking = budget()?;
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &steps, &mut checking)?;
    let mut export = budget()?;
    let bytes = fixture.cas.export_witness(&witness, &mut export)?;
    let (mut target, _, input) = trimming_case()?;
    let expected = Judgment {
        context: target.context.clone(),
        layer: Layer::Logic,
        space: None,
        statement: Statement::Empty {
            region: input.nodes.last().ok_or("root")?.region.clone(),
        },
        residuals: Vec::new(),
    };
    let mut replay = budget()?;
    target.cas.replay(&bytes, &expected, &mut replay)?;
    assert_eq!(
        [
            generation.usage().work,
            checking.usage().work,
            replay.usage().work,
            export.usage().work,
            u64::try_from(bytes.len())?,
            fixture.cas.counts().payload_bytes - before
        ],
        [5_002, 42_989, 72_471, 789, 789, 3_178]
    );
    Ok(())
}
