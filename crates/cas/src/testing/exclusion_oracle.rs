// SPDX-License-Identifier: AGPL-3.0-only
//! A scoped test switch retains complete nonvanishing search as an independent oracle.

use std::cell::Cell;

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn active() -> bool {
    ACTIVE.with(Cell::get)
}

pub(crate) fn run<T>(callback: impl FnOnce() -> T) -> T {
    let previous = ACTIVE.with(|active| active.replace(true));
    let result = callback();
    ACTIVE.with(|active| active.set(previous));
    result
}

#[cfg(test)]
mod tests {
    /// Nested oracle calls and error returns restore the previous thread-local checking route.
    /// ´test:unit:exclusion-oracle-restores-after-error´
    #[test]
    fn exclusion_oracle_restores_after_error() {
        assert!(!super::active());
        let result: Result<(), ()> = super::run(|| {
            assert!(super::active());
            super::run(|| assert!(super::active()));
            assert!(super::active());
            Err(())
        });
        assert_eq!(result, Err(()));
        assert!(!super::active());
    }
}
