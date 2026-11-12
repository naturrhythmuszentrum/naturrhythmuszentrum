// SPDX-License-Identifier: AGPL-3.0-only
//! Constructor admission controls with genuine checked authority and independent byte inventories.

use super::*;
use nrz_bindings::readings::algebra::{
    Exhaustion, FaultKind, Rational, Reference,
    domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr},
    enclosure::{CoordinateBound, EnclosureGoal, EnclosureRequest},
    limits::Limits,
    syntax::{AtomicSort, Draft, Environment, ExpressionDraft, Field, Operator, Sort},
};
use std::cell::Cell;

/// Preserve fixture and admission errors without converting their typed causes.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
thread_local! {
    /// Thread-local observation of the constructor's actual owner assembly closure.
    pub(super) static ASSEMBLED: Cell<bool> = const { Cell::new(false) };
}

/// Use the sigmoid integration controls' fixed ledger solely to prepare genuine authority.
/// These allowances never supply the final-admission ledger or its byte oracle.
fn preparation_limits() -> Limits {
    Limits {
        work: 2_000_000_000,
        integer_exponent: 4096,
        arithmetic_steps: 50_000_000,
        name_bytes: 131_072,
        literal_bytes: 618,
        source_bytes: 4_194_304,
        output_bytes: 4_194_304,
        witness_bytes: 4_194_304,
        ..Limits::default()
    }
}

/// Prepare sigmoid(1) in an empty environment under the established preparation ledger.
fn fixture() -> TestResult<(Cas, EnclosureRequest)> {
    let mut cas = Cas::new(preparation_limits())?;
    let term = |operator, operands| {
        Draft::Expression(ExpressionDraft {
            space: None,
            operator,
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            header: Reference::Local(0),
            scope: Vec::new(),
            binding: None,
            operands,
        })
    };
    let drafts = [
        Draft::Header(HeaderDraft::Core(Environment::default())),
        Draft::Region(RegionDraft {
            header: Reference::Local(0),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
        Draft::Context(ContextDraft::Root {
            header: Reference::Local(0),
            admissible: Reference::Local(1),
            assumptions: Vec::new(),
        }),
        term(Operator::Rational(Rational::one()), Vec::new()),
        term(Operator::Sigmoid, vec![Reference::Local(3)]),
    ];
    let [_, _, context, _, target] = cas
        .admit(&drafts, &mut Budget::new(preparation_limits())?)?
        .try_into()
        .map_err(|_| "fixture inventory")?;
    Ok((
        cas,
        EnclosureRequest {
            target,
            context,
            coordinates: Vec::new(),
            goal: EnclosureGoal::Width,
        },
    ))
}

/// Reconstruct the stated owner inventory without reading the basis's stored byte total.
/// Header, scope/key, 1,024 fixed evidence bytes and 32 bytes per coefficient
/// accompany 128 bytes per rational plus four times its unsigned component lengths.
fn owner_inventory(authority: &Prepared) -> TestResult<u64> {
    let proof = authority.proof();
    assert_eq!(
        authority.request().original.coordinates,
        [] as [CoordinateBound; 0]
    );
    let mut bytes = size_of::<PreparedSigmoid<'_>>() as u64
        + size_of::<EnclosureRequest>() as u64
        + proof.scope.len() as u64
        + authority.statement_key().len() as u64
        + 1024
        + 32 * proof.coefficients.len() as u64;
    let mut budget = Budget::new(Limits::default())?;
    for rational in [
        &proof.absolute_tail,
        &proof.relative_tail,
        &proof.import_ratio,
        &proof.central_ratio,
        &proof.endpoint_ratio,
        &proof.restoration_error,
    ]
    .into_iter()
    .chain(
        proof
            .stages
            .iter()
            .flat_map(|stage| [&stage.magnitude, &stage.error]),
    ) {
        let text = rational.exact_string(&mut budget)?;
        let (numerator, denominator) = text.split_once('/').unwrap_or((&text, "1"));
        bytes += 128 + 4 * (decimal_bytes(numerator) + decimal_bytes(denominator));
    }
    Ok(bytes)
}

/// Decimal long division by 256 independently counts unsigned component bytes.
fn decimal_bytes(text: &str) -> u64 {
    let mut digits: Vec<_> = text
        .trim_start_matches('-')
        .bytes()
        .map(|digit| u16::from(digit - b'0'))
        .collect();
    let mut count = 0;
    while digits.iter().any(|digit| *digit != 0) {
        let mut remainder = 0;
        for digit in &mut digits {
            let value = remainder * 10 + *digit;
            *digit = value / 256;
            remainder = value % 256;
        }
        count += 1;
    }
    count
}

/// The constructor's successful basis branch must admit the complete returned inventory.
/// Only basis preparation is substituted; domain, policy, retained storage, error mapping
/// and final owner assembly follow the public constructor's production path.
/// The sigmoid integration controls' fixed preparation ledger builds the checked basis first.
/// Final admission uses a separate default ledger restricted to N or N-1 output bytes
/// with prior Work preserved; the independent byte oracle also uses default limits.
/// The independent boundaries are N = H + R + S + K + 1,024 + 32C + sum(128 + 4U)
/// and N-1: H/R are the owner/original-request headers, S/K the scope/key lengths, C the coefficient
/// count, and U the unsigned component bytes of each of six proof rationals and
/// two rationals per stage. This coordinate-free fixture contributes no coordinate bytes.
/// Seven prior Work plus 14,202 construction Work exhaust 14,209 units in both cases.
/// Original request transformation remains inside the tested constructor path.
/// # Panics
/// Panics if final admission, its typed cause, assembly order or prior Work changes.
/// ´claim:matrix:prepared-sigmoid-constructor-finalization´
/// ´test:unit:prepared-sigmoid-constructor-finalization´
#[test]
fn prepared_sigmoid_constructor_finalization() -> TestResult {
    for below in [0, 1] {
        let (mut cas, request) = fixture()?;
        let context = ArithmeticContext::new()?;
        let policy = SigmoidPreparationPolicy {
            bits: 40,
            requested_bits: 44,
            revision: 1,
        };
        // Setup authority belongs to a separate graph; its handles are graph-local.
        // Only basis preparation is substituted, not the original transformation.
        let (mut basis_cas, basis_original) = fixture()?;
        let auxiliary = transform(
            &mut basis_cas,
            &basis_original,
            [1.0, 1.0],
            policy,
            &mut Budget::new(preparation_limits())?,
        )?;
        let basis = PreparedExpm1::new(
            &mut basis_cas,
            &auxiliary,
            [-1.0, -1.0],
            Policy::default(),
            &context,
            &mut Budget::new(preparation_limits())?,
            &mut Counts::default(),
        )?;
        let needed = owner_inventory(basis.authority())?;
        let limit = needed - below;
        let mut budget = Budget::new(Limits {
            output_bytes: limit,
            work: 14_209,
            ..Limits::default()
        })?;
        budget.work(7, Location::new("caller"))?;
        ASSEMBLED.set(false);
        let prepared = Cell::new(false);
        let result = PreparedSigmoid::with_basis(
            &mut cas,
            &request,
            [1.0, 1.0],
            40,
            policy,
            Policy::default(),
            &mut budget,
            |_, actual, _| {
                prepared.set(true);
                assert_eq!(actual.bits, auxiliary.bits);
                assert_eq!(actual.environment, auxiliary.environment);
                assert_eq!(actual.original.coordinates, auxiliary.original.coordinates);
                Ok(basis)
            },
        );
        assert!(
            prepared.get(),
            "the successful basis branch was reached: {:?}",
            result.as_ref().err()
        );
        assert_eq!(
            budget.usage().work,
            14_209,
            "seven prior Work plus original transformation and policy Work"
        );
        if below == 0 {
            let owner = result.map_err(|error| error.to_string())?;
            assert!(ASSEMBLED.get());
            assert_eq!(owner.preparation_policy(), policy);
            assert_eq!(owner.original, &request);
            assert_eq!(owner.basis().request(), &auxiliary);
        } else {
            assert!(
                !ASSEMBLED.get(),
                "final output admission precedes owner assembly"
            );
            let PreparedSigmoidError::Exact(fault) =
                result.expect_err("constructor final output boundary")
            else {
                panic!("typed final output refusal");
            };
            assert_eq!(
                fault.kind,
                FaultKind::Exhausted {
                    reason: Exhaustion::Limit,
                    budget: BudgetKind::OutputBytes,
                    limit,
                    needed,
                }
            );
            assert_eq!(fault.location, Location::new("prepared sigmoid"));
            assert!(fault.series_evidence.is_none());
        }
    }
    Ok(())
}
