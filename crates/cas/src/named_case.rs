// SPDX-License-Identifier: AGPL-3.0-only
//! Transparent named functions, checked against their retained canonical definition.
//!
//! # Transparent definitions
//!
//! `Cas::named_case` builds `NamedCase { name, coordinate, parameter }` with a retained canonical case operand. The checked case machinery unfolds abs, sign, ramp and a step whose rational parameter is its value at zero. Ordinary coordinate substitution must use an expanded definition; `rename_axes` transports the coordinate and definition together. Names grant no primitive shortcut.

use crate::boundary_model::{CaseName, NamedCase};
use crate::boundary_regions as syntax;
use crate::budget::Budget;
use crate::case_syntax::{self, invalid};
use crate::domain_model::CasesDraft;
use crate::error::Fault;
use crate::graph::{Cas, Draft, Handle, LogicalId, Reference};
use crate::model::Operator;
use crate::number::Rational;

impl Cas {
    /// Construct a transparent named function on the exact supplied real domain.
    ///
    /// # Errors
    /// Refuses an undeclared coordinate, malformed step parameter or exhausted budget.
    pub fn named_case(
        &mut self,
        domain: &Handle,
        name: &NamedCase,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        self.transaction(budget, |candidate, budget| {
            let expanded = definition(candidate, domain, name, budget)?;
            let result = syntax::term(
                candidate,
                domain,
                Operator::NamedCase(Box::new(name.clone())),
                &[expanded],
                budget,
            )?;
            Ok(result)
        })
    }
}

/// Regenerate the complete definition; a name alone never grants a theorem.
pub(crate) fn check(cas: &mut Cas, id: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
    let value = cas.state.expression(id)?.clone();
    let Operator::NamedCase(name) = value.operator else {
        return Ok(());
    };
    let child = *value.operands.first().ok_or_else(invalid)?;
    let domain = cas.state.expression(child)?.domain.ok_or_else(invalid)?;
    let domain = cas.handle(domain);
    let expected = definition(cas, &domain, &name, budget)?;
    if expected.logical_id() != child || value.operands.len() != 1 {
        return Err(invalid());
    }
    Ok(())
}

/// Every family has separate negative, zero and positive associations.
fn definition(
    cas: &mut Cas,
    domain: &Handle,
    name: &NamedCase,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    if name.parameter.is_some() != (name.name == CaseName::Step) {
        return Err(invalid());
    }
    let coordinate = syntax::term(
        cas,
        domain,
        Operator::Free(name.coordinate.clone()),
        &[],
        budget,
    )?;
    crate::boundary_roles::region(cas, &coordinate, domain, &name.coordinate)?;
    let branches = values(cas, domain, name, &coordinate, budget)?;
    let metadata = cas.state.region(domain.logical_id())?.clone();
    let zero = Rational::zero();
    let mut arms = Vec::new();
    for (index, value) in branches.iter().enumerate() {
        let region = syntax::cell(
            cas,
            domain,
            &name.coordinate,
            std::slice::from_ref(&zero),
            index,
            budget,
        )?;
        let arm = crate::case_ordered::arm(
            cas,
            metadata.header,
            &metadata.scope,
            region.logical_id(),
            value.logical_id(),
            budget,
        )?;
        arms.push(Reference::Existing(arm));
    }
    case_syntax::admit(
        cas,
        Draft::Cases(CasesDraft {
            sort: crate::model::Sort::Value(crate::model::AtomicSort::Scalar(
                crate::model::Field::Real,
            )),
            header: Reference::Existing(cas.handle(metadata.header)),
            scope: metadata.scope,
            domain: Reference::Existing(domain.clone()),
            arms,
        }),
        budget,
    )
}

/// Defining values are literal formulas rather than unrecorded analytic shortcuts.
fn values(
    cas: &mut Cas,
    domain: &Handle,
    name: &NamedCase,
    coordinate: &Handle,
    budget: &mut Budget,
) -> Result<[Handle; 3], Fault> {
    let zero = syntax::term(
        cas,
        domain,
        Operator::Rational(Rational::zero()),
        &[],
        budget,
    )?;
    let one = syntax::term(
        cas,
        domain,
        Operator::Rational(Rational::one()),
        &[],
        budget,
    )?;
    let result = match name.name {
        CaseName::Abs => [
            syntax::term(
                cas,
                domain,
                Operator::Neg,
                std::slice::from_ref(coordinate),
                budget,
            )?,
            zero,
            coordinate.clone(),
        ],
        CaseName::Sign => [
            syntax::term(
                cas,
                domain,
                Operator::Rational(Rational::from_ratio(-1, 1, budget)?),
                &[],
                budget,
            )?,
            zero,
            one,
        ],
        CaseName::Ramp => [zero.clone(), zero, coordinate.clone()],
        CaseName::Step => {
            let height = name.parameter.as_ref().ok_or_else(invalid)?;
            let point = syntax::term(cas, domain, Operator::Rational(height.clone()), &[], budget)?;
            [zero, point, one]
        }
    };
    Ok(result)
}

/// Stable portable name parameters, including the step's actual point value.
pub(crate) fn encode(
    value: &NamedCase,
    out: &mut crate::encoding::Encoder<'_>,
) -> Result<(), Fault> {
    out.byte(match value.name {
        CaseName::Abs => 0,
        CaseName::Sign => 1,
        CaseName::Ramp => 2,
        CaseName::Step => 3,
    })?;
    out.name(&value.coordinate)?;
    out.byte(u8::from(value.parameter.is_some()))?;
    if let Some(parameter) = &value.parameter {
        out.number(parameter)?;
    }
    Ok(())
}

/// Decode exact parameters; admission checks their family-specific shape.
pub(crate) fn decode(reader: &mut crate::wire_read::Reader<'_, '_>) -> Result<NamedCase, Fault> {
    let name = match reader.byte()? {
        0 => CaseName::Abs,
        1 => CaseName::Sign,
        2 => CaseName::Ramp,
        3 => CaseName::Step,
        _ => return Err(invalid()),
    };
    let coordinate = reader.name()?;
    let parameter = if reader.boolean()? {
        Some(reader.rational()?)
    } else {
        None
    };
    Ok(NamedCase {
        name,
        coordinate,
        parameter,
    })
}
