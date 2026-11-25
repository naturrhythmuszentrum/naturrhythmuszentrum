// SPDX-License-Identifier: AGPL-3.0-only
//! Atomic certificate assembly and paired provenance replay.

use super::check::{AT, invalid, number, unsupported};
use super::{ResidualRow, ResidualSubject, SeriesCertificate, SeriesStatement};
use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{AmbientDomain, RegionDraft, RegionExpr};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference, size};
use crate::model::{AtomicSort, Extent, Field, Operator, PredicateKind, Sort};
use crate::proof_model::{ApplicationDraft, Certificate, Judgment, Statement, VerifiedWitness};
use crate::solving::{
    CertificateExactLinearEquivalence, CertificateMembership, CoordinateSpaceData, RowOperation,
    SolveCertificate, SolveDraft, SolveProblemData, SolveSchema, SolveStatement,
    StatementLinearSystem, StatementMember, TupleData,
};

/// API-only references to derived objects; never part of a wire certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResidualReference {
    /// Caller-supplied admitted object in an ordinary evidence field.
    Existing(Handle),
    /// The exact derived coefficient problem.
    Problem,
    /// The unique empty unknown tuple.
    Tuple,
    /// The original augmented constant matrix, with entries minus the residual.
    Matrix,
    /// The candidate context's exact admissible region.
    Region,
}

/// Supplied ordinary solving evidence with explicit derived-field bindings.
#[derive(Clone, Debug)]
pub struct ResidualSolve {
    /// Member or `LinearSystem` only.
    pub statement: SolveStatement<ResidualReference>,
    /// Matching Membership or `ExactLinearEquivalence` only.
    pub certificate: SolveCertificate<ResidualReference>,
    /// Ordinary prerequisites follow the residual at slot zero.
    pub premises: Vec<Handle>,
}

/// One checked graph admission within the enclosing operation transaction.
fn admit(cas: &mut Cas, draft: Draft, budget: &mut Budget) -> Result<Handle, Fault> {
    cas.admit(&[draft], budget)?.pop().ok_or_else(invalid)
}

/// Exact original header and empty scope are fixed before construction.
fn expression(
    cas: &mut Cas,
    header: &Handle,
    operator: Operator,
    sort: Sort,
    operands: Vec<Handle>,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    admit(
        cas,
        Draft::Expression(ExpressionDraft {
            header: Reference::Existing(header.clone()),
            space: None,
            operator,
            sort,
            scope: Vec::new(),
            binding: None,
            operands: operands.into_iter().map(Reference::Existing).collect(),
        }),
        budget,
    )
}

/// Only derived fields may name placeholders; all other fields must already exist.
fn existing(value: &ResidualReference) -> Result<Handle, Fault> {
    if let ResidualReference::Existing(value) = value {
        Ok(value.clone())
    } else {
        Err(invalid())
    }
}

impl ResidualSolve {
    /// Authenticate prerequisites before sizing; reserve every copied caller-controlled inventory.
    fn reservation(&self, cas: &Cas, budget: &mut Budget) -> Result<u64, Fault> {
        budget.work(size(self.premises.len(), AT)?, AT)?;
        for proof in &self.premises {
            cas.checked(proof, budget)?;
        }
        // Each slot retains a Handle, its u32 application index and the helper's logical ID.
        // Include slot zero, which is filled with the independently checked residual later.
        let mut bytes = size(self.premises.len(), AT)?
            .checked_add(1)
            .and_then(|n| n.checked_mul(64))
            .ok_or_else(invalid)?;
        let inventories = match (&self.statement, &self.certificate) {
            (SolveStatement::Member(_), SolveCertificate::Membership(value)) => {
                [(value.definedness.len(), 4), (0, 0), (0, 0)]
            }
            (
                SolveStatement::LinearSystem(statement),
                SolveCertificate::ExactLinearEquivalence(value),
            ) => [
                (value.operations.len(), 64),
                (value.pivots.len(), 4),
                (statement.free_columns.len(), 4),
            ],
            _ => return Err(invalid()),
        };
        for (count, width) in inventories {
            bytes = size(count, AT)?
                .checked_mul(width)
                .and_then(|n| bytes.checked_add(n))
                .ok_or_else(invalid)?;
        }
        Ok(bytes)
    }

    /// Reject bypasses and misplaced placeholders before constructing derived syntax.
    fn preflight(&self) -> Result<(), Fault> {
        use ResidualReference::{Matrix, Problem, Region, Tuple};
        match (&self.statement, &self.certificate) {
            (SolveStatement::Member(statement), SolveCertificate::Membership(certificate)) => {
                if (&statement.problem, &statement.tuple, &statement.region)
                    != (&Problem, &Tuple, &Region)
                    || (
                        &certificate.problem,
                        &certificate.tuple,
                        &certificate.region,
                    ) != (&Problem, &Tuple, &Region)
                    || !certificate.substitutions.is_empty()
                {
                    return Err(invalid());
                }
            }
            (
                SolveStatement::LinearSystem(statement),
                SolveCertificate::ExactLinearEquivalence(certificate),
            ) => {
                if (&statement.problem, &statement.region) != (&Problem, &Region)
                    || (&certificate.matrix, &certificate.region) != (&Matrix, &Region)
                    || certificate.augmented_columns != 1
                {
                    return Err(invalid());
                }
                existing(&statement.reduced)?;
                existing(&certificate.reduced)?;
                existing(&certificate.minor)?;
                for operation in &certificate.operations {
                    match operation {
                        RowOperation::Swap { .. } => {}
                        RowOperation::Scale { value, .. }
                        | RowOperation::AddMultiple { value, .. } => {
                            existing(value)?;
                        }
                    }
                }
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }
}

/// Candidate assembly uses only the caller's final row; the checker reconstructs all arithmetic.
fn construct(
    cas: &mut Cas,
    subject: &ResidualSubject,
    rows: &[ResidualRow],
    budget: &mut Budget,
) -> Result<(Handle, Handle), Fault> {
    let input = subject.output();
    let final_row = rows.last().ok_or_else(invalid)?;
    let mut coefficients = Vec::new();
    for value in &final_row.coefficients {
        coefficients.push(super::produce::literal(
            cas,
            input,
            super::produce::copied(value, budget)?,
            budget,
        )?);
    }
    let output = super::produce::output(cas, input, coefficients.clone(), budget)?;
    let basis = cas.series_object(input, budget)?;
    let context = cas.state.context(basis.context.logical_id(), budget)?;
    let header = cas.handle(context.header_id);
    let region = cas.handle(context.admissible);
    let field = basis.field.clone();
    let AmbientDomain::Product {
        coordinates: product,
    } = &cas.state.region(context.admissible, budget)?.ambient
    else {
        return Err(unsupported());
    };
    let product = product.clone();
    let zero = super::produce::literal(cas, input, crate::number::Rational::zero(), budget)?;
    let (kind, arguments) = if coefficients.len() == 1 {
        (PredicateKind::Equal, vec![coefficients[0].clone(), zero])
    } else {
        let mut equations = Vec::new();
        for coefficient in coefficients {
            equations.push(expression(
                cas,
                &header,
                Operator::Predicate(PredicateKind::Equal),
                Sort::Value(AtomicSort::Boolean),
                vec![coefficient, zero.clone()],
                budget,
            )?);
        }
        (PredicateKind::And, equations)
    };
    let predicate = admit(
        cas,
        Draft::Condition {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            kind,
            arguments: arguments.into_iter().map(Reference::Existing).collect(),
        },
        budget,
    )?;
    let unknowns = admit(
        cas,
        Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            header: Reference::Existing(header.clone()),
            scalar_spaces: Vec::new(),
            coordinates: Vec::new(),
        })),
        budget,
    )?;
    let domain = admit(
        cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: unknowns.clone(),
            },
            expression: RegionExpr::Ambient,
        }),
        budget,
    )?;
    let problem = admit(
        cas,
        Draft::Solve(SolveDraft::SolveProblem(SolveProblemData {
            header: Reference::Existing(header),
            scope: Vec::new(),
            predicate: Reference::Existing(predicate),
            coefficients: Reference::Existing(product),
            unknowns: Reference::Existing(unknowns),
            region: Reference::Existing(region),
            domain: Reference::Existing(domain),
            coefficient_domain: Reference::Existing(field),
            ambient: Field::Real,
            denominators: Vec::new(),
        })),
        budget,
    )?;
    Ok((output, problem))
}

/// Bind the derived fields without manufacturing row operations, pivots or ordinary evidence.
#[expect(
    clippy::too_many_lines,
    reason = "both closed binding schemas remain visible together"
)]
fn bind(
    cas: &mut Cas,
    supplied: &ResidualSolve,
    problem: &Handle,
    output: &Handle,
    budget: &mut Budget,
) -> Result<(SolveStatement, SolveCertificate, SolveSchema), Fault> {
    let SolveDraft::SolveProblem(data) =
        crate::solving::graph::object(&cas.state, problem.logical_id(), budget)?
    else {
        return Err(invalid());
    };
    let region = data.region.clone();
    let header = data.header.clone();
    let unknowns = data.unknowns.clone();
    match (&supplied.statement, &supplied.certificate) {
        (SolveStatement::Member(_), SolveCertificate::Membership(certificate)) => {
            let tuple = admit(
                cas,
                Draft::Solve(SolveDraft::Tuple(TupleData {
                    header: Reference::Existing(header),
                    scope: Vec::new(),
                    coordinates: Reference::Existing(unknowns),
                    entries: Vec::new(),
                })),
                budget,
            )?;
            Ok((
                SolveStatement::Member(StatementMember {
                    problem: problem.clone(),
                    tuple: tuple.clone(),
                    region: region.clone(),
                }),
                SolveCertificate::Membership(CertificateMembership {
                    problem: problem.clone(),
                    tuple,
                    region,
                    substitutions: Vec::new(),
                    definedness: certificate.definedness.clone(),
                }),
                SolveSchema::Membership,
            ))
        }
        (
            SolveStatement::LinearSystem(statement),
            SolveCertificate::ExactLinearEquivalence(certificate),
        ) => {
            let order = cas.series_object(output, budget)?.order;
            let coefficients = cas.series_object(output, budget)?.coefficients.clone();
            let mut entries = Vec::new();
            for coefficient in coefficients {
                let value = number(cas, &coefficient, budget)?.negated(budget)?;
                entries.push(super::produce::literal(cas, output, value, budget)?);
            }
            let matrix = expression(
                cas,
                &header,
                Operator::Matrix {
                    rows: order,
                    columns: 1,
                },
                Sort::Value(AtomicSort::Matrix {
                    field: Field::Real,
                    rows: Extent::Literal(order),
                    columns: Extent::Literal(1),
                }),
                entries,
                budget,
            )?;
            let mut operations = Vec::new();
            operations
                .try_reserve_exact(certificate.operations.len())
                .map_err(|_| Fault::allocation(AT))?;
            for operation in &certificate.operations {
                operations.push(match operation {
                    RowOperation::Swap { left, right } => RowOperation::Swap {
                        left: *left,
                        right: *right,
                    },
                    RowOperation::Scale { row, value, unit } => RowOperation::Scale {
                        row: *row,
                        value: existing(value)?,
                        unit: *unit,
                    },
                    RowOperation::AddMultiple {
                        destination,
                        source,
                        value,
                    } => RowOperation::AddMultiple {
                        destination: *destination,
                        source: *source,
                        value: existing(value)?,
                    },
                });
            }
            Ok((
                SolveStatement::LinearSystem(StatementLinearSystem {
                    problem: problem.clone(),
                    reduced: existing(&statement.reduced)?,
                    rank: statement.rank,
                    augmented_rank: statement.augmented_rank,
                    region: region.clone(),
                    free_columns: statement.free_columns.clone(),
                }),
                SolveCertificate::ExactLinearEquivalence(CertificateExactLinearEquivalence {
                    matrix,
                    augmented_columns: 1,
                    operations,
                    pivots: certificate.pivots.clone(),
                    minor: existing(&certificate.minor)?,
                    reduced: existing(&certificate.reduced)?,
                    region,
                }),
                SolveSchema::ExactLinearEquivalence,
            ))
        }
        _ => Err(invalid()),
    }
}

impl Cas {
    /// Construct and independently check a finite polynomial residual, optionally checking its problem.
    ///
    /// Member certifies the complete derived predicate on the admissible region under all retained
    /// conditions. Polynomial powers use p^0=1, whereas `StrictReal` leaves 0^0 undefined. Neither
    /// strict definedness nor scalar or source correspondence
    /// of the original expression follows. A nonzero residual is a successful computation.
    ///
    /// # Errors
    /// Refuses unsupported syntax, false traces, mismatched evidence or exhausted resources atomically.
    #[expect(
        clippy::too_many_lines,
        reason = "the complete atomic construction and publication lifetime stays visible"
    )]
    pub fn check_evaluated_residual(
        &mut self,
        subject: &VerifiedWitness,
        expression: &Handle,
        dependent: &Handle,
        rows: &[ResidualRow],
        solving: Option<&ResidualSolve>,
        budget: &mut Budget,
    ) -> Result<(VerifiedWitness, Option<VerifiedWitness>), Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                let Statement::Series(statement) = &subject.judgment().statement else {
                    return Err(unsupported());
                };
                let subject_value = match statement.as_ref() {
                    SeriesStatement::Source(value) => ResidualSubject::Source(value.clone()),
                    SeriesStatement::Compose {
                        left,
                        right,
                        output,
                        order,
                        default_order,
                    } => ResidualSubject::Compose {
                        left: left.clone(),
                        right: right.clone(),
                        output: output.clone(),
                        order: *order,
                        default_order: *default_order,
                    },
                    _ => return Err(unsupported()),
                };
                cas.checked(subject.proof(), budget)?;
                let (derivative_order, order, count) =
                    super::residual::preflight(cas, &subject_value, expression, dependent, budget)?;
                if count != size(rows.len(), AT)? {
                    return Err(invalid());
                }
                // The known schedule prepays shape inspection, including a late mismatch.
                budget.work(count, AT)?;
                if rows.iter().any(|row| {
                    u64::from(order) != u64::try_from(row.coefficients.len()).unwrap_or(u64::MAX)
                }) {
                    return Err(invalid());
                }
                let supplied_bytes = solving
                    .map(|value| value.reservation(cas, budget))
                    .transpose()?
                    .unwrap_or(0);
                let entries = count.checked_mul(u64::from(order)).ok_or_else(invalid)?;
                // Validated widths make this the exact rational-sizing visit inventory.
                budget.work(entries, AT)?;
                let mut bytes = entries
                    .checked_mul(1024)
                    .and_then(|n| n.checked_add(4096))
                    .and_then(|n| n.checked_add(supplied_bytes))
                    .ok_or_else(invalid)?;
                for row in rows {
                    for value in &row.coefficients {
                        value.check(budget, AT)?;
                        let (a, b) = value.component_bits();
                        bytes = bytes
                            .checked_add(
                                a.checked_add(b)
                                    .and_then(|n| n.checked_mul(16))
                                    .ok_or_else(invalid)?,
                            )
                            .ok_or_else(invalid)?;
                    }
                }
                bytes = bytes
                    .checked_add(crate::proof_view::application_bytes(
                        cas,
                        cas.checked(subject.proof(), budget)?,
                        budget,
                    )?)
                    .ok_or_else(invalid)?;
                budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
                budget.work(bytes, AT)?;
                budget.with_retained_payload(bytes, AT, |budget| {
                    // The byte charge prepays visits and copies, including the operation walk.
                    // Ownership lasts through binding, checking, publication and both seals.
                    if let Some(solving) = solving {
                        solving.preflight()?;
                    }
                    let (output, problem) = construct(cas, &subject_value, rows, budget)?;
                    let context = cas
                        .series_object(subject_value.output(), budget)?
                        .context
                        .clone();
                    let draft = ApplicationDraft {
                        rule: crate::rule::Primitive::SeriesResidual,
                        context: context.clone(),
                        conclusion: Statement::Series(Box::new(
                            SeriesStatement::EvaluatedResidual {
                                subject: subject_value,
                                expression: expression.clone(),
                                dependent: dependent.clone(),
                                output: output.clone(),
                                problem: problem.clone(),
                                derivative_order,
                                order,
                            },
                        )),
                        premises: vec![0],
                        substitutions: Vec::new(),
                        certificate: Certificate::Series(Box::new(
                            SeriesCertificate::EvaluatedResidual {
                                form: 0,
                                stage: 0,
                                rows: rows.to_vec(),
                            },
                        )),
                    };
                    let proof = crate::scoped_check::check_supplied(
                        cas,
                        &draft,
                        std::slice::from_ref(subject.proof()),
                        budget,
                    )?;
                    let residual = cas.seal_scoped(&proof, budget)?;
                    let Some(solving) = solving else {
                        return Ok((residual, None));
                    };
                    let retained = size(residual.exported_byte_len(), AT)?;
                    budget.with_retained_payload(retained, AT, |budget| {
                        let (statement, certificate, schema) =
                            bind(cas, solving, &problem, &output, budget)?;
                        let count = solving.premises.len().checked_add(1).ok_or_else(invalid)?;
                        let mut proofs = Vec::new();
                        proofs
                            .try_reserve_exact(count)
                            .map_err(|_| Fault::allocation(AT))?;
                        proofs.push(proof);
                        proofs.extend(solving.premises.iter().cloned());
                        let mut slots = Vec::new();
                        slots
                            .try_reserve_exact(count)
                            .map_err(|_| Fault::allocation(AT))?;
                        for index in 0..count {
                            slots.push(u32::try_from(index).map_err(|_| invalid())?);
                        }
                        let draft = ApplicationDraft {
                            rule: crate::rule::Primitive::Solve(schema),
                            context,
                            conclusion: Statement::Solve(Box::new(statement)),
                            premises: slots,
                            substitutions: Vec::new(),
                            certificate: Certificate::Solve(Box::new(certificate)),
                        };
                        let proof =
                            crate::scoped_check::check_supplied(cas, &draft, &proofs, budget)?;
                        Ok((residual, Some(cas.seal_scoped(&proof, budget)?)))
                    })
                })
            })
        })
    }

    /// Replay both the solving root and its unique direct residual provenance atomically.
    ///
    /// # Errors
    /// Either independent expected-judgment mismatch rolls back every imported object and proof.
    pub fn replay_evaluated_residual(
        &mut self,
        bytes: &[u8],
        residual_expected: &Judgment,
        solve_expected: &Judgment,
        budget: &mut Budget,
    ) -> Result<(VerifiedWitness, VerifiedWitness), Fault> {
        crate::wire_decode::magic(bytes)?;
        self.proof_limits(budget, |cas, budget| crate::proof_check::replay_budget(budget, |budget| {
            crate::proof_view::validate_judgment(cas, residual_expected, budget)?;
            crate::proof_view::validate_judgment(cas, solve_expected, budget)?;
            cas.transaction(budget, |cas, budget| {
                let solve = crate::wire_decode::inspect(cas, bytes, budget)?;
                if !matches!(&solve.judgment().statement, Statement::Solve(value) if matches!(value.as_ref(), SolveStatement::Member(_) | SolveStatement::LinearSystem(_))) { return Err(invalid()); }
                let payload = crate::proof_view::application_bytes(cas, cas.checked(solve.proof(), budget)?, budget)?;
                let retained = size(solve.exported_byte_len(), AT)?.checked_add(payload).ok_or_else(invalid)?;
                budget.with_retained_payload(retained, AT, |budget| {
                    compare(cas, solve_expected, solve.judgment(), budget)?;
                    let application = crate::proof_view::application(cas, cas.checked(solve.proof(), budget)?, budget)?;
                    let first = application.premises.first().ok_or_else(invalid)?;
                    let residual = cas.seal_scoped(&cas.handle(*first), budget)?;
                    if !matches!(residual.judgment().statement, Statement::Series(ref value) if matches!(value.as_ref(), SeriesStatement::EvaluatedResidual { .. })) { return Err(invalid()); }
                    budget.with_retained_payload(size(residual.exported_byte_len(), AT)?, AT, |budget| compare(cas, residual_expected, residual.judgment(), budget))?;
                    Ok((residual, solve))
                })
            })
        }))
    }
}

/// Structural keys compare complete judgments independently of arena allocation order.
fn compare(
    cas: &Cas,
    expected: &Judgment,
    actual: &Judgment,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if crate::wire_build::judgment_key(cas, expected, budget)?
        != crate::wire_build::judgment_key(cas, actual, budget)?
    {
        return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
    }
    Ok(())
}

/// A residual prerequisite binds only its derived problem, never an independently intended template.
pub(crate) fn gate(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[crate::proof_model::CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut residual = None;
    for (index, premise) in premises.iter().enumerate() {
        if let Statement::Series(value) = &premise.judgment.statement
            && let SeriesStatement::EvaluatedResidual {
                problem, output, ..
            } = value.as_ref()
        {
            if index != 0 || residual.is_some() || premise.judgment.context != draft.context {
                return Err(invalid());
            }
            residual = Some((problem, output));
        }
    }
    let Some((problem, output)) = residual else {
        return Ok(());
    };
    let SolveDraft::SolveProblem(data) =
        crate::solving::graph::object(&cas.state, problem.logical_id(), budget)?
    else {
        return Err(invalid());
    };
    let (Statement::Solve(statement), Certificate::Solve(certificate)) =
        (&draft.conclusion, &draft.certificate)
    else {
        return Err(invalid());
    };
    match (statement.as_ref(), certificate.as_ref()) {
        (SolveStatement::Member(statement), SolveCertificate::Membership(certificate)) => {
            if &statement.problem != problem
                || &certificate.problem != problem
                || statement.region != data.region
                || certificate.region != data.region
                || statement.tuple != certificate.tuple
                || !certificate.substitutions.is_empty()
            {
                return Err(invalid());
            }
            let SolveDraft::Tuple(tuple) =
                crate::solving::graph::object(&cas.state, statement.tuple.logical_id(), budget)?
            else {
                return Err(invalid());
            };
            if tuple.header != data.header
                || !tuple.scope.is_empty()
                || tuple.coordinates != data.unknowns
                || !tuple.entries.is_empty()
            {
                return Err(invalid());
            }
        }
        (
            SolveStatement::LinearSystem(statement),
            SolveCertificate::ExactLinearEquivalence(certificate),
        ) => {
            if &statement.problem != problem
                || statement.region != data.region
                || certificate.region != data.region
                || certificate.augmented_columns != 1
            {
                return Err(invalid());
            }
            let output = cas.series_object(output, budget)?;
            let matrix = cas
                .state
                .expression(cas.checked(&certificate.matrix, budget)?, budget)?;
            if matrix.header != data.header.logical_id()
                || !matrix.scope.is_empty()
                || matrix.operator
                    != (Operator::Matrix {
                        rows: output.order,
                        columns: 1,
                    })
                || matrix.operands.len() != output.coefficients.len()
            {
                return Err(invalid());
            }
            for (id, coefficient) in matrix.operands.iter().zip(&output.coefficients) {
                let Operator::Rational(value) = &cas.state.expression(*id, budget)?.operator else {
                    return Err(invalid());
                };
                super::check::equal(
                    value,
                    &number(cas, coefficient, budget)?.negated(budget)?,
                    budget,
                )?;
            }
        }
        _ => return Err(invalid()),
    }
    Ok(())
}
