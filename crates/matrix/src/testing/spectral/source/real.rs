// SPDX-License-Identifier: AGPL-3.0-only
//! The physical real parameter context is separate from the original complex root fibre.

use super::{Fixture, TestResult, admit, expression, reference};
use nrz_bindings::readings::algebra::{
    Handle,
    domain::{AmbientDomain, AssumptionDraft, ContextDraft, HeaderDraft, RegionDraft, RegionExpr},
    limits::Budget,
    solving::{Coordinate, CoordinateRole, CoordinateSpaceData, SolveDraft},
    syntax::{AtomicSort, Declaration, Draft, Environment, Field, Operator, PredicateKind, Sort},
};

/// Explicitly construct the whole positive real base, retaining distinct original parameter handles.
pub(crate) fn context(source: &mut Fixture, budget: &mut Budget) -> TestResult<Handle> {
    let mut declarations: Vec<_> = ["L", "C"]
        .into_iter()
        .map(|key| Declaration {
            key: key.into(),
            display_name: key.into(),
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
        })
        .collect();
    declarations.extend(super::super::declarations::inventory());
    let header = admit(
        &mut source.cas,
        Draft::Header(HeaderDraft::Core(Environment {
            declarations,
            ..Environment::default()
        })),
        budget,
    )?;
    source.header = header;
    source.real = expression(source, Field::Real, Operator::Free("L".into()), &[], budget)?;
    let coordinates = ["L", "C"]
        .into_iter()
        .map(|key| Coordinate {
            key: key.into(),
            field: Field::Real,
            role: CoordinateRole::Coefficient,
        })
        .collect();
    let space = admit(
        &mut source.cas,
        Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            header: reference(&source.header),
            coordinates,
            scalar_spaces: Vec::new(),
        })),
        budget,
    )?;
    let mut region = admit(
        &mut source.cas,
        Draft::Region(RegionDraft {
            header: reference(&source.header),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: space.clone(),
            },
            expression: RegionExpr::Ambient,
        }),
        budget,
    )?;
    let mut assumptions = Vec::new();
    for key in ["L", "C"] {
        let (guarded, predicate) = guard(source, &space, &region, key, budget)?;
        region = guarded;
        assumptions.push(AssumptionDraft {
            key: format!("positive-{key}"),
            condition: reference(&predicate),
        });
    }
    admit(
        &mut source.cas,
        Draft::Context(ContextDraft::Root {
            header: reference(&source.header),
            admissible: reference(&region),
            assumptions,
        }),
        budget,
    )
}

/// The positive domain and the named proof premise retain the same original predicate.
fn guard(
    source: &mut Fixture,
    space: &Handle,
    base: &Handle,
    key: &str,
    budget: &mut Budget,
) -> TestResult<(Handle, Handle)> {
    let input = expression(source, Field::Real, Operator::Free(key.into()), &[], budget)?;
    let predicate = admit(
        &mut source.cas,
        Draft::Condition {
            header: reference(&source.header),
            scope: Vec::new(),
            kind: PredicateKind::Positive,
            arguments: vec![reference(&input)],
        },
        budget,
    )?;
    let guarded = admit(
        &mut source.cas,
        Draft::Region(RegionDraft {
            header: reference(&source.header),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: space.clone(),
            },
            expression: RegionExpr::GuardTrue {
                base: reference(base),
                predicate: reference(&predicate),
            },
        }),
        budget,
    )?;
    Ok((guarded, predicate))
}
