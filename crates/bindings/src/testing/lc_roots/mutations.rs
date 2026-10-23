// SPDX-License-Identifier: AGPL-3.0-only
//! Public proposals rebind source changes without manufacturing a coverage theorem.

use super::{TestResult, admit, reference};
use nrz_bindings::readings::algebra::{
    Cas, Handle,
    limits::Budget,
    request::{Request, Task},
    solving::{
        SolutionBranchData, SolutionBranchKind, SolutionSetData, SolveDraft, SolveProblemData,
        SolveView,
    },
    syntax::Draft,
};

pub(crate) fn problem_data(cas: &Cas, request: &Request) -> TestResult<SolveProblemData<Handle>> {
    let Task::Solve(specification) = &request.task else {
        return Err("LC task".into());
    };
    let SolveView::SolveProblem(data) = cas.solve_view(&specification.problem)? else {
        return Err("LC problem".into());
    };
    Ok(data.clone())
}

pub(crate) fn replace_problem(
    cas: &mut Cas,
    request: &mut Request,
    data: SolveProblemData<Handle>,
    budget: &mut Budget,
) -> TestResult {
    let replacement = admit(
        cas,
        Draft::Solve(SolveDraft::SolveProblem(SolveProblemData {
            header: reference(&data.header),
            scope: data.scope,
            ambient: data.ambient,
            coefficient_domain: reference(&data.coefficient_domain),
            coefficients: reference(&data.coefficients),
            unknowns: reference(&data.unknowns),
            predicate: reference(&data.predicate),
            region: reference(&data.region),
            domain: reference(&data.domain),
            denominators: data.denominators.iter().map(reference).collect(),
        })),
        budget,
    )?;
    let Task::Solve(specification) = &mut request.task else {
        return Err("LC task".into());
    };
    specification.problem = replacement;
    Ok(())
}

pub(crate) fn rebind(
    cas: &mut Cas,
    request: &Request,
    set: &Handle,
    omit: bool,
    budget: &mut Budget,
) -> TestResult<Handle> {
    let Task::Solve(specification) = &request.task else {
        return Err("LC task".into());
    };
    let data = problem_data(cas, request)?;
    let mut tuples = roots(cas, set)?;
    if omit {
        tuples.pop().ok_or("LC omitted root")?;
    }
    let branch = admit(
        cas,
        Draft::Solve(SolveDraft::SolutionBranch(SolutionBranchData {
            problem: reference(&specification.problem),
            region: reference(&data.region),
            domain: reference(&data.domain),
            kind: SolutionBranchKind::Finite {
                tuples: tuples.iter().map(reference).collect(),
            },
        })),
        budget,
    )?;
    admit(
        cas,
        Draft::Solve(SolveDraft::SolutionSet(SolutionSetData {
            problem: reference(&specification.problem),
            branches: vec![reference(&branch)],
        })),
        budget,
    )
}

pub(crate) fn roots(cas: &Cas, set: &Handle) -> TestResult<Vec<Handle>> {
    let SolveView::SolutionSet(data) = cas.solve_view(set)? else {
        return Err("LC set".into());
    };
    let [branch] = data.branches.as_slice() else {
        return Err("LC finite branch".into());
    };
    let SolveView::SolutionBranch(data) = cas.solve_view(branch)? else {
        return Err("LC branch".into());
    };
    let SolutionBranchKind::Finite { tuples } = &data.kind else {
        return Err("LC roots".into());
    };
    assert_eq!(tuples.len(), 2);
    Ok(tuples.clone())
}

/// An equal-looking domain without its family projection is a different exact context.
pub(crate) fn omit_placement(
    cas: &mut Cas,
    request: &mut Request,
    budget: &mut Budget,
) -> TestResult {
    use nrz_bindings::readings::algebra::{
        domain::{ContextDraft, RegionDraft, RegionExpr},
        source::{SyntaxNode, SyntaxRole},
    };
    let domain = cas.admissible_region(&request.context)?;
    let syntax = cas.describe_syntax(&domain, budget)?;
    let SyntaxNode::Region(ambient, _) = syntax.node else {
        return Err("LC region".into());
    };
    let base = syntax
        .dependencies
        .iter()
        .find(|entry| entry.role == SyntaxRole::Base)
        .ok_or("LC guard base")?;
    let predicate = syntax
        .dependencies
        .iter()
        .find(|entry| entry.role == SyntaxRole::Predicate)
        .ok_or("LC guard predicate")?;
    let region = admit(
        cas,
        Draft::Region(RegionDraft {
            header: reference(&syntax.header),
            scope: syntax.scope,
            ambient,
            expression: RegionExpr::GuardTrue {
                base: reference(&base.target),
                predicate: reference(&predicate.target),
            },
        }),
        budget,
    )?;
    request.context = admit(
        cas,
        Draft::Context(ContextDraft::Root {
            header: reference(&syntax.header),
            admissible: reference(&region),
            assumptions: Vec::new(),
        }),
        budget,
    )?;
    Ok(())
}
