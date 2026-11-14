// SPDX-License-Identifier: AGPL-3.0-only
//! Exact population-derived streaming/matrix numerators and packing admission evidence.

use super::super::schema::{
    Dependency, Evidence, Operation, Scope, SummaryAnalysis, SummaryCase, SummaryOutcome,
    SummaryRecipe, Width,
};

/// Checked products reject overflow and counts outside the exact wire domain.
fn product(values: &[u64]) -> Result<u64, &'static str> {
    values
        .iter()
        .try_fold(1_u64, |n, value| n.checked_mul(*value))
        .filter(|n| (1..(1_u64 << 53)).contains(n))
        .ok_or("recipe count")
}

/// Positive explicit dimensions, with no inferred shape defaults.
fn positive(values: &[u64]) -> bool {
    values.iter().all(|value| *value > 0)
}

/// Complete records retain all four optional rate meanings, including intentional absence.
pub(super) fn numerators(
    record: &SummaryCase,
    analysis: &SummaryAnalysis,
) -> Result<(), &'static str> {
    let length = record.accepted_length.ok_or("accepted length")?;
    let expected = match record.recipe.dependency {
        Dependency::BackendKernel => matrix(&record.recipe, length)?,
        Dependency::MultiAccumulatorReduction | Dependency::IndependentElements => {
            stream(&record.recipe, length)?
        }
        _ => return Err("unsupported numerator family"),
    };
    let observed = [
        Some(analysis.elements_per_second.numerator),
        analysis
            .vector_operations_per_second
            .as_ref()
            .map(|rate| rate.numerator),
        analysis
            .floating_operations_per_second
            .as_ref()
            .map(|rate| rate.numerator),
        analysis
            .logical_bytes_per_second
            .as_ref()
            .map(|rate| rate.numerator),
    ];
    if observed != expected {
        return Err("recipe rate numerators");
    }
    Ok(())
}

/// Every stream pass retains the specified tail reduction and logical operand traffic.
fn stream(recipe: &SummaryRecipe, length: u64) -> Result<[Option<u64>; 4], &'static str> {
    let [n] = recipe.shape.as_slice() else {
        return Err("stream shape");
    };
    if recipe.scope != Scope::Prepared {
        return Err("stream scope");
    }
    let passes = product(&[length, recipe.workers])?;
    let elements = product(&[passes, *n])?;
    let (traffic, floating, vectors) = match recipe.operation {
        Operation::Read => {
            if recipe.dependency != Dependency::MultiAccumulatorReduction {
                return Err("read dependency");
            }
            let lanes = match recipe.width {
                Width::Simd1 => 1,
                Width::Simd2 => 2,
                Width::Simd4 => 4,
                Width::Simd8 => 8,
                Width::Scalar | Width::BackendSelectedBinary64 => return Err("read width"),
            };
            let partials = 8 * lanes;
            if *n < partials {
                return Err("read population");
            }
            let adds = n.checked_add(partials - 1).ok_or("read count")?;
            (
                8,
                Some(product(&[passes, adds])?),
                Some(product(&[passes, n / partials, 8])?),
            )
        }
        Operation::Write | Operation::Copy | Operation::FusedStream => {
            if recipe.width != Width::Scalar || recipe.dependency != Dependency::IndependentElements
            {
                return Err("stream identity");
            }
            match recipe.operation {
                Operation::Write => (8, None, None),
                Operation::Copy => (16, None, None),
                Operation::FusedStream => (24, Some(product(&[elements, 2])?), None),
                _ => return Err("stream operation"),
            }
        }
        _ => return Err("stream operation"),
    };
    Ok([
        Some(elements),
        vectors,
        floating,
        Some(product(&[elements, traffic])?),
    ])
}

/// Matrix factor and solve deliberately have no invented floating-operation numerator.
fn matrix(recipe: &SummaryRecipe, length: u64) -> Result<[Option<u64>; 4], &'static str> {
    if recipe.workers != 1 || recipe.width != Width::BackendSelectedBinary64 {
        return Err("matrix identity");
    }
    let repeated = recipe.scope == Scope::RestorationInclusiveRepeated;
    if repeated && recipe.operation != Operation::Factor {
        return Err("matrix scope");
    }
    let (elements, floating) = match (recipe.operation, recipe.shape.as_slice()) {
        (Operation::Gemm, [m, k, n]) => (
            product(&[*m, *n, length])?,
            Some(product(&[2, *m, *k, *n, length])?),
        ),
        (Operation::Gemv, [m, k]) => (
            product(&[*m, length])?,
            Some(product(&[2, *m, *k, length])?),
        ),
        (Operation::Factor, [n]) => {
            if !repeated && length != 1 {
                return Err("factor repetition scope");
            }
            (product(&[*n, *n, length])?, None)
        }
        (Operation::Solve, [n, rhs]) => (product(&[*n, *rhs, length])?, None),
        _ => return Err("matrix shape"),
    };
    Ok([Some(elements), None, floating, None])
}

/// Packing capability is determined only by paths that bypass inaccessible thresholds.
pub(super) fn packing(record: &SummaryCase) -> Result<(), &'static str> {
    if record.recipe.dependency != Dependency::BackendKernel {
        return if record.recipe.packing.is_none() {
            Ok(())
        } else {
            Err("unexpected packing evidence")
        };
    }
    let evidence = record
        .recipe
        .packing
        .as_ref()
        .ok_or("missing packing evidence")?;
    let path = match (record.recipe.operation, record.recipe.shape.as_slice()) {
        (Operation::Gemv, [m, k]) if positive(&[*m, *k]) => {
            Some("contiguous matrix-vector dispatch")
        }
        (Operation::Gemm, [m, k, n]) if positive(&[*m, *k, *n]) => {
            if *m == 1 || *n == 1 {
                Some("contiguous matrix-vector dispatch")
            } else if *k == 1 {
                Some("contiguous rank-one dispatch")
            } else {
                None
            }
        }
        (Operation::Factor, [1]) => Some("order-one LU"),
        (Operation::Solve, [1, rhs]) if *rhs > 0 => Some("order-one solve"),
        (Operation::Factor, [_]) | (Operation::Solve, [_, _]) => None,
        _ => return Err("packing shape"),
    };
    let unavailable = |value: &Evidence<u64>| matches!(value, Evidence::Unavailable(reason) if !reason.is_empty());
    if !unavailable(&evidence.rhs_threshold) || !unavailable(&evidence.lhs_single_thread_threshold)
    {
        return Err("packing threshold evidence");
    }
    if let Some(path) = path {
        if evidence.path != Evidence::Known(path.to_owned()) || evidence.bytes != Evidence::Known(0)
        {
            return Err("packing bypass evidence");
        }
    } else {
        let reason =
            matches!(evidence.path, Evidence::Unavailable(ref reason) if !reason.is_empty());
        let started = matches!(
            record.outcome,
            SummaryOutcome::Complete(_) | SummaryOutcome::Incomplete(_)
        );
        if !reason || !unavailable(&evidence.bytes) || started {
            return Err("unaccounted packing");
        }
    }
    Ok(())
}

/// Reset, cache, wait, reduction tail and conditioning belong to the case key.
pub(super) fn protocol(recipe: &SummaryRecipe) -> Result<(), &'static str> {
    use super::super::schema::{
        AllocationStrategy, CachePolicy, CaseProtocol, ConditionEstimate, Conditioning,
        MatrixConstruction, ReadOrder, ResetPolicy, RestorationSchedule, StartWaitPolicy,
    };
    let matrix = recipe.dependency == Dependency::BackendKernel;
    let read = recipe.operation == Operation::Read;
    let lanes = match recipe.width {
        Width::Simd2 => 2,
        Width::Simd4 => 4,
        Width::Simd8 => 8,
        _ => 1,
    };
    let partials = 8 * lanes;
    let conditioned = matches!(recipe.operation, Operation::Factor | Operation::Solve);
    let repeated = recipe.scope == Scope::RestorationInclusiveRepeated;
    let solve = recipe.operation == Operation::Solve && recipe.scope == Scope::Prepared;
    let elements = if repeated {
        let n = *recipe.shape.first().ok_or("restoration shape")?;
        product(&[n, n])?
    } else if solve {
        product(&recipe.shape)?
    } else {
        0
    };
    let expected = CaseProtocol {
        allocation: if matrix {
            AllocationStrategy::MatrixOwnedBuffers
        } else if matches!(
            recipe.dependency,
            Dependency::Dependent | Dependency::Independent
        ) {
            AllocationStrategy::BoxedFixedState
        } else {
            AllocationStrategy::FallibleContiguousVectors
        },
        read_order: read.then_some(ReadOrder::AccumulatorMajorLaneMinorTailThenFold),
        matrix_construction: matrix.then_some(if conditioned {
            MatrixConstruction::ColumnMajorSeedAscendingRowSums
        } else {
            MatrixConstruction::ColumnMajorSeed
        }),
        condition_estimate: matrix.then_some(ConditionEstimate::UnknownNotEstimated),
        restoration_schedule: if repeated {
            RestorationSchedule::BeforeEveryTraversal
        } else if solve {
            RestorationSchedule::BetweenTraversals
        } else {
            RestorationSchedule::None
        },
        restoration_copied_bytes: elements
            .checked_mul(8)
            .filter(|n| *n < (1_u64 << 53))
            .ok_or("restoration bytes")?,
        restoration_logical_bytes: elements
            .checked_mul(16)
            .filter(|n| *n < (1_u64 << 53))
            .ok_or("restoration bytes")?,
        reset: if matrix {
            ResetPolicy::RestoreMatrixSource
        } else if matches!(
            recipe.dependency,
            Dependency::Dependent | Dependency::Independent
        ) {
            ResetPolicy::ReloadRuntimeSeeds
        } else {
            ResetPolicy::ZeroOutput
        },
        cache: CachePolicy::NoFlushWarmReuse,
        start_wait: if recipe.workers == 1 {
            StartWaitPolicy::DirectCall
        } else {
            StartWaitPolicy::BlockingChannel
        },
        read_partials: read.then_some(partials),
        read_tail: if read {
            Some(recipe.shape.first().ok_or("read shape")? % partials)
        } else {
            None
        },
        conditioning: matrix.then_some(if conditioned {
            Conditioning::RowAbsoluteSumPlusPositiveMargin
        } else {
            Conditioning::SeededCentered
        }),
        timed_restoration_elements: elements,
    };
    if recipe.protocol != expected {
        return Err("case protocol");
    }
    Ok(())
}
