// SPDX-License-Identifier: AGPL-3.0-only
//! Original ordered witnesses, coherent coordinates, and declared physical descriptors are mandatory.

use super::{BinaryElementwise, Limits, TestResult, admit, fixture};
use crate::loan::fault;
use crate::{
    ApplicationError, BinaryApplicationRequest, BinaryRegion, ExecutionLimits, LoanError,
    PointwiseBinaryOperation, RegionContract, StructureScope,
};
use nrz_bindings::Descriptor;
use std::rc::Rc;

/// Recheck precisely the original regions with explicit source order and output symbols.
fn recheck(
    setup: &mut super::Fixture,
    inputs: [RegionContract; 2],
    output: RegionContract,
    reversed: bool,
    limits: ExecutionLimits,
) -> Result<PointwiseBinaryOperation, ApplicationError> {
    let pins = if reversed {
        [&setup.request.right, &setup.request.left]
    } else {
        [&setup.request.left, &setup.request.right]
    };
    PointwiseBinaryOperation::check(
        inputs,
        output,
        pins,
        setup.request.destination.symbols.clone(),
        setup.request.destination.axis,
        BinaryElementwise::Product,
        limits,
        &mut setup.account,
    )
}
/// Rebuild an original shape with one independently specified entry, preserving its frozen owner.
fn original(
    region: &RegionContract,
    descriptor: &Descriptor,
    zero: bool,
    scope: StructureScope,
) -> TestResult<RegionContract> {
    let shape = region.shape();
    let extent = shape.extent();
    let entries = (0..extent.entries())
        .map(|entry| {
            if zero && entry + 1 == extent.entries() {
                crate::EntryDraft::Zero(descriptor.clone())
            } else {
                crate::EntryDraft::Declared(descriptor.clone())
            }
        })
        .collect();
    Ok(RegionContract::check(
        crate::ShapeDraft::new(
            shape.domain().clone(),
            shape.codomain().clone(),
            vec![crate::BlockDraft::new(
                0..extent.rows(),
                0..extent.columns(),
                entries,
            )],
            scope,
        )
        .check(
            Rc::clone(&shape.declarations),
            crate::ShapeLimits::default(),
        )?,
    )?)
}
/// Both original witnesses are checked before their normalized homogeneous descriptor can hide scale.
#[test]
fn binary_noncoherent_original_input_on_either_side_refuses() -> TestResult {
    let mut budget = nrz_bindings::Budget::new(Limits::default())?;
    let scaled = Descriptor::new(
        Descriptor::ratio().kind().clone(),
        nrz_bindings::Measure::new(
            "scaled-ratio",
            nrz_bindings::Scale::Exact(nrz_bindings::readings::algebra::Rational::from_ratio(
                2,
                1,
                &mut budget,
            )?),
            nrz_bindings::readings::algebra::Rational::zero(),
        )?,
        nrz_bindings::Role::Difference,
    )?;
    for index in 0..2 {
        let mut setup = fixture(
            1,
            1,
            [1; 2],
            0,
            false,
            BinaryElementwise::Product,
            Limits::default(),
        )?;
        let operation = &setup.request.operation.operation;
        let mut inputs = [operation.inputs()[0].clone(), operation.inputs()[1].clone()];
        inputs[index] = original(&inputs[index], &scaled, false, StructureScope::CenterOnly)?;
        assert_eq!(inputs[index].descriptor(), &Descriptor::ratio());
        let output = operation.outputs()[0].clone();
        let injection = fault::Injection::after(0);
        assert!(matches!(
            recheck(
                &mut setup,
                inputs,
                output,
                false,
                ExecutionLimits::default()
            ),
            Err(ApplicationError::Loan(LoanError::UnsupportedContract))
        ));
        assert_eq!(fault::Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}
/// Empty demand cannot hide a structural assertion on Left, Right or Output in either scope.
#[test]
fn binary_every_original_structural_scope_refuses() -> TestResult {
    for position in [
        BinaryRegion::Left,
        BinaryRegion::Right,
        BinaryRegion::Output,
    ] {
        for scope in [StructureScope::CenterOnly, StructureScope::WholeFamily] {
            let mut setup = fixture(
                3,
                2,
                [1; 2],
                0,
                false,
                BinaryElementwise::Product,
                Limits::default(),
            )?;
            let operation = &setup.request.operation.operation;
            let mut inputs = [operation.inputs()[0].clone(), operation.inputs()[1].clone()];
            let mut output = operation.outputs()[0].clone();
            match position {
                BinaryRegion::Left => {
                    inputs[0] = original(&inputs[0], &Descriptor::ratio(), true, scope)?;
                }
                BinaryRegion::Right => {
                    inputs[1] = original(&inputs[1], &Descriptor::ratio(), true, scope)?;
                }
                BinaryRegion::Output => {
                    output = original(&output, &Descriptor::ratio(), true, scope)?;
                }
            }
            let injection = fault::Injection::after(0);
            let result = recheck(
                &mut setup,
                inputs,
                output,
                false,
                ExecutionLimits::default(),
            );
            assert!(
                matches!(result, Err(ApplicationError::UnsupportedBinaryStructure { position: actual, entry: 5, scope: actual_scope }) if actual == position && actual_scope == scope)
            );
            assert_eq!(fault::Injection::attempts(), 0);
            drop(injection);
        }
    }
    Ok(())
}
/// Two pins to the same occupation and separate occupations with one value symbol both refuse.
#[test]
fn binary_duplicate_occupation_and_symbol_refuse_original_constructor() -> TestResult {
    for duplicate_symbols in [false, true] {
        let mut setup = fixture(
            1,
            1,
            [1; 2],
            2,
            false,
            BinaryElementwise::Product,
            Limits::default(),
        )?;
        let duplicate = if duplicate_symbols {
            let source = crate::loan::application::binary::bundle(&setup.request.left)?;
            let declaration = crate::HostTileDeclaration::check(
                source.declaration.symbols.clone(),
                source.declaration.axis,
                crate::Coverage::RepresentedInputs,
                &mut setup.account,
            )?;
            let mut loan = match setup.heap.request_loan(declaration, &mut setup.account) {
                crate::LoanRequest::Granted(loan) => loan,
                other @ crate::LoanRequest::Refused(_) => return Err(format!("{other:?}").into()),
            };
            loan.write_component(crate::HostChannel::Center, 0, 0, 2.0, &mut setup.account)?;
            match setup.heap.handback(loan, &mut setup.account) {
                crate::HandbackOutcome::Published(tile, _) => tile.into_pin(),
                other => return Err(format!("{other:?}").into()),
            }
        } else {
            setup
                .heap
                .retain_pin(&setup.request.left, &mut setup.account)?
        };
        setup.request.right = duplicate;
        let operation = &setup.request.operation.operation;
        let inputs = [operation.inputs()[0].clone(), operation.inputs()[1].clone()];
        let output = operation.outputs()[0].clone();
        assert!(matches!(
            recheck(
                &mut setup,
                inputs,
                output,
                false,
                ExecutionLimits::default()
            ),
            Err(ApplicationError::Loan(LoanError::TargetMismatch))
        ));
    }
    Ok(())
}
/// Captured source order matters even with identical descriptors and commuting product answers.
#[test]
fn binary_public_request_order_identity_demand_and_family_refusals() -> TestResult {
    for choice in 0..7 {
        let mut setup = fixture(
            1,
            1,
            [1; 2],
            2,
            false,
            BinaryElementwise::Product,
            Limits::default(),
        )?;
        match choice {
            0 => std::mem::swap(&mut setup.request.left, &mut setup.request.right),
            1 => {
                setup.request.right = setup
                    .heap
                    .retain_pin(&setup.request.left, &mut setup.account)?;
            }
            2 => {
                setup.request.demand = setup.heap.create_mask(
                    setup.request.right.ticket()?.identity.layout,
                    &[0],
                    &mut setup.account.ledger,
                )?;
            }
            3 => {
                setup
                    .heap
                    .retire_mask(setup.request.demand, &mut setup.account.ledger)?;
            }
            4 => {
                setup.request.destination.axis =
                    setup.heap.create_cell_axis(1, &mut setup.account.ledger)?;
            }
            5 => setup.request.destination.coverage = crate::Coverage::DeclaredBounds,
            6 => {}
            _ => return Err("case".into()),
        }
        let pins = [
            Rc::as_ptr(&setup.request.left.record),
            Rc::as_ptr(&setup.request.right.record),
        ];
        let injection = fault::Injection::after(0);
        fault::reset_reads();
        fault::reset_scalars();
        let function = if choice == 6 {
            BinaryElementwise::Quotient
        } else {
            BinaryElementwise::Product
        };
        match admit(&mut setup.heap, setup.request, function, &mut setup.account) {
            BinaryApplicationRequest::ApplicationRefused(error, request) => {
                if choice == 4 {
                    assert!(matches!(
                        error,
                        ApplicationError::Loan(LoanError::LayoutMismatch)
                    ));
                }
                assert_eq!(
                    [
                        Rc::as_ptr(&request.left.record),
                        Rc::as_ptr(&request.right.record)
                    ],
                    pins
                );
            }
            other @ BinaryApplicationRequest::ApplicationReady(_) => {
                return Err(format!("{other:?}").into());
            }
        }
        assert_eq!(
            [
                fault::reads(),
                fault::scalars(),
                fault::Injection::attempts()
            ],
            [0; 3]
        );
        drop(injection);
    }
    Ok(())
}
/// Original allowance is 21L and uses two slots independently of one-cell admission.
#[test]
fn binary_original_constructor_invocation_work_neighbours() -> TestResult {
    for (cells, components) in [(0, 3), (1, 3), (65, 1)] {
        for ceiling in 0..3 {
            for short in [false, true] {
                let required = 21 * cells * components;
                if required == 0 && ceiling > 0 && short {
                    continue;
                }
                let mut setup = fixture(
                    cells,
                    components,
                    [1; 2],
                    0,
                    false,
                    BinaryElementwise::Product,
                    Limits::default(),
                )?;
                let operation = &setup.request.operation.operation;
                let inputs = [operation.inputs()[0].clone(), operation.inputs()[1].clone()];
                let output = operation.outputs()[0].clone();
                let limits = ExecutionLimits::new(
                    2 - usize::from(ceiling == 0 && short),
                    required - usize::from(ceiling == 1 && short),
                    required - usize::from(ceiling == 2 && short),
                );
                assert_eq!(
                    recheck(&mut setup, inputs, output, false, limits).is_ok(),
                    !short
                );
            }
        }
    }
    Ok(())
}
/// The constructor visits one envelope, three regions, 3L originals, eight declarations and one derivation.
#[test]
fn binary_original_constructor_literal_work_prefixes() -> TestResult {
    for remaining in 0..=22 {
        let mut setup = fixture(
            3,
            1,
            [1; 2],
            0,
            false,
            BinaryElementwise::Product,
            Limits::default(),
        )?;
        let operation = &setup.request.operation.operation;
        let inputs = [operation.inputs()[0].clone(), operation.inputs()[1].clone()];
        let output = operation.outputs()[0].clone();
        super::leave(&mut setup.account, remaining)?;
        let before = setup.account.usage().work;
        let result = recheck(
            &mut setup,
            inputs,
            output,
            false,
            ExecutionLimits::default(),
        );
        assert_eq!(result.is_ok(), remaining == 22);
        if remaining == 22 {
            assert_eq!(setup.account.usage().work - before, 22);
        }
    }
    Ok(())
}

/// Every admission rechecks all original structural entries, including unselected output entries.
#[test]
fn binary_admission_rechecks_original_structure_before_allocation() -> TestResult {
    for position in [
        BinaryRegion::Left,
        BinaryRegion::Right,
        BinaryRegion::Output,
    ] {
        for scope in [StructureScope::CenterOnly, StructureScope::WholeFamily] {
            let mut setup = fixture(
                3,
                2,
                [1; 2],
                0,
                false,
                BinaryElementwise::Product,
                Limits::default(),
            )?;
            let operation = &mut setup.request.operation.operation;
            match position {
                BinaryRegion::Left => {
                    operation.inputs = Rc::new([
                        original(&operation.inputs()[0], &Descriptor::ratio(), true, scope)?,
                        operation.inputs()[1].clone(),
                    ]);
                }
                BinaryRegion::Right => {
                    operation.inputs = Rc::new([
                        operation.inputs()[0].clone(),
                        original(&operation.inputs()[1], &Descriptor::ratio(), true, scope)?,
                    ]);
                }
                BinaryRegion::Output => {
                    operation.outputs = Rc::new([original(
                        &operation.outputs()[0],
                        &Descriptor::ratio(),
                        true,
                        scope,
                    )?]);
                }
            }
            let injection = fault::Injection::after(0);
            fault::reset_reads();
            fault::reset_scalars();
            let result = admit(
                &mut setup.heap,
                setup.request,
                BinaryElementwise::Product,
                &mut setup.account,
            );
            assert!(
                matches!(result, BinaryApplicationRequest::ApplicationRefused(ApplicationError::UnsupportedBinaryStructure { position: actual, entry: 5, scope: actual_scope }, _) if actual == position && actual_scope == scope)
            );
            assert_eq!(
                [
                    fault::Injection::attempts(),
                    fault::reads(),
                    fault::scalars()
                ],
                [0; 3]
            );
            drop(injection);
        }
    }
    Ok(())
}

/// Fresh output symbols, exact snapshots and literal shapes are checked independently of dimensions.
#[test]
fn binary_fresh_symbols_foreign_accounts_and_shape_refusals() -> TestResult {
    for choice in 0..5 {
        let mut setup = fixture(
            1,
            1,
            [3; 2],
            2,
            false,
            BinaryElementwise::Product,
            Limits::default(),
        )?;
        let operation = &setup.request.operation.operation;
        let mut inputs = [operation.inputs()[0].clone(), operation.inputs()[1].clone()];
        let output = operation.outputs()[0].clone();
        let mut symbols = setup.request.destination.symbols.clone();
        let mut axis = setup.request.destination.axis;
        match choice {
            0 | 1 => {
                symbols = crate::loan::application::binary::bundle(if choice == 0 {
                    &setup.request.left
                } else {
                    &setup.request.right
                })?
                .declaration
                .symbols
                .clone();
            }
            2 => {
                let region = RegionContract::check(crate::kernel_testing::shape(
                    &Rc::clone(&output.shape().declarations),
                    &crate::kernel_testing::axis(2)?,
                    &crate::kernel_testing::axis(1)?,
                )?)?;
                inputs[1] = region;
            }
            3 => axis = setup.heap.create_cell_axis(1, &mut setup.account.ledger)?,
            4 => {
                setup.account =
                    crate::ExecutionAccount::new(nrz_bindings::Budget::new(Limits::default())?)?;
            }
            _ => return Err("case".into()),
        }
        let injection = fault::Injection::after(0);
        let result = PointwiseBinaryOperation::check(
            inputs,
            output,
            [&setup.request.left, &setup.request.right],
            symbols,
            axis,
            BinaryElementwise::Product,
            ExecutionLimits::default(),
            &mut setup.account,
        );
        assert!(result.is_err());
        assert_eq!(fault::Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}
