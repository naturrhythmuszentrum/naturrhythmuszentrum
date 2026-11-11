// SPDX-License-Identifier: AGPL-3.0-only
//! Small independent unsigned limb arithmetic for exact unreduced rational comparisons.

use std::cmp::Ordering;

/// Little-endian radix 2^32 magnitude with no high zero limb.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Natural(Vec<u32>);
impl Natural {
    /// Decode an integer without any floating operation.
    pub(super) fn new(value: u64) -> Self {
        let [low, high] = [value & u64::from(u32::MAX), value >> 32]
            .map(|part| u32::try_from(part).expect("32-bit limb"));
        Self::normalize(vec![low, high])
    }
    /// Canonical zero has no limbs.
    fn normalize(mut words: Vec<u32>) -> Self {
        while words.last() == Some(&0) {
            words.pop();
        }
        Self(words)
    }
    /// Canonical magnitude comparison inspects high limbs first.
    pub(super) fn compare(&self, other: &Self) -> Ordering {
        self.0
            .len()
            .cmp(&other.0.len())
            .then_with(|| self.0.iter().rev().cmp(other.0.iter().rev()))
    }
    /// Zero is tested without a normalization operation.
    pub(super) fn is_zero(&self) -> bool {
        self.0.is_empty()
    }
    /// Exact power-of-two scaling uses complete limbs and a final carry.
    pub(super) fn shift(&self, bits: usize) -> Self {
        let mut words = vec![0; bits / 32];
        let mut carry = 0_u64;
        for word in &self.0 {
            let shifted = (u64::from(*word) << (bits % 32)) | carry;
            words.push(u32::try_from(shifted & u64::from(u32::MAX)).expect("low limb"));
            carry = shifted >> 32;
        }
        words.push(u32::try_from(carry).expect("carry limb"));
        Self::normalize(words)
    }
    /// Exact sum with an explicit radix carry.
    pub(super) fn add(&self, other: &Self) -> Self {
        let mut result = Vec::new();
        let mut carry = 0_u64;
        for index in 0..self.0.len().max(other.0.len()) {
            let value = u64::from(*self.0.get(index).unwrap_or(&0))
                + u64::from(*other.0.get(index).unwrap_or(&0))
                + carry;
            result.push(u32::try_from(value & u64::from(u32::MAX)).expect("sum limb"));
            carry = value >> 32;
        }
        result.push(u32::try_from(carry).expect("sum carry"));
        Self::normalize(result)
    }
    /// Exact difference for a precompared ordered magnitude pair.
    pub(super) fn sub(&self, other: &Self) -> Self {
        assert_ne!(self.compare(other), Ordering::Less);
        let mut result = Vec::new();
        let mut borrow = 0_i64;
        for (index, word) in self.0.iter().enumerate() {
            let value = i64::from(*word) - i64::from(*other.0.get(index).unwrap_or(&0)) - borrow;
            borrow = i64::from(value < 0);
            result.push(u32::try_from(value + (borrow << 32)).expect("difference limb"));
        }
        Self::normalize(result)
    }
    /// Schoolbook multiplication never calls a production numerical or exact-arithmetic kernel.
    pub(super) fn mul(&self, other: &Self) -> Self {
        let mut result = vec![0_u32; self.0.len() + other.0.len()];
        for (left_index, left) in self.0.iter().enumerate() {
            let mut carry = 0_u64;
            for (right_index, right) in other.0.iter().enumerate() {
                let index = left_index + right_index;
                let value = u64::from(*left) * u64::from(*right) + u64::from(result[index]) + carry;
                result[index] = u32::try_from(value & u64::from(u32::MAX)).expect("product limb");
                carry = value >> 32;
            }
            if !other.is_zero() {
                result[left_index + other.0.len()] = u32::try_from(carry).expect("product carry");
            }
        }
        Self::normalize(result)
    }
}
