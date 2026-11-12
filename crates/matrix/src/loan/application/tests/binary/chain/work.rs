// SPDX-License-Identifier: AGPL-3.0-only
//! Literal Work values derive from the design's canonical retained-table history.

use super::{TestResult, candidate, product, published, request};
use crate::Elementwise;

/// Admission is fixed 345/415 plus 5L+3t+10q+C; zero capacity removes eleven allocations.
/// Evaluation is 1+2(1+b)+2q+19C; publication is fixed 178/218 plus 3L+7q+1+C+3C.
/// ´test:unit:chain-canonical-work-inventories´
#[test]
fn chain_canonical_work_inventories() -> TestResult {
    for (cells, admissions) in [(0, [334, 404]), (1, [364, 434]), (3, [376, 446])] {
        let (mut heap, mut account, mut tile, _) = product(cells, 1)?;
        for (index, function) in [Elementwise::SquareRoot, Elementwise::Reciprocal]
            .into_iter()
            .enumerate()
        {
            let request = request(&mut heap, tile.into_pin(), function, 2, &mut account)?;
            let before = account.usage().work;
            let owner = super::super::super::ready(super::admit(
                &mut heap,
                request,
                function,
                &mut account,
            ))?;
            assert_eq!(account.usage().work - before, admissions[index]);
            let before = account.usage().work;
            let owner = candidate(owner, &mut account)?;
            let words = u64::from(cells != 0);
            assert_eq!(
                account.usage().work - before,
                [5, 3][index] + 2 * words + 19 * u64::try_from(cells)?
            );
            let before = account.usage().work;
            (tile, _) = published(heap.publish_application(owner, &mut account))?;
            assert_eq!(
                account.usage().work - before,
                [178, 218][index] + 7 * u64::try_from(cells)? + 7 * words + 1
            );
        }
    }
    Ok(())
}
