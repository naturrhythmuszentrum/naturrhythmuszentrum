// SPDX-License-Identifier: AGPL-3.0-only
//! The existing coefficient equality kernel seals and replays reading identities.

use super::{check, invalid};
use crate::budget::Budget;
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle};
use crate::proof_model::{
    ApplicationDraft, Certificate, DomainContract, Statement, VerifiedWitness,
};
use crate::rule::Primitive;

impl Cas {
    /// Prove an exact equality of composed readings throughout the admitted family.
    ///
    /// The ordinary `poly.jets-equal` checker reconstructs both affine forms over
    /// identical ordered free coordinates. No endpoint value, center or caller equation
    /// supplies authority. The witness retains all original definitions and selections
    /// and uses ordinary `export_witness` and expected-judgment `replay`.
    /// # Errors
    /// Refuses unrelated families, unequal maps, unsupported originals, changed sorts,
    /// foreign handles and exhausted cumulative construction or proof limits.
    pub fn prove_reading_equality(
        &mut self,
        context: &Handle,
        left: &Handle,
        right: &Handle,
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        let ordered = check::applicable(&self.state, self.checked(left)?, self.checked(right)?)?;
        let matrix = crate::polynomial::matrix::applicable(
            &self.state,
            self.checked(left)?,
            self.checked(right)?,
        )?;
        if !ordered && !matrix {
            return Err(invalid(InvalidCode::ReadingDefinition));
        }
        if matrix {
            let previous = budget.restrict_storage(&self.limits);
            let result = (|| {
                crate::polynomial::matrix::definition(&self.state, self.checked(left)?, budget)?;
                crate::polynomial::matrix::definition(&self.state, self.checked(right)?, budget)
            })();
            budget.restore_limits(previous);
            result?;
        }
        let step = ApplicationDraft {
            rule: Primitive::Jets,
            context: context.clone(),
            conclusion: Statement::Equality {
                left: left.clone(),
                right: right.clone(),
                domain: self.admissible_region(context)?,
                domain_contract: DomainContract::DefinedAndEqual,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        };
        if ordered {
            return crate::reading_memo::prove(self, &step, budget);
        }
        self.check_proof(context, &[step], budget)
    }
}
