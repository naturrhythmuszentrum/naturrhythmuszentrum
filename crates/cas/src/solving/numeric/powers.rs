// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded integer-root refinement and independent exact-power verification.

use num_bigint::{BigInt, BigUint, Sign};
use num_integer::Roots;
use num_traits::One;

use super::plan::{Plan, Scalar, Size, add, cost, mul};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::number::Rational;

/// A primitive root call is limited to 64-bit input and at most 64-bit-order logic.
/// Reserve 64 trials of 64 primitive operations at the integer envelope `C(64) = 32`.
const SEED_WORK: u64 = 131_072;

/// Validate positive order and charge a coefficient invocation before sign decisions.
fn begin(bits: u64, order: u32, budget: &mut Budget, at: Location) -> Result<(), Fault> {
    budget.solving(BudgetKind::CoefficientOps, 1, at)?;
    if order == 0 {
        return Err(Fault::invalid(InvalidCode::UndefinedPower, at));
    }
    budget.check(BudgetKind::IntegerExponent, u64::from(order), at)?;
    budget.check(BudgetKind::ComponentBits, bits, at)?;
    budget.check(BudgetKind::IntermediateBits, bits, at)?;
    budget.arithmetic(cost(bits, at)?, at)
}

/// Check all binary-search trials, exact powers, copies and final verification.
fn reserve(bits: u64, order: u32, budget: &Budget, at: Location) -> Result<u64, Fault> {
    let padding = mul(2, u64::from(order), BudgetKind::IntermediateBits, at)?;
    let extent = add(bits, padding, BudgetKind::IntermediateBits, at)?;
    budget.check(BudgetKind::IntermediateBits, extent, at)?;
    let trials = add(
        bits.div_ceil(u64::from(order)),
        1,
        BudgetKind::ArithmeticSteps,
        at,
    )?;
    let powers = mul(2, u64::from(order.ilog2()), BudgetKind::ArithmeticSteps, at)?;
    let per_trial = add(powers, 5, BudgetKind::ArithmeticSteps, at)?;
    let passes = add(trials, 1, BudgetKind::ArithmeticSteps, at)?;
    let total = mul(passes, per_trial, BudgetKind::ArithmeticSteps, at)?;
    let calls = add(total, 16, BudgetKind::ArithmeticSteps, at)?;
    reserve_resources(extent, calls, budget, at)?;
    Ok(extent)
}

/// Check cumulative work and the peak live integer buffers without allocating them.
fn reserve_resources(bits: u64, calls: u64, budget: &Budget, at: Location) -> Result<(), Fault> {
    let usage = budget.usage();
    let steps = add(
        usage.arithmetic_steps,
        calls,
        BudgetKind::ArithmeticSteps,
        at,
    )?;
    budget.check(BudgetKind::ArithmeticSteps, steps, at)?;
    let limb_work = mul(calls, cost(bits, at)?, BudgetKind::Work, at)?;
    let work = add(limb_work, SEED_WORK, BudgetKind::Work, at)?;
    budget.check(
        BudgetKind::Work,
        add(usage.work, work, BudgetKind::Work, at)?,
        at,
    )?;
    let limbs = add(bits, 63, BudgetKind::StagedPayloadBytes, at)? / 64;
    let buffers = mul(limbs, 128, BudgetKind::StagedPayloadBytes, at)?;
    let bytes = add(buffers, 256, BudgetKind::StagedPayloadBytes, at)?;
    usize::try_from(bytes).map_err(|_| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, at)
}

/// Prepay each exact product and check its result even inside the whole-operation envelope.
fn product(
    left: &BigUint,
    right: &BigUint,
    bound: u64,
    budget: &mut Budget,
    at: Location,
) -> Result<BigUint, Fault> {
    let predicted = add(left.bits(), right.bits(), BudgetKind::IntermediateBits, at)?;
    budget.check(BudgetKind::IntermediateBits, predicted, at)?;
    budget.arithmetic(cost(bound, at)?, at)?;
    let value = left * right;
    budget.check(BudgetKind::IntermediateBits, value.bits(), at)?;
    Ok(value)
}

/// Exact repeated squaring, with no floating approximation or hidden bigint power call.
fn power(
    base: &BigUint,
    order: u32,
    bound: u64,
    budget: &mut Budget,
    at: Location,
) -> Result<BigUint, Fault> {
    budget.arithmetic(cost(bound, at)?, at)?;
    let mut factor = base.clone();
    let mut result = BigUint::one();
    let mut remaining = order;
    while remaining != 0 {
        if remaining & 1 == 1 {
            result = product(&result, &factor, bound, budget, at)?;
        }
        remaining /= 2;
        if remaining != 0 {
            factor = product(&factor, &factor, bound, budget, at)?;
        }
    }
    Ok(result)
}

/// Use `num_integer` only on a bounded primitive prefix, not an opaque bigint loop.
/// Dropping a multiple of the order makes the root's corresponding scale exact.
fn seed(
    value: &BigUint,
    order: u32,
    bound: u64,
    budget: &mut Budget,
    at: Location,
) -> Result<(BigUint, BigUint), Fault> {
    let bits = value.bits();
    budget.arithmetic(SEED_WORK, at)?;
    if order > 64 {
        let shift = usize::try_from(bits.div_ceil(u64::from(order)))
            .map_err(|_| Fault::capacity(BudgetKind::IntermediateBits, at))?;
        // Here the radicand exceeds `u64::MAX`, whose exact root is a lower bound.
        let candidate = Roots::nth_root(&u64::MAX, order);
        return Ok((BigUint::from(candidate), BigUint::one() << shift));
    }
    let dropped = mul(
        bits.saturating_sub(64).div_ceil(u64::from(order)),
        u64::from(order),
        BudgetKind::IntermediateBits,
        at,
    )?;
    let shift =
        usize::try_from(dropped).map_err(|_| Fault::capacity(BudgetKind::IntermediateBits, at))?;
    budget.arithmetic(cost(bound, at)?, at)?;
    let prefix = (value >> shift).iter_u64_digits().next().unwrap_or(0);
    let candidate = Roots::nth_root(&prefix, order);
    let next = candidate
        .checked_add(1)
        .ok_or_else(|| Fault::capacity(BudgetKind::IntermediateBits, at))?;
    let scale = usize::try_from(dropped / u64::from(order))
        .map_err(|_| Fault::capacity(BudgetKind::IntermediateBits, at))?;
    budget.arithmetic(cost(bound, at)?, at)?;
    Ok((
        BigUint::from(candidate) << scale,
        BigUint::from(next) << scale,
    ))
}

/// The seed interval is at most `2^ceil(B/n)` wide; each trial halves it.
fn refine(
    value: &BigUint,
    order: u32,
    bound: u64,
    budget: &mut Budget,
    at: Location,
) -> Result<BigUint, Fault> {
    let (mut low, mut high) = seed(value, order, bound, budget, at)?;
    let trials = add(
        value.bits().div_ceil(u64::from(order)),
        1,
        BudgetKind::ArithmeticSteps,
        at,
    )?;
    for _ in 0..trials {
        budget.arithmetic(cost(bound, at)?, at)?;
        if &high - &low <= BigUint::one() {
            return Ok(low);
        }
        budget.arithmetic(cost(bound, at)?, at)?;
        let middle = (&low + &high) >> 1_usize;
        let raised = power(&middle, order, bound, budget, at)?;
        budget.arithmetic(cost(bound, at)?, at)?;
        if raised <= *value {
            low = middle;
        } else {
            high = middle;
        }
    }
    Err(Fault::invalid(InvalidCode::Encoding, at))
}

/// Return a verified nonnegative root; a floor candidate alone never succeeds.
fn magnitude_root(
    value: &BigUint,
    order: u32,
    budget: &mut Budget,
    at: Location,
) -> Result<Option<BigUint>, Fault> {
    if order == 1 || value.bits() <= 1 {
        let mut plan = Plan::default();
        plan.magnitude_copy(value.bits(), at)?;
        plan.reserve(budget, at)?;
        budget.arithmetic(cost(value.bits(), at)?, at)?;
        return Ok(Some(value.clone()));
    }
    // For a magnitude above one, `2^order >= 2^bits > value` excludes every root.
    if u64::from(order) >= value.bits() {
        return Ok(None);
    }
    let bound = reserve(value.bits(), order, budget, at)?;
    let candidate = refine(value, order, bound, budget, at)?;
    let verified = power(&candidate, order, bound, budget, at)?;
    budget.arithmetic(cost(bound, at)?, at)?;
    Ok((verified == *value).then_some(candidate))
}

/// Signed integer perfect powers, with the real-root sign restriction separate from magnitude work.
pub(crate) fn integer_root(
    value: &BigInt,
    order: u32,
    budget: &mut Budget,
) -> Result<Option<BigInt>, Fault> {
    let at = Location::new("integer perfect power");
    begin(value.bits(), order, budget, at)?;
    if value.sign() == Sign::Minus && order.is_multiple_of(2) {
        return Ok(None);
    }
    Ok(magnitude_root(value.magnitude(), order, budget, at)?
        .map(|root| BigInt::from_biguint(value.sign(), root)))
}

/// Read one bounded canonical magnitude without exposing the rational's private backing.
fn magnitude(
    bytes: &mut &[u8],
    bits: u64,
    budget: &mut Budget,
    at: Location,
) -> Result<BigUint, Fault> {
    let length_bytes: [u8; 4] = bytes
        .get(..4)
        .ok_or_else(|| Fault::invalid(InvalidCode::Encoding, at))?
        .try_into()
        .map_err(|_| Fault::invalid(InvalidCode::Encoding, at))?;
    let length = usize::try_from(u32::from_le_bytes(length_bytes))
        .map_err(|_| Fault::capacity(BudgetKind::IntermediateBits, at))?;
    let end = length
        .checked_add(4)
        .ok_or_else(|| Fault::capacity(BudgetKind::IntermediateBits, at))?;
    let digits = bytes
        .get(4..end)
        .ok_or_else(|| Fault::invalid(InvalidCode::Encoding, at))?;
    budget.arithmetic(cost(bits, at)?, at)?;
    let result = BigUint::from_bytes_le(digits);
    *bytes = bytes
        .get(end..)
        .ok_or_else(|| Fault::invalid(InvalidCode::Encoding, at))?;
    Ok(result)
}

/// Extract canonical magnitudes through the existing metered rational byte interface.
fn components(
    value: &Rational,
    budget: &mut Budget,
    at: Location,
) -> Result<(BigUint, BigUint), Fault> {
    let mut plan = Plan::default();
    let size = Size::of(value, budget, at)?;
    // Encoding, decoding and both retained components fit inside this envelope.
    for _ in 0..4 {
        plan.copy(size, at)?;
    }
    plan.reserve(budget, at)?;
    let encoded = value.structural_bytes(budget)?;
    let mut bytes = encoded
        .get(1..)
        .ok_or_else(|| Fault::invalid(InvalidCode::Encoding, at))?;
    let (numerator_bits, denominator_bits) = value.component_bits();
    let numerator = magnitude(&mut bytes, numerator_bits, budget, at)?;
    let denominator = magnitude(&mut bytes, denominator_bits, budget, at)?;
    Ok((numerator, denominator))
}

/// Both canonical magnitudes must pass exact verification before a rational is constructed.
pub(crate) fn rational_root(
    value: &Rational,
    order: u32,
    budget: &mut Budget,
) -> Result<Option<Rational>, Fault> {
    let at = Location::new("rational perfect power");
    let (numerator_bits, denominator_bits) = value.component_bits();
    begin(numerator_bits.max(denominator_bits), order, budget, at)?;
    if value.is_negative() && order.is_multiple_of(2) {
        return Ok(None);
    }
    if order == 1 {
        let mut plan = Plan::default();
        plan.copy(Size::of(value, budget, at)?, at)?;
        plan.reserve(budget, at)?;
        budget.arithmetic(cost(numerator_bits.max(denominator_bits), at)?, at)?;
        return Ok(Some(value.clone()));
    }
    let (numerator, denominator) = components(value, budget, at)?;
    let Some(root_numerator) = magnitude_root(&numerator, order, budget, at)? else {
        return Ok(None);
    };
    let Some(root_denominator) = magnitude_root(&denominator, order, budget, at)? else {
        return Ok(None);
    };
    let mut plan = Plan::default();
    plan.scalar(
        Scalar::Mul,
        Size::of(value, budget, at)?,
        Size::constant(true),
        at,
    )?;
    plan.reserve(budget, at)?;
    Rational::new(
        value.is_negative(),
        &root_numerator,
        &root_denominator,
        budget,
    )
    .map(Some)
}
