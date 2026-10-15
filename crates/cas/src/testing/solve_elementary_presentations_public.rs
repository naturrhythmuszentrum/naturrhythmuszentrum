// SPDX-License-Identifier: AGPL-3.0-only
//! Independent recursively closed presentation fixtures using only public proof admission.

use super::{BinomialFixture, Fixture, admit};
use crate::test_api::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
use crate::test_api::proof::{ApplicationDraft, Certificate, ConstructorLaw, Primitive, Statement};
use crate::test_api::solving::{
    CertificateProductCompleteness, CertificateTotalPredicateSplit, PredicateSplitMode, RootFactor,
    SolutionSetData, SolveCertificate, SolveDraft, SolveForm, SolveSchema, SolveStatement,
    SolveView, StatementCoverage,
};
use crate::test_api::syntax::Draft;
use crate::test_api::{Handle, Reference};
use crate::testing::common::TestResult;

/// Exact set identity and the separately checked completeness and representation theorems.
struct Node {
    /// Unchanged set at this tree node.
    set: Handle,
    /// Earlier coverage application index.
    coverage: u32,
    /// Earlier presentation application index.
    presentation: u32,
}

/// Append one draft, returning its actual absolute proof index.
fn push(steps: &mut Vec<ApplicationDraft>, draft: ApplicationDraft) -> TestResult<u32> {
    let index = u32::try_from(steps.len())?;
    steps.push(draft);
    Ok(index)
}

/// The equation x=0 has one explicitly represented root in every exact leaf stratum.
fn leaf(
    base: &mut Fixture,
    problem: &Handle,
    steps: &mut Vec<ApplicationDraft>,
) -> TestResult<Node> {
    let unknown = base.free("x")?;
    let zero = base.integer(0, false)?;
    let one = base.integer(1, false)?;
    let set = base.finite(problem, std::slice::from_ref(&zero))?;
    let algebra = base.algebra.clone();
    let map = base.scalar_map(&algebra, &[])?;
    let identity = push(steps, base.identity(&unknown, &unknown, &map))?;
    let mut complete = base.step(
        SolveSchema::ProductCompleteness,
        SolveStatement::Coverage(StatementCoverage {
            problem: problem.clone(),
            set: set.clone(),
            region: base.region.clone(),
        }),
        SolveCertificate::ProductCompleteness(CertificateProductCompleteness {
            problem: problem.clone(),
            set: set.clone(),
            scalar: one,
            factors: vec![RootFactor {
                root: zero,
                multiplicity: 1,
            }],
            product: 0,
            existence: Vec::new(),
            region: base.region.clone(),
        }),
    );
    complete.premises = vec![identity];
    let coverage = push(steps, complete)?;
    let presentation = push(steps, base.present(&set, SolveForm::Radicals))?;
    Ok(Node {
        set,
        coverage,
        presentation,
    })
}

/// Open one local context while keeping the exact parent/child meet as its admissible region.
fn enter(base: &mut Fixture, child: &Handle) -> TestResult<Handle> {
    let region = admit(
        &mut base.cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(base.header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: base.coefficients.clone(),
            },
            expression: RegionExpr::Meet(vec![
                Reference::Existing(base.region.clone()),
                Reference::Existing(child.clone()),
            ]),
        }),
    )?;
    let frame = admit(
        &mut base.cas,
        &Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(base.context.clone()),
            region: Reference::Existing(child.clone()),
            hypotheses: Vec::new(),
        }),
    )?;
    base.context.clone_from(&frame);
    base.region = region;
    Ok(frame)
}

/// Closing transports each theorem separately; neither theorem supplies the other implicitly.
fn close(
    base: &Fixture,
    frame: &Handle,
    proof: u32,
    construction: u32,
    steps: &mut Vec<ApplicationDraft>,
) -> TestResult<u32> {
    let conclusion = steps
        .get(usize::try_from(proof)?)
        .ok_or("missing child proof")?
        .conclusion
        .clone();
    push(
        steps,
        ApplicationDraft {
            rule: Primitive::ContextClose,
            context: base.context.clone(),
            conclusion,
            premises: vec![proof, construction],
            substitutions: Vec::new(),
            certificate: Certificate::Close {
                frame: frame.clone(),
            },
        },
    )
}

/// A nested split closes into its immediate parent before that parent is itself closed.
fn local(
    base: &mut Fixture,
    problem: &Handle,
    child: &Handle,
    depth: u8,
    steps: &mut Vec<ApplicationDraft>,
) -> TestResult<Node> {
    let parent = base.context.clone();
    let domain = base.region.clone();
    let frame = enter(base, child)?;
    let mut node = tree(base, problem, depth, steps)?;
    let region = base.region.clone();
    base.context = parent;
    base.region = domain;
    let construction = push(
        steps,
        ApplicationDraft {
            rule: Primitive::RegionConstruct,
            context: base.context.clone(),
            conclusion: Statement::Constructor {
                region,
                law: ConstructorLaw::Membership,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        },
    )?;
    node.coverage = close(base, &frame, node.coverage, construction, steps)?;
    node.presentation = close(base, &frame, node.presentation, construction, steps)?;
    Ok(node)
}

/// Union the exact child branch handles without rewriting any branch region.
fn union(base: &mut Fixture, problem: &Handle, nodes: &[Node]) -> TestResult<Handle> {
    let mut branches = Vec::new();
    for node in nodes {
        let SolveView::SolutionSet(set) = base.cas.solve_view(&node.set)? else {
            return Err("missing child set".into());
        };
        branches.extend(set.branches.iter().cloned().map(Reference::Existing));
    }
    admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::SolutionSet(SolutionSetData {
            problem: Reference::Existing(problem.clone()),
            branches,
        })),
    )
}

/// Compose the representation using the separately proved combined coverage and closed children.
fn present(
    base: &Fixture,
    set: &Handle,
    coverage: u32,
    nodes: &[Node],
) -> TestResult<ApplicationDraft> {
    let mut draft = base.present(set, SolveForm::Radicals);
    draft.premises = vec![coverage];
    draft
        .premises
        .extend(nodes.iter().map(|node| node.presentation));
    let Certificate::Solve(certificate) = &mut draft.certificate else {
        return Err("missing certificate".into());
    };
    let SolveCertificate::RootSelectionRefinement(value) = certificate.as_mut() else {
        return Err("wrong certificate".into());
    };
    value.compatibility = (1..draft.premises.len())
        .map(u32::try_from)
        .collect::<Result<_, _>>()?;
    Ok(draft)
}

/// Recurse through two total parameter splits, preserving every closed child theorem.
fn tree(
    base: &mut Fixture,
    problem: &Handle,
    depth: u8,
    steps: &mut Vec<ApplicationDraft>,
) -> TestResult<Node> {
    if depth == 0 {
        return leaf(base, problem, steps);
    }
    let coefficient = base.free(if depth == 2 { "a" } else { "b" })?;
    let partition = base.split(&coefficient)?;
    let Statement::Cover { members, .. } = &partition.conclusion else {
        return Err("missing cover".into());
    };
    let children = members.clone();
    let partition_index = push(steps, partition)?;
    let mut nodes = Vec::new();
    for child in &children {
        nodes.push(local(base, problem, child, depth - 1, steps)?);
    }
    let set = union(base, problem, &nodes)?;
    let mut complete = base.step(
        SolveSchema::TotalPredicateSplit,
        SolveStatement::Coverage(StatementCoverage {
            problem: problem.clone(),
            set: set.clone(),
            region: base.region.clone(),
        }),
        SolveCertificate::TotalPredicateSplit(CertificateTotalPredicateSplit {
            mode: PredicateSplitMode::EliminateCover,
            expression: coefficient,
            region: base.region.clone(),
            children,
            branches: vec![1, 2],
            cover: Some(0),
        }),
    );
    complete.premises = vec![partition_index];
    complete
        .premises
        .extend(nodes.iter().map(|node| node.coverage));
    let coverage = push(steps, complete)?;
    let presentation = push(steps, present(base, &set, coverage, &nodes)?)?;
    Ok(Node {
        set,
        coverage,
        presentation,
    })
}

/// A complete four-leaf cover carries presentations closed twice into the original context.
pub(crate) fn closed_presentations() -> TestResult<BinomialFixture> {
    let mut base = Fixture::new()?;
    let unknown = base.free("x")?;
    let zero = base.integer(0, false)?;
    let predicate = base.equation(&unknown, &zero)?;
    let problem = base.problem("x", &predicate, &[])?;
    let mut steps = Vec::new();
    tree(&mut base, &problem, 2, &mut steps)?;
    Ok(BinomialFixture { base, steps })
}

/// Swap only the final target to a separately covered full-region branch of the same problem.
fn widen(fixture: &mut BinomialFixture, draft: &mut ApplicationDraft) -> TestResult {
    let Statement::Solve(statement) = &mut draft.conclusion else {
        return Err("missing claim".into());
    };
    let SolveStatement::Presentation(claim) = statement.as_mut() else {
        return Err("wrong claim".into());
    };
    let SolveView::SolutionSet(set) = fixture.base.cas.solve_view(&claim.set)? else {
        return Err("missing set".into());
    };
    let problem = set.problem.clone();
    let replacement = leaf(&mut fixture.base, &problem, &mut fixture.steps)?;
    claim.set.clone_from(&replacement.set);
    *draft.premises.first_mut().ok_or("missing coverage")? = replacement.coverage;
    let Certificate::Solve(certificate) = &mut draft.certificate else {
        return Err("missing certificate".into());
    };
    let SolveCertificate::RootSelectionRefinement(value) = certificate.as_mut() else {
        return Err("wrong certificate".into());
    };
    value.target = replacement.set;
    Ok(())
}

/// An independently valid presentation of y=0 cannot supply a branch for the original x=0 problem.
fn foreign(fixture: &mut BinomialFixture, draft: &mut ApplicationDraft) -> TestResult {
    let unknown = fixture.base.free("y")?;
    let zero = fixture.base.integer(0, false)?;
    let predicate = fixture.base.equation(&unknown, &zero)?;
    let problem = fixture.base.problem("y", &predicate, &[])?;
    let set = fixture.base.finite(&problem, std::slice::from_ref(&zero))?;
    let step = fixture.base.present(&set, SolveForm::Radicals);
    let index = push(&mut fixture.steps, step)?;
    *draft.premises.get_mut(1).ok_or("missing child")? = index;
    Ok(())
}

/// Mutate one independently checkable parent obligation after constructing all genuine children.
pub(crate) fn closed_presentation_mutation(mutation: u8) -> TestResult<BinomialFixture> {
    let mut fixture = closed_presentations()?;
    let mut draft = fixture.steps.pop().ok_or("missing presentation")?;
    match mutation {
        1 => widen(&mut fixture, &mut draft)?,
        3 => foreign(&mut fixture, &mut draft)?,
        _ => {
            let Certificate::Solve(certificate) = &mut draft.certificate else {
                return Err("missing certificate".into());
            };
            let SolveCertificate::RootSelectionRefinement(value) = certificate.as_mut() else {
                return Err("wrong certificate".into());
            };
            if mutation == 0 {
                value.compatibility.pop();
            } else if mutation == 2 {
                value.form = Some(SolveForm::AnyExact);
                let Statement::Solve(statement) = &mut draft.conclusion else {
                    return Err("missing claim".into());
                };
                let SolveStatement::Presentation(claim) = statement.as_mut() else {
                    return Err("wrong claim".into());
                };
                claim.form = SolveForm::AnyExact;
            } else {
                draft.premises.remove(0);
                value.compatibility = vec![0, 1];
            }
        }
    }
    fixture.steps.push(draft);
    fixture.steps = presentation_closure(&fixture.steps)?;
    Ok(fixture)
}

/// Keep the chosen root's exact dependency closure, relocating only absolute application indices.
pub(crate) fn presentation_closure(
    steps: &[ApplicationDraft],
) -> TestResult<Vec<ApplicationDraft>> {
    let root = steps.len().checked_sub(1).ok_or("missing proof root")?;
    let mut reached = vec![false; steps.len()];
    let mut pending = vec![root];
    while let Some(index) = pending.pop() {
        let seen = reached.get_mut(index).ok_or("missing proof dependency")?;
        if *seen {
            continue;
        }
        *seen = true;
        for previous in &steps
            .get(index)
            .ok_or("missing proof application")?
            .premises
        {
            let previous = usize::try_from(*previous)?;
            if previous >= index {
                return Err("forward proof dependency".into());
            }
            pending.push(previous);
        }
    }
    let mut relocated = vec![None; steps.len()];
    let mut result = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        if !reached
            .get(index)
            .copied()
            .ok_or("missing reachability flag")?
        {
            continue;
        }
        *relocated.get_mut(index).ok_or("missing relocation slot")? =
            Some(u32::try_from(result.len())?);
        let mut draft = step.clone();
        for previous in &mut draft.premises {
            *previous = relocated
                .get(usize::try_from(*previous)?)
                .copied()
                .flatten()
                .ok_or("unmapped proof dependency")?;
        }
        result.push(draft);
    }
    Ok(result)
}
