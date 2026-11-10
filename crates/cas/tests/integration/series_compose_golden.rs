// SPDX-License-Identifier: AGPL-3.0-only
//! Independent object and restriction DAGs use only hand-stated wire fields.

use super::series_golden::{
    Part, Record, bytes, count, encode_records, expression, join, name, number, object, records,
};
use crate::testing::common::{TestResult, default_budget};
use crate::testing::series_compose::fixture;

/// The complete ordered portable claim, certificate and checked conclusion.
#[expect(
    clippy::too_many_arguments,
    reason = "Each independent wire record states all typed fields explicitly"
)]
fn claim(
    values: &mut Vec<Record>,
    names: [&'static str; 3],
    key: &'static str,
    index: u32,
    statement: Vec<Part>,
    rows: Vec<u8>,
    premises: &[&'static str],
    closure: &[u8],
) {
    let [statement_name, certificate_name, conclusion] = names;
    let mut parts = vec![
        bytes(join(&[count(index), name(key), count(1)])),
        Part::Ref("context"),
    ];
    parts.extend(statement);
    parts.push(bytes(join(&[count(0), count(0), closure.to_vec()])));
    values.push(Record {
        name: statement_name,
        tag: 39,
        layer: 0,
        parts,
    });
    let mut parts = vec![
        Part::Ref(statement_name),
        bytes(join(&[
            vec![rows[0]],
            count(u32::try_from(premises.len()).expect("fixture arity")),
        ])),
    ];
    parts.extend(premises.iter().map(|p| Part::Ref(p)));
    parts.push(bytes(rows));
    values.push(Record {
        name: certificate_name,
        tag: 40,
        layer: 0,
        parts,
    });
    let mut parts = vec![
        Part::Ref(statement_name),
        Part::Ref(certificate_name),
        bytes(count(u32::try_from(premises.len()).expect("fixture arity"))),
    ];
    parts.extend(premises.iter().map(|p| Part::Ref(p)));
    parts.push(bytes(closure.to_vec()));
    values.push(Record {
        name: conclusion,
        tag: 41,
        layer: 0,
        parts,
    });
}

/// Order-one substitution with two explicit higher-order restrictions when selected.
#[expect(
    clippy::too_many_lines,
    reason = "The independent golden retains its complete hand-authored record grammar"
)]
fn golden_inner(restricted: bool, nonzero: bool) -> TestResult<Vec<u8>> {
    let mut values = records();
    values.retain(|r| {
        matches!(
            r.name,
            "real" | "complex" | "header" | "ambient" | "context" | "Q" | "x" | "zero" | "one"
        )
    });
    values.push(expression("seven", join(&[vec![0], number(false, 7, 1)])));
    if nonzero {
        values.push(expression("nonzero", join(&[vec![0], number(false, 2, 3)])));
    }
    values.push(object(
        "outer",
        if restricted {
            &["seven", "one"]
        } else {
            &["seven"]
        },
    ));
    values.push(object(
        "inner",
        if restricted {
            &["zero", "one"]
        } else if nonzero {
            &["nonzero"]
        } else {
            &["zero"]
        },
    ));
    // The output is the same mathematical object as the restricted outer input.
    if restricted {
        values.push(object("output", &["seven"]));
    }
    let closure = if restricted {
        join(&[
            count(2),
            name("series.compose"),
            count(1),
            name("series.truncate"),
            count(1),
        ])
    } else {
        join(&[count(1), name("series.compose"), count(1)])
    };
    if restricted {
        values.push(object("inner-short", &["zero"]));
        let restriction_closure = join(&[count(1), name("series.truncate"), count(1)]);
        for (names, input, output, value) in [
            (
                ["outer-statement", "outer-certificate", "outer-proof"],
                "outer",
                "output",
                7,
            ),
            (
                ["inner-statement", "inner-certificate", "inner-proof"],
                "inner",
                "inner-short",
                0,
            ),
        ] {
            claim(
                &mut values,
                names,
                "series.truncate",
                1,
                vec![
                    bytes(vec![0, 0]),
                    Part::Ref(input),
                    bytes(vec![0]),
                    Part::Ref(output),
                ],
                join(&[vec![0, 0], count(1), number(false, value, 1)]),
                &[],
                &restriction_closure,
            );
        }
    }
    let rows = join(&[
        vec![5, 0],
        count(1),
        vec![u8::from(!restricted)],
        number(false, 0, 1),
        count(1),
        number(false, 1, 1),
        count(0),
        count(1),
        count(0),
        count(0),
        count(0),
        number(false, 7, 1),
        number(false, 1, 1),
        number(false, 7, 1),
        number(false, 7, 1),
    ]);
    claim(
        &mut values,
        ["series-statement", "series-certificate", "conclusion"],
        "series.compose",
        0,
        vec![
            bytes(vec![5, 0]),
            Part::Ref("outer"),
            Part::Ref("inner"),
            bytes(vec![0]),
            Part::Ref(if restricted { "output" } else { "outer" }),
            bytes(join(&[count(1), vec![u8::from(!restricted)]])),
        ],
        rows,
        if restricted {
            &["outer-proof", "inner-proof"]
        } else {
            &[]
        },
        &closure,
    );
    // One is not a dependency of the unrestricted objects or certificate graph.
    if !restricted {
        values.retain(|r| r.name != "one");
    }
    encode_records(&values, closure)
}

/// Preserve the independently authored admissible object and restriction witnesses.
fn golden(restricted: bool) -> TestResult<Vec<u8>> {
    golden_inner(restricted, false)
}

/// Independently authored order-one import with an inadmissible inner constant.
pub(super) fn nonzero_import() -> TestResult<Vec<u8>> {
    golden_inner(false, true)
}

/// Object-only and ordered restriction goldens replay in both receiving allocation orders.
/// ´test:integration:compose-independent-goldens´
#[test]
fn compose_independent_goldens() -> TestResult {
    for restricted in [false, true] {
        let bytes = golden(restricted)?;
        for reordered in [false, true] {
            let mut f = fixture(
                if restricted { &["7", "1"] } else { &["7"] },
                if restricted { &["0", "1"] } else { &["0"] },
                &["7"],
                !restricted,
                reordered,
            )?;
            let checked = f
                .base
                .cas
                .replay(&bytes, &f.expected, &mut default_budget()?)?;
            assert_eq!(checked.judgment(), &f.expected);
            assert_eq!(
                f.base
                    .cas
                    .export_witness(&checked, &mut default_budget()?)?,
                bytes
            );
            let supplied =
                f.base
                    .cas
                    .check_proof(&f.base.context, &f.steps, &mut default_budget()?)?;
            assert_eq!(
                f.base
                    .cas
                    .export_witness(&supplied, &mut default_budget()?)?,
                bytes
            );
        }
    }
    Ok(())
}

/// Both dedicated readers retain closed mode, form and result-tag boundaries.
/// ´test:integration:compose-wire-reader-boundaries´
#[test]
fn compose_wire_reader_boundaries() -> TestResult {
    use nrz_cas::{FaultKind, InvalidCode};
    super::series_residual_wire::payload_control()?;
    let bytes = golden(false)?;
    let records = crate::testing::answer::records(&bytes)?;
    let (_, statement) = records
        .iter()
        .find(|(tag, _)| *tag == 39)
        .ok_or("statement")?;
    let (_, certificate) = records
        .iter()
        .find(|(tag, _)| *tag == 40)
        .ok_or("certificate")?;
    let statement = statement.start + 36;
    let certificate = certificate.start + 15;
    assert_eq!(&bytes[statement..statement + 2], &[5, 0]);
    assert_eq!(&bytes[certificate..certificate + 2], &[5, 0]);
    assert_eq!(bytes[statement + 10], 0);
    let mut mutations = Vec::new();
    for offset in [statement, certificate] {
        for mode in [6, 7, 255] {
            mutations.push((offset, mode));
        }
        for form in [1, 2] {
            mutations.push((offset + 1, form));
        }
    }
    // Mode eleven is live; these unchanged Compose payloads lack its residual fields.
    mutations.extend([(statement, 11), (certificate, 11)]);
    mutations.push((statement + 10, 3));
    for (offset, value) in mutations {
        let mut changed = bytes.clone();
        changed[offset] = value;
        for reordered in [false, true] {
            let mut f = fixture(&["7"], &["0"], &["7"], true, reordered)?;
            f.base
                .cas
                .replay(&bytes, &f.expected, &mut default_budget()?)?;
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            let mut budget = default_budget()?;
            budget.work(37, nrz_cas::Location::new("composition wire prior work"))?;
            let fault = f
                .base
                .cas
                .replay(&changed, &f.expected, &mut budget)
                .expect_err("closed composition reader");
            assert_eq!(
                fault.kind,
                if offset == certificate && value == 7 {
                    FaultKind::Exhausted {
                        reason: nrz_cas::Exhaustion::Limit,
                        budget: nrz_cas::limits::BudgetKind::WitnessBytes,
                        limit: 1_048_576,
                        needed: 16_777_216,
                    }
                } else {
                    FaultKind::Invalid(InvalidCode::SeriesUnsupported)
                },
                "offset {offset} value {value}"
            );
            assert!(budget.usage().work > 37);
            assert_eq!(
                before,
                f.base.cas.canonical_snapshot(&mut default_budget()?)?
            );
        }
    }
    Ok(())
}

/// Unknown higher coefficients stay outside the result but inside the exact original dependency.
/// ´test:integration:compose-original-precision-identity´
#[test]
fn compose_original_precision_identity() -> TestResult {
    use nrz_cas::proof::Statement;
    use nrz_cas::series::SeriesStatement as S;
    use nrz_cas::{FaultKind, InvalidCode};
    let bytes = golden(true)?;
    for reordered in [false, true] {
        for mutation in 0..3 {
            let mut f = fixture(
                &["7", if mutation == 0 { "2" } else { "1" }],
                &["0", "1"],
                &["7"],
                false,
                reordered,
            )?;
            let own = f
                .base
                .cas
                .check_proof(&f.base.context, &f.steps, &mut default_budget()?)?;
            let mut expected = own.judgment().clone();
            let Statement::Series(s) = &mut expected.statement else {
                return Err("series".into());
            };
            let S::Compose {
                left, right, order, ..
            } = s.as_mut()
            else {
                return Err("compose".into());
            };
            match mutation {
                1 => *order = 2,
                2 => std::mem::swap(left, right),
                _ => {}
            }
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                f.base
                    .cas
                    .replay(&bytes, &expected, &mut default_budget()?)
                    .expect_err("changed original claim")
                    .kind,
                FaultKind::Invalid(InvalidCode::EndpointMismatch)
            );
            assert_eq!(
                before,
                f.base.cas.canonical_snapshot(&mut default_budget()?)?
            );
        }
    }
    Ok(())
}

/// Complete mode-five bytes distinguish capability refusal from a false computation row.
pub(super) fn payload_control() -> TestResult {
    use crate::testing::answer::records;
    use nrz_cas::{FaultKind, InvalidCode};
    let bytes = golden(false)?;
    let records = records(&bytes)?;
    let (_, certificate) = records
        .iter()
        .find(|(tag, _)| *tag == 40)
        .ok_or("certificate")?;
    // Record prefix, statement reference, mode and empty premise count precede the payload.
    let begin = certificate.start + 15;
    assert_eq!(&bytes[begin..begin + 2], &[5, 0]);
    let last = certificate.end - 6;
    assert_eq!(bytes[last], 7); // Numerator magnitude in the final exact partial sum.
    let mut changed = bytes.clone();
    changed[last] = 8;
    for reordered in [false, true] {
        let mut f = fixture(&["7"], &["0"], &["7"], true, reordered)?;
        f.base
            .cas
            .replay(&bytes, &f.expected, &mut default_budget()?)?;
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = f
            .base
            .cas
            .replay(&changed, &f.expected, &mut default_budget()?)
            .expect_err("false composition sum");
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
        assert_eq!(
            before,
            f.base.cas.canonical_snapshot(&mut default_budget()?)?
        );
    }
    Ok(())
}
