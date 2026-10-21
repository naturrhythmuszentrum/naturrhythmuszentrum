// SPDX-License-Identifier: AGPL-3.0-only
//! Both declaration spellings now produce independently replayable complete answers.

use crate::testing::common::TestResult;

/// Establish both accepted baselines before comparing inverse-renamed returned strata.
pub(crate) fn complete() -> TestResult {
    super::acceptance::source()
}
