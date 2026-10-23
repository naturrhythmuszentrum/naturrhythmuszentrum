// SPDX-License-Identifier: AGPL-3.0-only
//! Read-only descriptor evidence over opaque numerical tickets; no ticket grants live access.
//!
//! The region-bindings integration must resolve scope, generations, versions, layouts,
//! precision and companion targets under its execution lease. These records check the
//! exact inventory and overlap obligations without allocating, leasing or publishing storage.

mod coverage;
mod entry;
mod operation;
mod reading;

pub use coverage::ReadingCoverage;
pub use entry::EntryTicket;
pub use operation::{AffineAdapter, OperatorAdapter};
pub use reading::{CompanionAdapter, ReadingAdapter};

use nrz_bindings::{
    Budget,
    readings::algebra::{Fault, Location, limits::BudgetKind},
};

/// Read-only descriptor inventory refusal; none of these checks establishes live heap authority.
#[derive(Clone, Debug, thiserror::Error)]
pub enum ReadingAdapterError {
    /// Selected entries do not match the exact reading or representation extent.
    #[error("reading adapter extent mismatch")]
    Extent,
    /// One proposed ordered selection repeats a numerical entry.
    #[error("reading adapter duplicate entry")]
    Duplicate,
    /// A required numerical entry lacks any declared reading.
    #[error("reading adapter missing coverage")]
    Coverage,
    /// Intersecting readings lack the exact ordered selected-coordinate equality.
    #[error("reading adapter missing checked overlap")]
    Overlap,
    /// Evidence names a substituted value, companion or operation target.
    #[error("reading adapter target mismatch")]
    Target,
    /// An exact resource ceiling or bounded metadata allocation refused.
    #[error("reading adapter resources: {0}")]
    Resources(#[from] Fault),
}

/// Location shared by read-only metadata construction and comparison charges.
const AT: Location = Location::new("reading adapter");

/// Bound retained metadata and charge the complete caller-owned ledger before traversal.
fn capacity<T>(count: usize, budget: &mut Budget) -> Result<(), ReadingAdapterError> {
    let count = u64::try_from(count).map_err(|_| ReadingAdapterError::Extent)?;
    let bytes = count
        .checked_mul(
            u64::try_from(std::mem::size_of::<T>()).map_err(|_| ReadingAdapterError::Extent)?,
        )
        .ok_or(ReadingAdapterError::Extent)?;
    budget.check(BudgetKind::MatrixEntries, count, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(count, AT)?;
    Ok(())
}

/// Fallibly allocate bounded metadata without touching numerical storage.
fn reserve<T>(count: usize, budget: &mut Budget) -> Result<Vec<T>, ReadingAdapterError> {
    capacity::<T>(count, budget)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    Ok(values)
}

/// Charge complete original physical metadata before an exact reading identity comparison.
fn reading_work(
    reading: &nrz_bindings::readings::PhysicalReading,
    budget: &mut Budget,
) -> Result<(), ReadingAdapterError> {
    for count in [
        reading.entries().len(),
        reading.family().entries().len(),
        reading.family().original_entries().len(),
        reading.selection().indices().len(),
        reading.map().coefficients().len(),
        reading.map().offsets().len(),
    ] {
        budget.work(
            u64::try_from(count).map_err(|_| ReadingAdapterError::Extent)?,
            AT,
        )?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod fixtures;

#[cfg(test)]
mod tests {
    use super::fixtures::{
        Case, Fixture, TestResult, boundaries, complex_case, ledger, map, matrix, meaning,
        state_case,
    };
    use super::*;
    use nrz_bindings::{
        CompanionKind,
        readings::{
            AffineOperation, CompanionParameters, CompanionReading, CompanionReadingKind,
            CoordinateFamily, LinearOperator, OperatorApplication, ReadingAxis, ReadingRelation,
            ReadingShape, Representation, RepresentationDefinition,
            algebra::{
                readings::{AffineMap, SelectionSpec},
                syntax::{AtomicSort, Sort},
            },
        },
    };

    /// Exact base-region positions; changing the opaque identity changes the numerical version.
    fn tickets(region: u64, indices: &[u64]) -> Vec<EntryTicket<u64>> {
        indices
            .iter()
            .map(|index| EntryTicket {
                region,
                index: *index,
            })
            .collect()
    }

    /// Read-only bindings and the sole exact selected-coordinate intersection proof.
    type Pair = (
        Case,
        ReadingAdapter<u64>,
        ReadingAdapter<u64>,
        ReadingRelation,
    );

    /// A state and a scalar component overlap at exactly their common selected entry.
    fn pair() -> TestResult<Pair> {
        let mut case = state_case()?;
        let symbol = case.fixture.symbol("z")?;
        let whole = Representation::prove(
            &mut case.fixture.cas,
            &case.whole,
            RepresentationDefinition::Defined {
                symbol,
                expansion: AffineMap::identity(2, &mut ledger()?)?,
            },
            &mut ledger()?,
        )?;
        let symbol = case.fixture.symbol("v")?;
        let component = Representation::prove(
            &mut case.fixture.cas,
            &case.component,
            RepresentationDefinition::Defined {
                symbol,
                expansion: AffineMap::identity(1, &mut ledger()?)?,
            },
            &mut ledger()?,
        )?;
        let left = case
            .whole
            .project_selected(&mut case.fixture.cas, &[0], &mut ledger()?)?;
        let right = case
            .component
            .project_selected(&mut case.fixture.cas, &[0], &mut ledger()?)?;
        let relation = ReadingRelation::prove(&mut case.fixture.cas, left, right, &mut ledger()?)?;
        let whole = ReadingAdapter::new(whole, tickets(1, &[0, 1]), &mut ledger()?)?;
        let component = ReadingAdapter::new(component, tickets(1, &[0]), &mut ledger()?)?;
        Ok((case, whole, component, relation))
    }

    /// Total coverage stores all checked views and rejects absent or surplus overlap claims.
    /// ´test:unit:reading-coverage-requires-every-intersection´
    #[test]
    fn reading_coverage_requires_every_intersection() -> TestResult {
        let (_, whole, component, relation) = pair()?;
        let inventory = vec![whole.clone(), component.clone()];
        let checked = ReadingCoverage::new(
            tickets(1, &[0, 1]),
            inventory.clone(),
            vec![relation.clone()],
            &mut ledger()?,
        )?;
        assert_eq!(checked.readings().len(), 2);
        assert_eq!(checked.overlaps().len(), 1);
        assert!(matches!(
            ReadingCoverage::new(
                tickets(1, &[0, 1]),
                inventory.clone(),
                vec![],
                &mut ledger()?
            ),
            Err(ReadingAdapterError::Overlap)
        ));
        assert!(matches!(
            ReadingCoverage::new(
                tickets(1, &[0, 1, 2]),
                inventory.clone(),
                vec![relation.clone()],
                &mut ledger()?
            ),
            Err(ReadingAdapterError::Coverage)
        ));
        assert!(matches!(
            ReadingCoverage::new(
                tickets(1, &[0]),
                inventory.clone(),
                vec![relation.clone()],
                &mut ledger()?
            ),
            Err(ReadingAdapterError::Coverage)
        ));
        assert!(
            ReadingCoverage::new(
                tickets(2, &[0, 1]),
                inventory,
                vec![relation.clone()],
                &mut ledger()?
            )
            .is_err()
        );
        assert!(
            ReadingCoverage::new(
                tickets(1, &[0, 1]),
                vec![whole, component.clone(), component],
                vec![relation],
                &mut ledger()?
            )
            .is_err()
        );
        Ok(())
    }

    /// Equal exact copies still require the proof for the actually intersecting stored position.
    /// ´test:unit:reading-coverage-refuses-wrong-stored-copy´
    #[test]
    fn reading_coverage_refuses_wrong_stored_copy() -> TestResult {
        let mut case = complex_case()?;
        let symbol = case.fixture.symbol("block")?;
        let whole = Representation::prove(
            &mut case.fixture.cas,
            &case.whole,
            RepresentationDefinition::ComplexBlock(symbol),
            &mut ledger()?,
        )?;
        let symbol = case.fixture.symbol("r")?;
        let component = Representation::prove(
            &mut case.fixture.cas,
            &case.component,
            RepresentationDefinition::Defined {
                symbol,
                expansion: AffineMap::identity(1, &mut ledger()?)?,
            },
            &mut ledger()?,
        )?;
        let whole = ReadingAdapter::new(whole, tickets(1, &[0, 1, 2, 3]), &mut ledger()?)?;
        let component = ReadingAdapter::new(component, tickets(1, &[3]), &mut ledger()?)?;
        let mut proofs = Vec::new();
        for selected in [0, 3] {
            let left =
                case.whole
                    .project_selected(&mut case.fixture.cas, &[selected], &mut ledger()?)?;
            let right =
                case.component
                    .project_selected(&mut case.fixture.cas, &[0], &mut ledger()?)?;
            proofs.push(ReadingRelation::prove(
                &mut case.fixture.cas,
                left,
                right,
                &mut ledger()?,
            )?);
        }
        assert!(matches!(
            ReadingCoverage::new(
                tickets(1, &[0, 1, 2, 3]),
                vec![whole.clone(), component.clone()],
                vec![proofs[0].clone()],
                &mut ledger()?
            ),
            Err(ReadingAdapterError::Overlap)
        ));
        let checked = ReadingCoverage::new(
            tickets(1, &[0, 1, 2, 3]),
            vec![whole, component],
            vec![proofs[1].clone()],
            &mut ledger()?,
        )?;
        assert_eq!(checked.overlaps()[0].left().coordinates(), &[3]);
        Ok(())
    }

    /// Representation None covers no numerical entries and cannot hide missing varying storage.
    /// ´test:unit:reading-adapter-keeps-exact-no-storage-contract´
    #[test]
    fn reading_adapter_keeps_exact_no_storage_contract() -> TestResult {
        let mut case = state_case()?;
        let fixed = case.fixture.define(
            case.whole.family(),
            "v",
            SelectionSpec::Column(0),
            map(2, 1, &[0, 0], &[7])?,
            case.component.entries().to_vec(),
        )?;
        let representation = Representation::prove(
            &mut case.fixture.cas,
            &fixed,
            RepresentationDefinition::Exact,
            &mut ledger()?,
        )?;
        assert!(
            ReadingAdapter::new(representation.clone(), tickets(1, &[0]), &mut ledger()?).is_err()
        );
        let exact = ReadingAdapter::<u64>::new(representation, Vec::new(), &mut ledger()?)?;
        let checked = ReadingCoverage::new(Vec::new(), vec![exact], Vec::new(), &mut ledger()?)?;
        assert_eq!(checked.entries(), [] as [EntryTicket<u64>; 0]);
        let (_, whole, _, _) = pair()?;
        assert!(
            ReadingAdapter::new(
                whole.representation().clone(),
                tickets(1, &[0, 0]),
                &mut ledger()?
            )
            .is_err()
        );
        Ok(())
    }

    /// Exact affine operands and results remain bound to their checked map equation.
    /// ´test:unit:affine-adapter-refuses-result-substitution´
    #[test]
    fn affine_adapter_refuses_result_substitution() -> TestResult {
        let (mut case, whole, component, _) = pair()?;
        let operation = AffineOperation::prove(
            &mut case.fixture.cas,
            &case.whole,
            &case.component,
            map(2, 1, &[1, 0], &[0])?,
            &mut ledger()?,
        )?;
        let checked = AffineAdapter::new(
            operation.clone(),
            whole.clone(),
            component.clone(),
            &mut ledger()?,
        )?;
        assert_eq!(checked.output().reading(), &case.component);
        assert!(matches!(
            AffineAdapter::new(operation, component, whole, &mut ledger()?),
            Err(ReadingAdapterError::Target)
        ));
        Ok(())
    }

    /// An actual distinct radius parameter symbol and its target reading share no numerical payload.
    fn companion() -> TestResult<(CompanionReading, ReadingAdapter<u64>)> {
        let mut fixture = Fixture::new(&[
            ("z", matrix(2, 1)),
            ("x", matrix(2, 1)),
            ("parameters", matrix(2, 1)),
            ("radii", matrix(2, 1)),
        ])?;
        let entry = meaning("voltage", [2, 1, -3, -1, 0, 0, 0])?;
        let family = CoordinateFamily::new(
            &fixture.cas,
            &fixture.context,
            &fixture.symbol("z")?,
            vec![entry.clone(); 2],
            &mut ledger()?,
        )?;
        let reading = fixture.define(
            &family,
            "x",
            SelectionSpec::Column(0),
            AffineMap::identity(2, &mut ledger()?)?,
            vec![entry; 2],
        )?;
        let parameter = CompanionParameters::new(
            &fixture.cas,
            &fixture.context,
            &fixture.symbol("parameters")?,
            CompanionReadingKind::Radius(CompanionKind::NumericalError),
            std::slice::from_ref(&reading),
            &mut ledger()?,
        )?;
        let symbol = fixture.symbol("radii")?;
        let companion = CompanionReading::define(
            &mut fixture.cas,
            &symbol,
            &parameter,
            std::slice::from_ref(&reading),
            &mut ledger()?,
        )?;
        let symbol = fixture.symbol("z")?;
        let representation = Representation::prove(
            &mut fixture.cas,
            &reading,
            RepresentationDefinition::Defined {
                symbol,
                expansion: AffineMap::identity(2, &mut ledger()?)?,
            },
            &mut ledger()?,
        )?;
        Ok((
            companion,
            ReadingAdapter::new(representation, tickets(1, &[0, 1]), &mut ledger()?)?,
        ))
    }

    /// Companion adapters retain exact parameter targets and reject equal-extent substitutions.
    /// ´test:unit:companion-adapter-refuses-target-substitution´
    #[test]
    fn companion_adapter_refuses_target_substitution() -> TestResult {
        let (reading, target) = companion()?;
        let checked = CompanionAdapter::new(
            reading.clone(),
            tickets(2, &[0, 1]),
            vec![target],
            &mut ledger()?,
        )?;
        assert_eq!(
            checked.companion().kind(),
            CompanionReadingKind::Radius(CompanionKind::NumericalError)
        );
        let (_, wrong, _, _) = pair()?;
        assert!(matches!(
            CompanionAdapter::new(reading, tickets(2, &[0, 1]), vec![wrong], &mut ledger()?),
            Err(ReadingAdapterError::Target)
        ));
        Ok(())
    }

    /// Exact ordered product fixture supplies each coefficient and result contract independently.
    fn product_parts() -> TestResult<(OperatorApplication, ReadingAdapter<u64>)> {
        let integer = Sort::Value(AtomicSort::Integer);
        let mut fixture = Fixture::new(&[
            ("z", matrix(2, 1)),
            ("x", matrix(2, 1)),
            ("A", matrix(2, 2)),
            ("input", integer.clone()),
            ("output", integer.clone()),
            ("batch", integer),
        ])?;
        let entries = vec![
            meaning("voltage", [2, 1, -3, -1, 0, 0, 0])?,
            meaning("current", [0, 0, 0, 1, 0, 0, 0])?,
        ];
        let family = CoordinateFamily::new(
            &fixture.cas,
            &fixture.context,
            &fixture.symbol("z")?,
            entries.clone(),
            &mut ledger()?,
        )?;
        let reading = fixture.define(
            &family,
            "x",
            SelectionSpec::Column(0),
            AffineMap::identity(2, &mut ledger()?)?,
            entries.clone(),
        )?;
        let mut axes = Vec::new();
        for (key, count) in [("input", 2), ("output", 2), ("batch", 1)] {
            axes.push(ReadingAxis::new(
                &fixture.cas,
                &fixture.context,
                &fixture.symbol(key)?,
                (0..count).collect(),
                vec![count],
                &mut ledger()?,
            )?);
        }
        let input = ReadingShape::new(
            &reading,
            [axes[0].clone(), axes[2].clone()],
            &[SelectionSpec::Block {
                row: 0,
                column: 0,
                rows: 2,
                columns: 1,
            }],
            &mut ledger()?,
        )?;
        let operator = LinearOperator::declare(
            &fixture.cas,
            &fixture.context,
            &fixture.symbol("A")?,
            [axes[1].clone(), axes[0].clone()],
            entries.clone(),
            entries,
            &mut ledger()?,
        )?;
        let operation =
            OperatorApplication::define(&mut fixture.cas, &operator, &input, &mut ledger()?)?;
        let symbol = fixture.symbol("z")?;
        let representation = Representation::prove(
            &mut fixture.cas,
            &reading,
            RepresentationDefinition::Defined {
                symbol,
                expansion: AffineMap::identity(2, &mut ledger()?)?,
            },
            &mut ledger()?,
        )?;
        Ok((
            operation,
            ReadingAdapter::new(representation, tickets(1, &[0, 1]), &mut ledger()?)?,
        ))
    }

    /// Native operation tickets retain every coefficient, input and result without live access.
    /// ´test:unit:operator-adapter-refuses-incomplete-inventory´
    #[test]
    fn operator_adapter_refuses_incomplete_inventory() -> TestResult {
        let (operation, input) = product_parts()?;
        let checked = OperatorAdapter::new(
            operation.clone(),
            tickets(2, &[0, 1, 2, 3]),
            input.clone(),
            tickets(3, &[0, 1]),
            &mut ledger()?,
        )?;
        assert_eq!(checked.operation().node(), operation.node());
        assert_eq!(checked.coefficients().len(), 4);
        assert!(matches!(
            OperatorAdapter::new(
                operation.clone(),
                tickets(2, &[0, 1, 2]),
                input.clone(),
                tickets(3, &[0, 1]),
                &mut ledger()?
            ),
            Err(ReadingAdapterError::Extent)
        ));
        assert!(matches!(
            OperatorAdapter::new(
                operation.clone(),
                tickets(2, &[0, 1, 2, 3]),
                input,
                tickets(3, &[0, 0]),
                &mut ledger()?
            ),
            Err(ReadingAdapterError::Duplicate)
        ));
        let (_, wrong, _, _) = pair()?;
        assert!(matches!(
            OperatorAdapter::new(
                operation,
                tickets(2, &[0, 1, 2, 3]),
                wrong,
                tickets(3, &[0, 1]),
                &mut ledger()?
            ),
            Err(ReadingAdapterError::Target)
        ));
        Ok(())
    }

    /// Read-only constructors retain exact Work and payload limits without granting live authority.
    /// ´test:unit:reading-adapter-resource-boundaries´
    #[test]
    fn reading_adapter_resource_boundaries() -> TestResult {
        for operation in 0..5 {
            boundaries(|limits, precharge| {
                let mut budget = Budget::new(limits)?;
                budget.work(precharge, AT)?;
                adapter_attempt(operation, &mut budget)?;
                Ok(budget.usage().work)
            })?;
        }
        Ok(())
    }

    /// Independent complete exact fixtures precede each measured metadata constructor.
    fn adapter_attempt(operation: u32, budget: &mut Budget) -> TestResult {
        if operation == 4 {
            let (operation, input) = product_parts()?;
            OperatorAdapter::new(
                operation,
                tickets(2, &[0, 1, 2, 3]),
                input,
                tickets(3, &[0, 1]),
                budget,
            )?;
        } else if operation == 3 {
            let (reading, target) = companion()?;
            CompanionAdapter::new(reading, tickets(2, &[0, 1]), vec![target], budget)?;
        } else {
            let (mut case, whole, component, relation) = pair()?;
            if operation == 0 {
                ReadingAdapter::new(whole.representation().clone(), tickets(1, &[0, 1]), budget)?;
            } else if operation == 1 {
                ReadingCoverage::new(
                    tickets(1, &[0, 1]),
                    vec![whole, component],
                    vec![relation],
                    budget,
                )?;
            } else {
                let law = AffineOperation::prove(
                    &mut case.fixture.cas,
                    &case.whole,
                    &case.component,
                    map(2, 1, &[1, 0], &[0])?,
                    &mut ledger()?,
                )?;
                AffineAdapter::new(law, whole, component, budget)?;
            }
        }
        Ok(())
    }
}
