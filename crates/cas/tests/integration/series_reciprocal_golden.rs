// SPDX-License-Identifier: AGPL-3.0-only
//! Literal mode-seven wire oracles independent of the producer and production encoder.

use super::series_golden::{
    Part, Record, bytes, count, encode_records, expression, join, name, number, object, records,
};
use crate::testing::common::{TestResult, default_budget};
use crate::testing::series_proof::{fixture, object as admit_object};
use nrz_cas::proof::Statement;
use nrz_cas::series::{SeriesOperation, SeriesStatement};

/// Encode a signed one-byte rational literal without the production codec.
fn q(n: i8, d: u8) -> Vec<u8> {
    number(n < 0, n.unsigned_abs(), d)
}

/// Write complete ordered primitive names and versions for this literal witness.
fn closure(keys: &[&str]) -> Vec<u8> {
    let mut out = count(u32::try_from(keys.len()).unwrap_or(u32::MAX));
    for key in keys {
        out.extend(name(key));
        out.extend(count(1));
    }
    out
}

/// Keep the literal record kind and logical layer beside its independent fields.
const fn record(key: &'static str, tag: u8, parts: Vec<Part>) -> Record {
    Record {
        name: key,
        tag,
        layer: 0,
        parts,
    }
}

/// Encode an original scalar node and its ordered operands.
fn node(key: &'static str, operator: u8, operands: &[&'static str]) -> Record {
    let mut parts = vec![
        Part::Ref("header"),
        bytes(join(&[count(0), vec![1]])),
        Part::Ref("real"),
        bytes(join(&[
            vec![0, 1, 1, operator, 0],
            count(u32::try_from(operands.len()).unwrap_or(u32::MAX)),
        ])),
    ];
    parts.extend(operands.iter().map(|key| Part::Ref(key)));
    parts.push(bytes(join(&[vec![0, 0], count(0)])));
    Record {
        name: key,
        tag: 3,
        layer: 1,
        parts,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Independent records expose every grammar field"
)]
/// Keep statement, certificate, proof slots and retained conditions in the literal grammar.
fn claim(
    values: &mut Vec<Record>,
    names: [&'static str; 3],
    key: &'static str,
    index: u32,
    statement: Vec<Part>,
    evidence: Vec<u8>,
    premises: &[&'static str],
    hypotheses: &[u32],
    keys: &[&str],
) {
    let [statement_name, certificate_name, conclusion] = names;
    let mut parts = vec![
        bytes(join(&[count(index), name(key), count(1)])),
        Part::Ref("context"),
    ];
    parts.extend(statement);
    parts.push(bytes(count(
        u32::try_from(hypotheses.len()).unwrap_or(u32::MAX),
    )));
    for slot in hypotheses {
        parts.extend([Part::Ref("context"), bytes(count(*slot))]);
    }
    parts.push(bytes(join(&[count(0), closure(keys)])));
    values.push(record(statement_name, 39, parts));
    let mut parts = vec![
        Part::Ref(statement_name),
        bytes(join(&[
            vec![evidence[0]],
            count(u32::try_from(premises.len()).unwrap_or(u32::MAX)),
        ])),
    ];
    parts.extend(premises.iter().map(|key| Part::Ref(key)));
    parts.push(bytes(evidence));
    values.push(record(certificate_name, 40, parts));
    let mut parts = vec![
        Part::Ref(statement_name),
        Part::Ref(certificate_name),
        bytes(count(u32::try_from(premises.len()).unwrap_or(u32::MAX))),
    ];
    parts.extend(premises.iter().map(|key| Part::Ref(key)));
    parts.push(bytes(closure(keys)));
    values.push(record(conclusion, 41, parts));
}

/// Literal rational products and prefix sums for every pair, including all zeros.
fn rows(
    left: &[(i8, u8)],
    right: &[(i8, u8)],
    products: &[(i8, u8)],
    sums: &[(i8, u8)],
) -> Vec<u8> {
    let mut out = count(21);
    let mut index = 0;
    for degree in 0..6 {
        for i in 0..=degree {
            out.extend(count(u32::try_from(degree).unwrap_or(u32::MAX)));
            out.extend(count(u32::try_from(i).unwrap_or(u32::MAX)));
            out.extend(count(u32::try_from(degree - i).unwrap_or(u32::MAX)));
            for (n, d) in [left[i], right[degree - i], products[index], sums[index]] {
                out.extend(q(n, d));
            }
            index += 1;
        }
    }
    out
}

/// The complete sine/cosine, inverse and product graph is authored without CAS constructors.
#[expect(
    clippy::too_many_lines,
    reason = "The complete independent mixed grammar is kept together"
)]
fn quotient_golden(residual: bool, renamed: bool) -> TestResult<Vec<u8>> {
    let keys = [
        "ctx.hypothesis",
        "series.mul",
        "series.reciprocal",
        "series.source",
    ];
    let mut values = super::series_residual_golden::frame(false, false);
    values.retain(|r| {
        r.name != "context"
            && r.name != "equation"
            && (residual || !matches!(r.name, "problem" | "unknowns" | "domain" | "y"))
    });
    values.extend([
        node("sine", 58, &["x"]),
        node("cosine", 59, &["x"]),
        node("secant", 8, &["one", "cosine"]),
        node("tangent", 8, &["sine", "cosine"]),
    ]);
    let predicates = [
        "sin-defined",
        "cos-defined",
        "sec-defined",
        "tan-defined",
        "cos-nonzero",
    ];
    let sources = ["sine", "cosine", "secant", "tangent", "cosine"];
    let guard_statements = [
        "sin-guard-statement",
        "cos-guard-statement",
        "sec-guard-statement",
        "tan-guard-statement",
        "nz-guard-statement",
    ];
    let guards = [
        "sin-guard",
        "cos-guard",
        "sec-guard",
        "tan-guard",
        "nz-guard",
    ];
    let assumption_keys = [
        "quotient-guard-0",
        "quotient-guard-1",
        "quotient-guard-2",
        "quotient-guard-3",
        "quotient-guard-4",
    ];
    let mut context = vec![
        Part::Ref("header"),
        bytes(vec![0]),
        Part::Ref("ambient"),
        bytes(join(&[vec![0], count(5)])),
    ];
    for (i, ((predicate, source), assumption)) in predicates
        .iter()
        .zip(sources)
        .zip(assumption_keys)
        .enumerate()
    {
        values.push(record(
            predicate,
            7,
            vec![
                Part::Ref("header"),
                bytes(join(&[count(0), vec![u8::from(i == 4)], count(1)])),
                Part::Ref(source),
            ],
        ));
        context.extend([bytes(name(assumption)), Part::Ref(predicate)]);
        values.push(record(
            guard_statements[i],
            10,
            vec![
                Part::Ref("context"),
                bytes(vec![1]),
                Part::Ref("ambient"),
                Part::Ref(predicate),
            ],
        ));
        values.push(record(
            guards[i],
            11,
            vec![
                bytes(count(0)),
                Part::Ref(guard_statements[i]),
                bytes(join(&[count(0), count(0), vec![2]])),
                Part::Ref("context"),
                bytes(join(&[count(u32::try_from(i)?), count(0)])),
            ],
        ));
    }
    values.push(record("context", 8, context));
    for (key, n, d) in [
        ("minus-half", -1, 2),
        ("minus-sixth", -1, 6),
        ("one-24", 1, 24),
        ("one-120", 1, 120),
        ("positive-half", 1, 2),
        ("five-24", 5, 24),
        ("third", 1, 3),
        ("two-15", 2, 15),
    ] {
        values.push(expression(key, join(&[vec![0], q(n, d)])));
    }
    values.extend([
        object(
            "S",
            &["zero", "one", "zero", "minus-sixth", "zero", "one-120"],
        ),
        object(
            "C",
            &["one", "zero", "minus-half", "zero", "one-24", "zero"],
        ),
        object(
            "Qinv",
            &["one", "zero", "positive-half", "zero", "five-24", "zero"],
        ),
        object("T", &["zero", "one", "zero", "third", "zero", "two-15"]),
    ]);
    let s = [(0, 1), (1, 1), (0, 1), (-1, 6), (0, 1), (1, 120)];
    let c = [(1, 1), (0, 1), (-1, 2), (0, 1), (1, 24), (0, 1)];
    let inverse = [(1, 1), (0, 1), (1, 2), (0, 1), (5, 24), (0, 1)];
    for (names, source, output, selector, guard, hypothesis) in [
        (
            ["sin-statement", "sin-certificate", "sin-proof"],
            "sine",
            "S",
            3,
            "sin-guard",
            0,
        ),
        (
            ["cos-statement", "cos-certificate", "cos-proof"],
            "cosine",
            "C",
            4,
            "cos-guard",
            1,
        ),
    ] {
        let mut evidence = vec![10, 0, selector];
        for coeffs in [s, c] {
            evidence.extend(count(6));
            for (n, d) in coeffs {
                evidence.extend(q(n, d));
            }
        }
        evidence.extend(count(5));
        for i in 0..5 {
            evidence.extend(count(i));
            evidence.extend(q(i8::try_from(i + 1)?, 1));
            evidence.extend(q(1, u8::try_from(i + 1)?));
        }
        claim(
            &mut values,
            names,
            "series.source",
            if residual { 4 } else { 3 },
            vec![
                bytes(vec![10, 0, selector]),
                Part::Ref(source),
                Part::Ref("ambient"),
                bytes(vec![2]),
                Part::Ref(output),
            ],
            evidence,
            &[guard],
            &[hypothesis],
            &[keys[0], keys[3]],
        );
    }
    let zero = (0, 1);
    let inverse_products = [
        (1, 1),
        zero,
        zero,
        (1, 2),
        zero,
        (-1, 2),
        zero,
        zero,
        zero,
        zero,
        (5, 24),
        zero,
        (-1, 4),
        zero,
        (1, 24),
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
    ];
    let inverse_sums = [
        (1, 1),
        zero,
        zero,
        (1, 2),
        (1, 2),
        zero,
        zero,
        zero,
        zero,
        zero,
        (5, 24),
        (5, 24),
        (-1, 24),
        (-1, 24),
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
    ];
    claim(
        &mut values,
        ["inv-statement", "inv-certificate", "inv-proof"],
        keys[2],
        2,
        vec![
            bytes(vec![7, 0]),
            Part::Ref("C"),
            bytes(vec![0]),
            Part::Ref("Qinv"),
        ],
        join(&[
            vec![7, 0],
            q(1, 1),
            q(1, 1),
            rows(&c, &inverse, &inverse_products, &inverse_sums),
        ]),
        &[],
        &[],
        &[keys[2]],
    );
    claim(
        &mut values,
        ["sec-statement", "sec-certificate", "sec-proof"],
        keys[2],
        2,
        vec![
            bytes(vec![7, 1, 0]),
            Part::Ref("secant"),
            Part::Ref("ambient"),
            bytes(vec![2]),
            Part::Ref("Qinv"),
        ],
        join(&[vec![7, 1, 0], count(0), count(0), count(0)]),
        &["cos-proof", "inv-proof", "nz-guard", "sec-guard"],
        &[1, 2, 4],
        &[keys[0], keys[2], keys[3]],
    );
    let products = [
        zero,
        zero,
        (1, 1),
        zero,
        zero,
        zero,
        zero,
        (1, 2),
        zero,
        (-1, 6),
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
        (5, 24),
        zero,
        (-1, 12),
        zero,
        (1, 120),
    ];
    let sums = [
        zero,
        zero,
        (1, 1),
        zero,
        zero,
        zero,
        zero,
        (1, 2),
        (1, 2),
        (1, 3),
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
        (5, 24),
        (5, 24),
        (1, 8),
        (1, 8),
        (2, 15),
    ];
    claim(
        &mut values,
        ["mul-statement", "mul-certificate", "mul-proof"],
        keys[1],
        1,
        vec![
            bytes(vec![3, 0]),
            Part::Ref("S"),
            Part::Ref("Qinv"),
            bytes(vec![0]),
            Part::Ref("T"),
            bytes(join(&[count(6), vec![1]])),
        ],
        join(&[
            vec![3, 0],
            count(6),
            vec![1],
            rows(&s, &inverse, &products, &sums),
        ]),
        &[],
        &[],
        &[keys[1]],
    );
    claim(
        &mut values,
        [
            "quot-statement",
            "quot-certificate",
            if residual { "quot-proof" } else { "conclusion" },
        ],
        keys[1],
        1,
        vec![
            bytes(vec![3, 1, 1]),
            Part::Ref("tangent"),
            Part::Ref("ambient"),
            bytes(vec![2]),
            Part::Ref("T"),
        ],
        join(&[vec![3, 1, 1], count(0), count(0), count(0)]),
        &[
            "sin-proof",
            "cos-proof",
            "sec-proof",
            "mul-proof",
            "nz-guard",
            "tan-guard",
        ],
        &[0, 1, 2, 3, 4],
        &keys,
    );
    if renamed {
        let mut replacements = 0;
        for record in &mut values {
            if !matches!(record.name, "header" | "x" | "coordinates") {
                continue;
            }
            for part in &mut record.parts {
                let Part::Bytes(value) = part else {
                    continue;
                };
                for start in 0..value.len().saturating_sub(4) {
                    if value[start..start + 4] == [1, 0, 0, 0] {
                        let slot = &mut value[start + 4];
                        if let Some(to) = match *slot {
                            b'x' => Some(b'u'),
                            b'y' => Some(b'v'),
                            b'z' => Some(b'w'),
                            _ => None,
                        } {
                            *slot = to;
                            replacements += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(replacements, 8);
    }
    if residual {
        residual_records(&mut values)?;
        encode_records(
            &values,
            closure(&[
                "ctx.hypothesis",
                "series.mul",
                "series.reciprocal",
                "series.residual",
                "series.source",
                "solve.member",
            ]),
        )
    } else {
        encode_records(&values, closure(&keys))
    }
}

/// The full mixed source payload and retained conditions have an independent byte oracle.
/// ´test:integration:series-reciprocal-mixed-wire-golden´
#[test]
fn reciprocal_mixed_wire_golden() -> TestResult {
    for renamed in [false, true] {
        crate::conformance::series_reciprocal::quotient_payload_control(
            &quotient_golden(false, renamed)?,
            renamed,
        )?;
    }
    Ok(())
}

/// Extend the independently authored source DAG with nine exact residual rows and Member.
#[expect(
    clippy::too_many_lines,
    reason = "The complete independent grammar and its exact refusal controls remain adjacent"
)]
fn residual_records(values: &mut Vec<Record>) -> TestResult {
    let mut derivative = node("dy", 12, &["y"]);
    let Part::Bytes(operator) = &mut derivative.parts[3] else {
        return Err("operator".into());
    };
    *operator = join(&[vec![0, 1, 1, 12], name("x"), vec![0], count(1)]);
    let mut square = node("square", 9, &["y"]);
    let Part::Bytes(operator) = &mut square.parts[3] else {
        return Err("operator".into());
    };
    *operator = join(&[
        vec![0, 1, 1, 9, 0],
        2_u64.to_le_bytes().to_vec(),
        vec![0],
        count(1),
    ]);
    values.extend([
        derivative,
        square,
        node("dy-minus-one", 6, &["dy", "one"]),
        node("R", 6, &["dy-minus-one", "square"]),
    ]);
    values.push(record(
        "zero-equation",
        3,
        vec![
            Part::Ref("header"),
            bytes(join(&[count(0), vec![0, 0, 2, 17, 6, 0], count(2)])),
            Part::Ref("zero"),
            Part::Ref("zero"),
            bytes(join(&[vec![0, 0], count(0)])),
        ],
    ));
    let mut equation = vec![
        Part::Ref("header"),
        bytes(join(&[count(0), vec![8], count(5)])),
    ];
    equation.extend((0..5).map(|_| Part::Ref("zero-equation")));
    values.push(record("equation", 7, equation));
    values.push(object(
        "residual",
        &["zero", "zero", "zero", "zero", "zero"],
    ));
    let zero = (0, 1);
    let t = [zero, (1, 1), zero, (1, 3), zero];
    let square = [zero, zero, (1, 1), zero, (2, 3)];
    let trace = [
        t,
        [(1, 1), zero, (1, 1), zero, (2, 3)],
        [(1, 1), zero, zero, zero, zero],
        square,
        t,
        t,
        square,
        square,
        [zero; 5],
    ];
    let mut evidence = join(&[vec![11, 0, 0], count(9)]);
    for (i, row) in trace.into_iter().enumerate() {
        evidence.extend(count(u32::try_from(i)?));
        evidence.extend(count(5));
        for (n, d) in row {
            evidence.extend(q(n, d));
        }
    }
    claim(
        values,
        [
            "residual-statement",
            "residual-certificate",
            "residual-proof",
        ],
        "series.residual",
        3,
        vec![
            bytes(vec![11, 0, 3, 0, 3, 1, 1]),
            Part::Ref("tangent"),
            Part::Ref("ambient"),
            bytes(vec![2]),
            Part::Ref("T"),
            Part::Ref("R"),
            Part::Ref("y"),
            Part::Ref("residual"),
            Part::Ref("problem"),
            bytes(join(&[count(1), count(5)])),
        ],
        evidence,
        &["quot-proof"],
        &[0, 1, 2, 3, 4],
        &[
            "ctx.hypothesis",
            "series.mul",
            "series.reciprocal",
            "series.residual",
            "series.source",
        ],
    );
    values.push(record(
        "tuple",
        14,
        vec![
            Part::Ref("header"),
            bytes(count(0)),
            Part::Ref("unknowns"),
            bytes(count(0)),
        ],
    ));
    values.push(record(
        "solve-statement",
        10,
        vec![
            Part::Ref("context"),
            bytes(vec![25, 0]),
            Part::Ref("problem"),
            Part::Ref("tuple"),
            Part::Ref("ambient"),
        ],
    ));
    values.push(record(
        "solve-proof",
        11,
        vec![
            bytes(count(5)),
            Part::Ref("solve-statement"),
            bytes(count(1)),
            Part::Ref("residual-proof"),
            bytes(join(&[count(0), vec![12, 4]])),
            Part::Ref("problem"),
            Part::Ref("tuple"),
            Part::Ref("ambient"),
            bytes(join(&[count(0), count(0)])),
        ],
    ));
    let mut conclusion = vec![
        bytes(vec![1]),
        Part::Ref("solve-proof"),
        Part::Ref("solve-statement"),
        bytes(join(&[count(0), count(5)])),
    ];
    for i in 0_u64..5 {
        conclusion.extend([Part::Ref("context"), bytes(i.to_le_bytes().to_vec())]);
    }
    values.push(record("conclusion", 12, conclusion));
    Ok(())
}

/// All five zero equations and the original quotient descriptor survive independent paired replay.
/// ´test:integration:series-reciprocal-residual-wire-golden´
#[test]
fn reciprocal_residual_wire_golden() -> TestResult {
    crate::conformance::series_reciprocal::paired_payload_control(&quotient_golden(true, false)?)
}

/// Choose either complete independently authored positive object payload.
fn golden(single: bool) -> TestResult<Vec<u8>> {
    object_golden(single, false)
}

/// Retain every rational row of the complete object wire oracle.
#[expect(
    clippy::too_many_lines,
    reason = "The complete independent grammar and its exact refusal controls remain adjacent"
)]
fn object_golden(single: bool, nonunit: bool) -> TestResult<Vec<u8>> {
    let mut values = records();
    values.retain(|r| {
        matches!(
            r.name,
            "real" | "complex" | "header" | "ambient" | "context" | "Q" | "x" | "zero" | "two"
        ) || (!single && r.name == "one")
    });
    values.push(expression("positive-half", join(&[vec![0], q(1, 2)])));
    if !single {
        for (key, n, d) in [
            ("negative-quarter", -1, 4),
            ("eighth", 1, 8),
            ("negative-sixteenth", -1, 16),
        ] {
            values.push(expression(key, join(&[vec![0], q(n, d)])));
        }
    }
    values.push(object(
        "input",
        if nonunit {
            &["zero"]
        } else if single {
            &["two"]
        } else {
            &["two", "one", "zero", "zero"]
        },
    ));
    values.push(object(
        "output",
        if single {
            &["positive-half"]
        } else {
            &[
                "positive-half",
                "negative-quarter",
                "eighth",
                "negative-sixteenth",
            ]
        },
    ));
    let manifest = join(&[count(1), name("series.reciprocal"), count(1)]);
    values.push(Record {
        name: "statement",
        tag: 39,
        layer: 0,
        parts: vec![
            bytes(join(&[count(0), name("series.reciprocal"), count(1)])),
            Part::Ref("context"),
            bytes(vec![7, 0]),
            Part::Ref("input"),
            bytes(vec![0]),
            Part::Ref("output"),
            bytes(join(&[count(0), count(0), manifest.clone()])),
        ],
    });
    let table = [
        (0, 0, 0, (2, 1), (1, 2), (1, 1), (1, 1)),
        (1, 0, 1, (2, 1), (-1, 4), (-1, 2), (-1, 2)),
        (1, 1, 0, (1, 1), (1, 2), (1, 2), (0, 1)),
        (2, 0, 2, (2, 1), (1, 8), (1, 4), (1, 4)),
        (2, 1, 1, (1, 1), (-1, 4), (-1, 4), (0, 1)),
        (2, 2, 0, (0, 1), (1, 2), (0, 1), (0, 1)),
        (3, 0, 3, (2, 1), (-1, 16), (-1, 8), (-1, 8)),
        (3, 1, 2, (1, 1), (1, 8), (1, 8), (0, 1)),
        (3, 2, 1, (0, 1), (-1, 4), (0, 1), (0, 1)),
        (3, 3, 0, (0, 1), (1, 2), (0, 1), (0, 1)),
    ];
    let mut evidence = join(&[
        vec![7, 0],
        q(2, 1),
        q(1, 2),
        count(if single { 1 } else { 10 }),
    ]);
    for (degree, i, j, a, b, p, s) in table.iter().take(if single { 1 } else { 10 }) {
        evidence.extend(join(&[
            count(*degree),
            count(*i),
            count(*j),
            q(a.0, a.1),
            q(b.0, b.1),
            q(p.0, p.1),
            q(s.0, s.1),
        ]));
    }
    values.push(Record {
        name: "certificate",
        tag: 40,
        layer: 0,
        parts: vec![
            Part::Ref("statement"),
            bytes(join(&[vec![7], count(0), evidence])),
        ],
    });
    values.push(Record {
        name: "conclusion",
        tag: 41,
        layer: 0,
        parts: vec![
            Part::Ref("statement"),
            Part::Ref("certificate"),
            bytes(join(&[count(0), manifest.clone()])),
        ],
    });
    encode_records(&values, manifest)
}

/// Full portable payloads and used manifests are literal oracles, including every zero row.
/// ´test:integration:series-reciprocal-hand-wire-goldens´
#[test]
fn reciprocal_hand_wire_goldens() -> TestResult {
    payload_control()
}

/// A complete independently encoded nonunit record reaches the mathematical refusal before its false inverse.
/// ´test:integration:series-reciprocal-nonunit-wire´
#[test]
fn reciprocal_nonunit_wire() -> TestResult {
    let wire = object_golden(true, true)?;
    crate::conformance::series_reciprocal::nonunit_wire_control(&wire)?;
    let records = crate::testing::answer::records(&wire)?;
    let evidence = records
        .iter()
        .find(|(tag, _)| *tag == 40)
        .ok_or("reciprocal certificate")?
        .1
        .start
        + 15;
    // Neither an inverse magnitude nor the row vector may be decoded before zero is observed.
    for offset in [evidence + 14, evidence + 24] {
        let malicious = changed(&wire, offset, 1, 255);
        crate::conformance::series_reciprocal::nonunit_wire_control(&malicious)?;
    }
    Ok(())
}

/// Every changed byte is first checked against the independently authored original grammar.
fn changed(bytes: &[u8], offset: usize, old: u8, new: u8) -> Vec<u8> {
    assert_eq!(bytes[offset], old);
    let mut result = bytes.to_vec();
    result[offset] = new;
    result
}

/// New object and source payloads retain the reserved grammar and exact malformed-byte refusals.
/// ´test:integration:series-reciprocal-wire-mutations´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "The complete independent grammar and its exact refusal controls remain adjacent"
)]
fn reciprocal_wire_mutations() -> TestResult {
    use crate::testing::answer::{index, records as wire_records};
    use nrz_cas::{FaultKind, InvalidCode as I};
    let mut mutations = Vec::new();
    for source in [false, true] {
        let wire = if source {
            quotient_golden(false, false)?
        } else {
            golden(true)?
        };
        let records = wire_records(&wire)?;
        let mut manifest_cursor = 8;
        let manifest_count = index(&wire, &mut manifest_cursor)?;
        let mut omitted = None;
        for _ in 0..manifest_count {
            let start = manifest_cursor;
            let length = usize::try_from(index(&wire, &mut manifest_cursor)?)?;
            let reciprocal =
                &wire[manifest_cursor..manifest_cursor + length] == b"series.reciprocal";
            manifest_cursor += length;
            assert_eq!(index(&wire, &mut manifest_cursor)?, 1);
            if reciprocal {
                omitted = Some(start..manifest_cursor);
            }
        }
        let mut missing = wire.clone();
        missing.drain(omitted.ok_or("reciprocal manifest dependency")?);
        missing[8..12].copy_from_slice(&(manifest_count - 1).to_le_bytes());
        mutations.push((
            missing,
            if source {
                I::SeriesUnsupported
            } else {
                I::Rule
            },
        ));
        for (record_index, (tag, range)) in records.iter().enumerate() {
            if *tag != 39 {
                continue;
            }
            let mut cursor = range.start + 10;
            let key_len = usize::try_from(index(&wire, &mut cursor)?)?;
            let key = &wire[cursor..cursor + key_len];
            if key != b"series.reciprocal" && !(source && key == b"series.mul") {
                continue;
            }
            cursor += key_len;
            let version = cursor;
            assert_eq!(index(&wire, &mut cursor)?, 1);
            index(&wire, &mut cursor)?;
            let mode = wire[cursor];
            let form = wire[cursor + 1];
            if source && form != 1 {
                continue;
            }
            assert!(matches!((mode, form), (7, 0 | 1) | (3, 1)));
            mutations.push((changed(&wire, cursor, mode, 6), I::SeriesUnsupported));
            mutations.push((changed(&wire, cursor + 1, form, 2), I::SeriesUnsupported));
            mutations.push((changed(&wire, version, 1, 2), I::SeriesUnsupported));
            let result = cursor + if source { 11 } else { 6 };
            mutations.push((
                changed(&wire, result, if source { 2 } else { 0 }, 3),
                I::SeriesUnsupported,
            ));
            if source {
                mutations.push((
                    changed(&wire, cursor + 2, u8::from(mode != 7), 2),
                    I::SeriesUnsupported,
                ));
            }
            let certificate = records
                .iter()
                .find(|(tag, r)| {
                    *tag == 40
                        && u32::from_le_bytes(
                            wire[r.start + 6..r.start + 10]
                                .try_into()
                                .unwrap_or([255; 4]),
                        ) == u32::try_from(record_index).unwrap_or(u32::MAX)
                })
                .ok_or("certificate")?
                .1
                .clone();
            let slots = if source {
                if mode == 7 { 4 } else { 6 }
            } else {
                0
            };
            mutations.push((changed(&wire, certificate.start + 10, mode, 255), I::Rule));
            mutations.push((
                changed(&wire, certificate.start + 11, slots, slots + 1),
                I::Rule,
            ));
            let evidence = certificate.start + 15 + 4 * usize::from(slots);
            mutations.push((changed(&wire, evidence + 1, form, 2), I::SeriesUnsupported));
            if !source {
                assert_eq!(&wire[evidence..evidence + 2], &[7, 0]);
                assert_eq!(&wire[evidence + 2..evidence + 13], q(2, 1));
                mutations.push((changed(&wire, evidence + 12, 1, 0), I::Encoding));
                mutations.push((changed(&wire, evidence + 7, 2, 0), I::Encoding));
                mutations.push((changed(&wire, evidence + 24, 1, 0), I::Rule));
            }
            let mut reference = wire.clone();
            let at = cursor + if source { 3 } else { 2 };
            let mut reference_cursor = at;
            let original = index(&wire, &mut reference_cursor)?;
            assert!(usize::try_from(original)? < record_index);
            reference[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            mutations.push((reference, I::Encoding));
        }
        let mut trailing = wire.clone();
        trailing.push(0);
        mutations.push((trailing, I::Encoding));
        let mut truncated = wire;
        truncated.pop();
        mutations.push((truncated, I::Encoding));
    }
    for (case, (bytes, kind)) in mutations.into_iter().enumerate() {
        for reversed in [false, true] {
            let mut f =
                crate::testing::series_residual::Fixture::new(&["0"], "0", reversed, false)?;
            let mut budget = nrz_cas::limits::Budget::new(nrz_cas::limits::Limits {
                work: 20_000_000,
                replay_work: 20_000_000,
                ..nrz_cas::limits::Limits::default()
            })?;
            let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                f.cas
                    .inspect(&bytes, &mut budget)
                    .expect_err("mutated grammar")
                    .kind,
                FaultKind::Invalid(kind),
                "case {case}"
            );
            assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
        }
    }
    Ok(())
}

/// Complete independent payloads control the newly admitted object mode.
pub(super) fn payload_control() -> TestResult {
    for single in [true, false] {
        let bytes = golden(single)?;
        for reversed in [false, true] {
            let mut f = fixture(0, reversed)?;
            let input = admit_object(
                &mut f.base,
                &f.field,
                if single {
                    &["2"]
                } else {
                    &["2", "1", "0", "0"]
                },
            )?;
            let output = admit_object(
                &mut f.base,
                &f.field,
                if single {
                    &["1/2"]
                } else {
                    &["1/2", "-1/4", "1/8", "-1/16"]
                },
            )?;
            f.expected.statement = Statement::Series(Box::new(SeriesStatement::Reciprocal {
                input: input.clone(),
                output,
            }));
            f.base
                .cas
                .replay(&bytes, &f.expected, &mut default_budget()?)?;
            let produced = f.base.cas.compute_series(
                &SeriesOperation::Reciprocal { input },
                &[],
                &mut default_budget()?,
            )?;
            assert_eq!(produced.judgment(), &f.expected);
            assert_eq!(
                f.base
                    .cas
                    .export_witness(&produced, &mut default_budget()?)?,
                bytes
            );
        }
    }
    Ok(())
}
