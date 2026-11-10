// SPDX-License-Identifier: AGPL-3.0-only
//! Complete mixed DAGs authored from record fields, without the production encoder.

use super::series_golden::{
    Part, Record, bytes, count, encode_records, expression, join, name, number, object, records,
};
use crate::testing::common::{TestResult, default_budget};
use crate::testing::series_residual::{Fixture, Term, admit, binary, member, rational};
use nrz_cas::proof::{Judgment, Statement};
use nrz_cas::series::{
    ResidualSubject, SeriesOperation, SeriesStatement, SourceClause, SourceStatement,
};
use nrz_cas::syntax::Operator;

/// A literal record keeps its stable wire tag visible at the call site.
const fn record(key: &'static str, tag: u8, parts: Vec<Part>) -> Record {
    Record {
        name: key,
        tag,
        layer: 0,
        parts,
    }
}

/// Each closure states full keys and versions, independently of registry indices.
fn closure(keys: &[&str]) -> Vec<u8> {
    let mut out = count(u32::try_from(keys.len()).unwrap_or(u32::MAX));
    for key in keys {
        out.extend(name(key));
        out.extend(count(1));
    }
    out
}

/// Product domains preserve the complete coordinate identity.
fn region(key: &'static str, coordinates: &'static str) -> Record {
    record(
        key,
        1,
        vec![
            Part::Ref("header"),
            bytes(join(&[count(0), vec![4]])),
            Part::Ref(coordinates),
            bytes(vec![0, 0]),
        ],
    )
}

/// Series statement, finite certificate and checked conclusion share one ordered premise vector.
#[expect(
    clippy::too_many_arguments,
    reason = "the independent grammar states all record fields explicitly"
)]
fn claim(
    values: &mut Vec<Record>,
    names: [&'static str; 3],
    key: &'static str,
    index: u32,
    statement: Vec<Part>,
    rows: Vec<u8>,
    premises: &[&'static str],
    keys: &[&str],
    conditional: bool,
) {
    let [statement_name, certificate_name, conclusion] = names;
    let mut parts = vec![
        bytes(join(&[count(index), name(key), count(1)])),
        Part::Ref("context"),
    ];
    parts.extend(statement);
    parts.push(bytes(count(u32::from(conditional))));
    if conditional {
        parts.extend([Part::Ref("context"), bytes(count(0))]);
    }
    parts.push(bytes(join(&[count(0), closure(keys)])));
    values.push(record(statement_name, 39, parts));
    let mut parts = vec![
        Part::Ref(statement_name),
        bytes(join(&[
            vec![rows[0]],
            count(u32::try_from(premises.len()).unwrap_or(u32::MAX)),
        ])),
    ];
    parts.extend(premises.iter().map(|key| Part::Ref(key)));
    parts.push(bytes(rows));
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

/// Common Real header, coefficient x product, empty unknown product and exact constant equation.
pub(super) fn frame(source: bool, nonzero: bool) -> Vec<Record> {
    let mut values = records();
    values.retain(|r| {
        matches!(r.name, "real" | "complex" | "Q" | "x" | "zero") || (!source && r.name == "one")
    });
    let mut header = join(&[vec![0, 1, 0], count(3)]);
    for key in ["x", "y", "z"] {
        header.extend(join(&[name(key), name(key), vec![0, 1, 1]]));
    }
    header.extend(count(2));
    values.push(record(
        "header",
        0,
        vec![
            bytes(header),
            Part::Ref("real"),
            Part::Ref("complex"),
            bytes(count(0)),
        ],
    ));
    values.push(expression("y", join(&[vec![1], name("y")])));
    values.push(record(
        "coordinates",
        13,
        vec![
            Part::Ref("header"),
            bytes(join(&[count(1), vec![1]])),
            Part::Ref("real"),
            bytes(join(&[count(1), name("x"), vec![1, 0]])),
        ],
    ));
    values.push(record(
        "unknowns",
        13,
        vec![Part::Ref("header"), bytes(join(&[count(0), count(0)]))],
    ));
    values.push(region("ambient", "coordinates"));
    values.push(region("domain", "unknowns"));
    let mut context = vec![
        Part::Ref("header"),
        bytes(vec![0]),
        Part::Ref("ambient"),
        bytes(join(&[vec![0], count(u32::from(source))])),
    ];
    if source {
        context.extend([bytes(name("source-defined")), Part::Ref("defined")]);
    }
    values.push(record("context", 8, context));
    if nonzero {
        values.push(expression(
            "minus-one",
            join(&[vec![0], number(true, 1, 1)]),
        ));
    }
    values.push(record(
        "equation",
        7,
        vec![
            Part::Ref("header"),
            bytes(join(&[count(0), vec![6], count(2)])),
            Part::Ref(if nonzero { "minus-one" } else { "zero" }),
            Part::Ref("zero"),
        ],
    ));
    values.push(record(
        "problem",
        26,
        vec![
            Part::Ref("header"),
            bytes(count(0)),
            Part::Ref("equation"),
            Part::Ref("coordinates"),
            Part::Ref("unknowns"),
            Part::Ref("ambient"),
            Part::Ref("domain"),
            Part::Ref("Q"),
            bytes(join(&[vec![1], count(0)])),
        ],
    ));
    values
}

/// Order-one polynomial seed zero with a retained, consistent Defined(0) premise.
fn source_subject(values: &mut Vec<Record>) {
    values.push(record(
        "defined",
        7,
        vec![
            Part::Ref("header"),
            bytes(join(&[count(0), vec![0], count(1)])),
            Part::Ref("zero"),
        ],
    ));
    values.push(record(
        "guard-statement",
        10,
        vec![
            Part::Ref("context"),
            bytes(vec![1]),
            Part::Ref("ambient"),
            Part::Ref("defined"),
        ],
    ));
    values.push(record(
        "guard",
        11,
        vec![
            bytes(count(0)),
            Part::Ref("guard-statement"),
            bytes(join(&[count(0), count(0), vec![2]])),
            Part::Ref("context"),
            bytes(vec![0; 8]),
        ],
    ));
    values.push(object("candidate", &["zero"]));
    claim(
        values,
        ["subject-statement", "subject-certificate", "subject"],
        "series.truncate",
        2,
        vec![
            bytes(vec![0, 2, 0]),
            Part::Ref("zero"),
            Part::Ref("ambient"),
            bytes(vec![2]),
            Part::Ref("candidate"),
        ],
        join(&[
            vec![0, 2, 0],
            count(1),
            number(false, 0, 1),
            count(0),
            count(0),
        ]),
        &["guard"],
        &["ctx.hypothesis", "series.truncate"],
        true,
    );
}

/// Higher-order P=7+h and Q=h are explicitly restricted before composing at order one.
fn compose_subject(values: &mut Vec<Record>) {
    values.push(expression("seven", join(&[vec![0], number(false, 7, 1)])));
    values.push(object("outer", &["seven", "one"]));
    values.push(object("inner", &["zero", "one"]));
    values.push(object("candidate", &["seven"]));
    values.push(object("inner-short", &["zero"]));
    for (names, input, output, value) in [
        (
            ["left-statement", "left-certificate", "left-proof"],
            "outer",
            "candidate",
            7,
        ),
        (
            ["right-statement", "right-certificate", "right-proof"],
            "inner",
            "inner-short",
            0,
        ),
    ] {
        claim(
            values,
            names,
            "series.truncate",
            3,
            vec![
                bytes(vec![0, 0]),
                Part::Ref(input),
                bytes(vec![0]),
                Part::Ref(output),
            ],
            join(&[vec![0, 0], count(1), number(false, value, 1)]),
            &[],
            &["series.truncate"],
            false,
        );
    }
    claim(
        values,
        ["subject-statement", "subject-certificate", "subject"],
        "series.compose",
        1,
        compose_descriptor(),
        join(&[
            vec![5, 0],
            count(1),
            vec![0],
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
        ]),
        &["left-proof", "right-proof"],
        &["series.compose", "series.truncate"],
        false,
    );
}

/// Nested and top-level composition use the same explicitly retained inputs and precision.
fn compose_descriptor() -> Vec<Part> {
    vec![
        bytes(vec![5, 0]),
        Part::Ref("outer"),
        Part::Ref("inner"),
        bytes(vec![0]),
        Part::Ref("candidate"),
        bytes(join(&[count(1), vec![0]])),
    ]
}

/// A 1-by-1 matrix uses the original ordinary expression grammar and exact literal entry.
fn matrix(nonzero: bool) -> Record {
    let one = 1_u64.to_le_bytes().to_vec();
    let mut value = record(
        "matrix",
        3,
        vec![
            Part::Ref("header"),
            bytes(join(&[
                count(0),
                vec![0, 0, 3, 1, 0],
                one.clone(),
                vec![0],
                one.clone(),
                vec![19],
                one.clone(),
                one,
                vec![0],
                count(1),
            ])),
            Part::Ref(if nonzero { "one" } else { "zero" }),
            bytes(join(&[vec![0, 0], count(0)])),
        ],
    );
    value.layer = 1;
    value
}

/// Entire Source→Residual→Member or restricted Compose→Residual→LinearSystem witness.
#[expect(
    clippy::too_many_lines,
    reason = "the three hand-authored complete DAGs share an explicit wire grammar"
)]
fn golden(source: bool, nonzero: bool) -> TestResult<Vec<u8>> {
    let mut values = frame(source, nonzero);
    if source {
        source_subject(&mut values);
    } else {
        compose_subject(&mut values);
    }
    let root_expression = if source { "y" } else { "R" };
    if !source {
        if nonzero {
            values.push(expression("eight", join(&[vec![0], number(false, 8, 1)])));
        }
        let mut expr = record(
            "R",
            3,
            vec![
                Part::Ref("header"),
                bytes(join(&[count(0), vec![1]])),
                Part::Ref("real"),
                bytes(join(&[vec![0, 1, 1, 6, 0], count(2)])),
                Part::Ref("y"),
                Part::Ref(if nonzero { "eight" } else { "seven" }),
                bytes(join(&[vec![0, 0], count(0)])),
            ],
        );
        expr.layer = 1;
        values.push(expr);
        if nonzero {
            values.push(object("residual", &["minus-one"]));
        }
    }
    let mut statement = vec![bytes(vec![11, 0, 3, 0])];
    if source {
        statement.extend([
            bytes(vec![0, 2, 0]),
            Part::Ref("zero"),
            Part::Ref("ambient"),
            bytes(vec![2]),
            Part::Ref("candidate"),
        ]);
    } else {
        statement.extend(compose_descriptor());
    }
    statement.extend([
        Part::Ref(root_expression),
        Part::Ref("y"),
        Part::Ref(if source {
            "candidate"
        } else if nonzero {
            "residual"
        } else {
            "inner-short"
        }),
        Part::Ref("problem"),
        bytes(join(&[count(0), count(1)])),
    ]);
    let mut rows = join(&[vec![11, 0, 0], count(if source { 1 } else { 3 })]);
    let coefficients = if source {
        vec![(false, 0)]
    } else {
        vec![
            (false, 7),
            (false, if nonzero { 8 } else { 7 }),
            (nonzero, u8::from(nonzero)),
        ]
    };
    for (index, (negative, coefficient)) in coefficients.into_iter().enumerate() {
        rows.extend(join(&[
            count(u32::try_from(index)?),
            count(1),
            number(negative, coefficient, 1),
        ]));
    }
    let keys: &[&str] = if source {
        &["ctx.hypothesis", "series.residual", "series.truncate"]
    } else {
        &["series.compose", "series.residual", "series.truncate"]
    };
    claim(
        &mut values,
        [
            "residual-statement",
            "residual-certificate",
            "residual-proof",
        ],
        "series.residual",
        if source { 1 } else { 2 },
        statement,
        rows,
        &["subject"],
        keys,
        source,
    );
    if source {
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
                bytes(count(3)),
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
    } else {
        values.push(matrix(nonzero));
        values.push(record(
            "solve-statement",
            10,
            vec![
                Part::Ref("context"),
                bytes(vec![25, 6]),
                Part::Ref("problem"),
                Part::Ref("matrix"),
                bytes(join(&[count(0), count(u32::from(nonzero))])),
                Part::Ref("ambient"),
                bytes(count(0)),
            ],
        ));
        let mut proof = vec![
            bytes(count(0)),
            Part::Ref("solve-statement"),
            bytes(count(1)),
            Part::Ref("residual-proof"),
            bytes(join(&[count(0), vec![12, 9]])),
            Part::Ref("matrix"),
            bytes(join(&[count(1), count(0), count(u32::from(nonzero))])),
        ];
        if nonzero {
            proof.push(bytes(count(0)));
        }
        proof.extend([Part::Ref("one"), Part::Ref("matrix"), Part::Ref("ambient")]);
        values.push(record("solve-proof", 11, proof));
    }
    let mut conclusion = vec![
        bytes(vec![1]),
        Part::Ref("solve-proof"),
        Part::Ref("solve-statement"),
        bytes(join(&[count(0), count(u32::from(source))])),
    ];
    if source {
        conclusion.extend([Part::Ref("context"), bytes(vec![0; 8])]);
    }
    values.push(record("conclusion", 12, conclusion));
    encode_records(
        &values,
        closure(if source {
            &[
                "ctx.hypothesis",
                "series.residual",
                "series.truncate",
                "solve.member",
            ]
        } else {
            &[
                "linear.equivalent",
                "series.compose",
                "series.residual",
                "series.truncate",
            ]
        }),
    )
}

/// Independently construct expected syntax without any receiver proof or sender handle.
fn expected(f: &mut Fixture, source: bool, nonzero: bool) -> TestResult<(Judgment, Judgment)> {
    use nrz_cas::Reference;
    use nrz_cas::domain::{AssumptionDraft, ContextDraft};
    use nrz_cas::syntax::{Draft, PredicateKind};
    if source {
        let zero = f.node(Operator::Rational(rational("0")?), &[])?;
        let predicate = admit(
            &mut f.cas,
            Draft::Condition {
                header: Reference::Existing(f.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Defined,
                arguments: vec![Reference::Existing(zero)],
            },
        )?;
        f.context = admit(
            &mut f.cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(f.header.clone()),
                admissible: Reference::Existing(f.region.clone()),
                assumptions: vec![AssumptionDraft {
                    key: "source-defined".into(),
                    condition: Reference::Existing(predicate),
                }],
            }),
        )?;
    }
    let term = if source {
        Term::Y(0)
    } else {
        binary(
            Operator::Sub,
            Term::Y(0),
            Term::Q(if nonzero { "8" } else { "7" }),
        )
    };
    let (mut residual, mut solving) =
        f.expectations(&term, &[rational(if nonzero { "-1" } else { "0" })?], 0)?;
    let Statement::Series(statement) = &mut residual.statement else {
        return Err("residual".into());
    };
    let SeriesStatement::EvaluatedResidual { subject, .. } = statement.as_mut() else {
        return Err("residual".into());
    };
    *subject = if source {
        ResidualSubject::Source(SourceStatement {
            clause: SourceClause::PolynomialSeed,
            source: f.node(Operator::Rational(rational("0")?), &[])?,
            domain: f.region.clone(),
            output: f.object(&[rational("0")?])?,
        })
    } else {
        ResidualSubject::Compose {
            left: f.object(&[rational("7")?, rational("1")?])?,
            right: f.object(&[rational("0")?, rational("1")?])?,
            output: f.object(&[rational("7")?])?,
            order: 1,
            default_order: false,
        }
    };
    if !source {
        use nrz_cas::series::ResidualReference;
        use nrz_cas::solving::{SolveStatement, StatementLinearSystem};
        let Statement::Solve(member) = &solving.statement else {
            return Err("solve".into());
        };
        let SolveStatement::Member(member) = member.as_ref() else {
            return Err("member".into());
        };
        let linear = f.linear(&[rational(if nonzero { "-1" } else { "0" })?])?;
        let SolveStatement::LinearSystem(linear) = linear.statement else {
            return Err("linear".into());
        };
        let ResidualReference::Existing(reduced) = linear.reduced else {
            return Err("matrix".into());
        };
        solving.statement = Statement::Solve(Box::new(SolveStatement::LinearSystem(
            StatementLinearSystem {
                problem: member.problem.clone(),
                reduced,
                rank: 0,
                augmented_rank: u32::from(nonzero),
                region: f.region.clone(),
                free_columns: Vec::new(),
            },
        )));
    }
    Ok((residual, solving))
}

/// Three complete independent witnesses include both zero and augmented contradiction results.
/// ´test:integration:residual-independent-goldens´
#[test]
fn residual_independent_goldens() -> TestResult {
    for (source, nonzero) in [(true, false), (false, false), (false, true)] {
        let bytes = golden(source, nonzero)?;
        for reversed in [false, true] {
            let mut f = Fixture::new(if source { &["0"] } else { &["7"] }, "0", reversed, false)?;
            let (residual, solving) = expected(&mut f, source, nonzero)?;
            let mut sender =
                Fixture::new(if source { &["0"] } else { &["7"] }, "0", !reversed, false)?;
            let subject = if source {
                sender.source(&Term::Q("0"), SourceClause::PolynomialSeed)?
            } else {
                let left = sender.object(&[rational("7")?, rational("1")?])?;
                let right = sender.object(&[rational("0")?, rational("1")?])?;
                let left_proof = sender.cas.compute_series(
                    &SeriesOperation::Truncate {
                        input: left.clone(),
                        order: 1,
                    },
                    &[],
                    &mut default_budget()?,
                )?;
                let right_proof = sender.cas.compute_series(
                    &SeriesOperation::Truncate {
                        input: right.clone(),
                        order: 1,
                    },
                    &[],
                    &mut default_budget()?,
                )?;
                sender
                    .cas
                    .compute_series(
                        &SeriesOperation::Compose {
                            left,
                            right,
                            order: Some(1),
                        },
                        &[left_proof.proof().clone(), right_proof.proof().clone()],
                        &mut default_budget()?,
                    )
                    .map_err(|fault| format!("golden subject: {fault:?}"))?
            };
            let term = if source {
                Term::Y(0)
            } else {
                binary(
                    Operator::Sub,
                    Term::Y(0),
                    Term::Q(if nonzero { "8" } else { "7" }),
                )
            };
            let (expression, rows) = sender.term(&term, 1)?;
            let supplied = if source {
                member()
            } else {
                sender.linear(&[rational(if nonzero { "-1" } else { "0" })?])?
            };
            let (_, checked) = sender
                .cas
                .check_evaluated_residual(
                    &subject,
                    &expression,
                    &sender.y,
                    &rows,
                    Some(&supplied),
                    &mut default_budget()?,
                )
                .map_err(|fault| {
                    format!("golden construct source={source} nonzero={nonzero}: {fault:?}")
                })?;
            let actual = sender
                .cas
                .export_witness(&checked.ok_or("solve")?, &mut default_budget()?)?;
            if bytes != actual {
                let offset = bytes
                    .iter()
                    .zip(&actual)
                    .position(|(a, b)| a != b)
                    .unwrap_or_else(|| bytes.len().min(actual.len()));
                return Err(format!("golden source={source} nonzero={nonzero}: lengths {}/{} offset {offset} hand {:?} actual {:?}", bytes.len(), actual.len(), bytes.get(offset.saturating_sub(8)..(offset+48).min(bytes.len())), actual.get(offset.saturating_sub(8)..(offset+48).min(actual.len()))).into());
            }
            let (_, checked) = f
                .cas
                .replay_evaluated_residual(&bytes, &residual, &solving, &mut default_budget()?)
                .map_err(|fault| {
                    format!("golden replay source={source} nonzero={nonzero}: {fault:?}")
                })?;
            assert_eq!(
                bytes,
                f.cas.export_witness(&checked, &mut default_budget()?)?
            );
        }
    }
    Ok(())
}
