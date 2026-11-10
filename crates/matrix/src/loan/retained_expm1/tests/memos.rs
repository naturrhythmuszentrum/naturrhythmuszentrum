// SPDX-License-Identifier: AGPL-3.0-only
//! Public exact services seed independently enumerated persistent memo ownership beside admission.
//! Graph G and judgment J are public CAS inventories: these are ledger-composition
//! controls, not independent CAS inventory controls. Omitting a CAS inventory item
//! would lower both the expected ceiling and reservation. A separate whole-graph
//! census would duplicate private vertex payload, interning-key and proof encodings.

use super::{TestResult, bytes, fixture};
use crate::{
    Heap, HeapLimits, LoanError,
    loan::{fault::Injection, retained_expm1::admission},
};
use nrz_bindings::{
    Budget, Limits,
    readings::algebra::{
        Cas, Exhaustion, FaultKind, Handle, Location,
        enclosure::prepared_expm1::{Counts, Policy, Request},
        limits::BudgetKind,
        proof::{ApplicationDraft, Certificate, Primitive, RegionCertificate, Statement},
        readings::AffineMap,
    },
};
use nrz_scalar::ArithmeticContext;

/// A large caller prefix makes the isolated managed peak dominate earlier symbolic services.
const PREFIX: u64 = 1_000_000;
/// Stable location for the caller's reservation and prior Work.
const AT: Location = Location::new("retained law persistent memo fixture");
/// Independent current wire schemas: tag, payload bytes, reference count (framing is 11 bytes).
/// Scalar spaces spell scalar.real/scalar.complex with two four-byte prefixes. The header
/// spells x twice, three sort bytes and two space references. Region/context are ambient/root.
/// The free expression has its header and domain. The affine definition also has two operands,
/// dimensions (16), vector counts (8), exact 1 and 0 encodings (11 and 10). Equality has context,
/// left, right and domain; its application has one statement; its conclusion has proof/statement.
const RECORDS: [(u8, usize, usize); 10] = [
    (34, 4 + 11 + 4, 0),
    (34, 4 + 14 + 4, 0),
    (0, 1 + 2 + 4 + 5 + 5 + 3 + 4 + 8 + 4, 2),
    (1, 4 + 4 + 1 + 1 + 1, 1),
    (3, 4 + 4 + 1 + 3 + 6 + 1 + 4 + 1 + 1 + 4 + 4, 2),
    (
        3,
        4 + 4 + 1 + 3 + 1 + 16 + 8 + 11 + 10 + 1 + 4 + 8 + 1 + 1 + 4 + 4,
        4,
    ),
    (8, 4 + 1 + 4 + 1 + 4, 2),
    (10, 4 + 1 + 12 + 1, 4),
    (11, 4 + 4 + 4 + 4 + 1, 1),
    (12, 1 + 4 + 4 + 4 + 4, 2),
];
/// Independent 64-bit handle ABI: one shared token and one aligned kind/serial identifier.
#[repr(C)]
struct HandleMirror {
    token: std::sync::Arc<()>,
    kind: u8,
    serial: u64,
}
/// A reading entry retains the proof handle and all 71 finite u64 limits.
#[repr(C)]
struct ReadingEntryMirror {
    proof: HandleMirror,
    limits: [u64; 71],
}
/// Enumerate `reading_memo::key` and its judgment's writes, then grow the byte backing.
fn reading_key_backing() -> usize {
    let count = size_of::<u32>();
    let serial = size_of::<u64>();
    let id = 1 + serial;
    let fields = [
        count,
        b"NRZCAS05".len(), // Representation length and spelling.
        count,
        "poly.jets-equal".len(), // Primitive length and spelling.
        serial,                  // Rule revision.
        id,
        1,
        1, // Context, layer and absent space.
        1,
        id,
        id,
        id,
        1, // Equality tag, left/right/domain and domain contract.
        count,
        count,
        count, // Empty residuals, premises and substitutions.
        1,
        count, // No certificate or imported hypotheses.
    ];
    let mut length = 0;
    let mut backing = 0;
    for field in fields {
        length += field;
        if length > backing {
            // Byte Vec starts at eight and doubles, or admits the whole pending write.
            backing = (2 * backing).max(8).max(length);
        }
    }
    assert_eq!(length, 96);
    assert_eq!(backing, 128);
    backing
}
/// All three expected memo allowances follow their public fixture records, never ledger probes.
fn memo_bytes() -> TestResult<u64> {
    assert_eq!(size_of::<HandleMirror>(), size_of::<Handle>());
    assert_eq!(size_of::<[u64; 71]>(), size_of::<Limits>());
    // Two identical ambient syntax roots, each an envelope (20) and the first four records.
    let syntax: usize = 20
        + RECORDS[..4]
            .iter()
            .map(|(_, bytes, _)| bytes + 11)
            .sum::<usize>();
    let region = 4 + 2 * (4 + syntax) + 1 + 1024 + 2 * (1 + 64);
    let reading = reading_key_backing() + size_of::<ReadingEntryMirror>() + 128;
    let witness: usize = RECORDS
        .iter()
        .map(|(_, bytes, edges)| 16 * bytes + 256 * edges)
        .sum();
    assert_eq!([region, reading, witness], [1471, 848, 8896]);
    Ok(u64::try_from(region + reading + witness)?)
}
/// Prepare exact evidence on a separate build ledger; the tested original ceiling is fixed later.
fn seed() -> TestResult<(Cas, Request, Handle, usize)> {
    let (mut cas, request) = fixture::request()?;
    let mut build = Budget::new(fixture::limits())?;
    let coordinate = &request.original.coordinates[0].coordinate;
    let reading = cas.define_reading(
        coordinate,
        coordinate,
        &AffineMap::identity(1, &mut build)?,
        &mut build,
    )?;
    let parts = crate::prepared_expm1::owned_parts(
        &mut cas,
        &request,
        [-2.0, 0.0],
        Policy::default(),
        &ArithmeticContext::new()?,
        &mut build,
        &mut Counts::default(),
        0,
    )?;
    let evidence = bytes::evidence(&parts.authority)?;
    assert_eq!(evidence, 20_968);
    Ok((cas, request, reading, evidence))
}
/// Seed every memo through its public service and verify the independent canonical record census.
fn populate(cas: &mut Cas, request: &Request, reading: &Handle, budget: &mut Budget) -> TestResult {
    cas.prepare_expm1(request, Policy::default(), budget, &mut Counts::default())?;
    let ambient = cas.admissible_region(&request.original.context)?;
    let step = ApplicationDraft {
        rule: Primitive::RegionDecide,
        context: request.original.context.clone(),
        conclusion: Statement::Subset {
            subset: ambient.clone(),
            superset: ambient,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Region(RegionCertificate {
            cuts: Vec::new(),
            membership: vec![vec![true], vec![true]],
        }),
    };
    cas.check_proof(
        &request.original.context,
        std::slice::from_ref(&step),
        budget,
    )?;
    let witness =
        cas.prove_reading_equality(&request.original.context, reading, reading, budget)?;
    let dependencies = cas.witness_dependencies(&witness, budget)?;
    let shapes: Vec<_> = dependencies
        .contents()
        .records
        .iter()
        .map(|record| (record.first().copied(), record.len()))
        .collect();
    assert_eq!(
        shapes,
        RECORDS.map(|(tag, bytes, _)| (Some(tag), bytes + 11))
    );
    Ok(())
}
/// N=P+(G-J)+M+S includes both real graph registrations and every persistent memo inventory.
#[test]
fn retained_law_persistent_memo_ledger_composition_boundary() -> TestResult {
    let (mut exemplar, request, reading, evidence) = seed()?;
    populate(
        &mut exemplar,
        &request,
        &reading,
        &mut Budget::new(fixture::limits())?,
    )?;
    let graph = exemplar.counts().payload_bytes;
    let judgments = exemplar.retained_judgment_bytes();
    let [header, use_header, row] = bytes::headers();
    let staged = u64::try_from(header + evidence + use_header + 2 * row)?;
    let persistent = 2 * (judgments + 64) + memo_bytes()?;
    let boundary = PREFIX + (graph - judgments) + persistent + staged;
    for short in [0, 1] {
        let (mut cas, request, reading, _) = seed()?;
        let (mut other, other_request, _, _) = seed()?;
        let mut budget = Budget::new(Limits {
            staged_payload_bytes: boundary - short,
            ..fixture::limits()
        })?;
        other.prepare_expm1(
            &other_request,
            Policy::default(),
            &mut budget,
            &mut Counts::default(),
        )?;
        assert_eq!(other.retained_judgment_bytes(), judgments);
        populate(&mut cas, &request, &reading, &mut budget)?;
        assert_eq!(cas.counts().payload_bytes, graph);
        budget.work(7, AT)?;
        let before = budget.usage().work;
        let mut heap = Heap::new(HeapLimits::default())?;
        let injection = Injection::after(usize::MAX);
        let result = budget.with_retained_payload(PREFIX, AT, |budget| {
            cas.with_retained_graph(budget, |budget| {
                admission::shells(
                    &mut heap,
                    17,
                    u64::try_from(evidence).map_err(|_| LoanError::ByteOverflow)?,
                    budget,
                    fixture::seal(),
                )
            })
        });
        if short == 0 {
            drop(result?);
            assert_eq!(Injection::attempts(), 3);
            assert_eq!(Injection::prefix(), [3, 1, 0]);
        } else {
            let LoanError::Budget(fault) = result.err().ok_or("combined memo boundary")? else {
                return Err("typed combined memo refusal".into());
            };
            assert_eq!(
                fault.kind,
                FaultKind::Exhausted {
                    reason: Exhaustion::Limit,
                    budget: BudgetKind::StagedPayloadBytes,
                    limit: persistent + staged - 1,
                    needed: persistent + staged,
                }
            );
            assert_eq!(Injection::attempts(), 0);
            assert_eq!(heap.loans.debits.capacity(), 0);
        }
        assert!(heap.loans.debits.iter().all(Option::is_none));
        assert!(budget.usage().work > before);
        assert_eq!(budget.limits().staged_payload_bytes, boundary - short);
        drop(injection);
    }
    Ok(())
}
