// SPDX-License-Identifier: AGPL-3.0-only
//! Actual retained capacities of simultaneous provenance and public projection owners.

use super::{RequestError, add, capacity, multiply, size};
use crate::{
    BuildIntent, BuildRecord, EnvironmentRecord, Evidence, LoweringRecord, PublicProjection,
    Setting,
};

/// Dynamic payload only; the enclosing record or vector charges inline storage.
trait Heap: Sized {
    /// Recursively sum live capacities with checked arithmetic.
    fn heap(&self) -> Result<u64, RequestError>;
    /// Copy through the same fallible allowance as the other retained owners.
    fn owned(&self, budget: &mut super::Budget) -> Result<Self, crate::recipe::RecipeError>;
}

impl Heap for String {
    fn owned(&self, budget: &mut super::Budget) -> Result<Self, crate::recipe::RecipeError> {
        budget.text(self)
    }
    fn heap(&self) -> Result<u64, RequestError> {
        u64::try_from(self.capacity()).map_err(|_| capacity())
    }
}

impl<T: Heap> Heap for Vec<T> {
    fn owned(&self, budget: &mut super::Budget) -> Result<Self, crate::recipe::RecipeError> {
        let mut output = budget.reserve(self.len())?;
        for value in self {
            output.push(value.owned(budget)?);
        }
        Ok(output)
    }
    fn heap(&self) -> Result<u64, RequestError> {
        let fixed = multiply(
            u64::try_from(self.capacity()).map_err(|_| capacity())?,
            size::<T>()?,
        )?;
        self.iter()
            .try_fold(fixed, |bytes, value| add(bytes, value.heap()?))
    }
}

impl<T: Heap> Heap for Evidence<T> {
    fn owned(&self, budget: &mut super::Budget) -> Result<Self, crate::recipe::RecipeError> {
        Ok(match self {
            Self::Known(value) => Self::Known(value.owned(budget)?),
            Self::Unknown(value) => Self::Unknown(value.owned(budget)?),
            Self::Unavailable(value) => Self::Unavailable(value.owned(budget)?),
            Self::Redacted(value) => Self::Redacted(value.owned(budget)?),
        })
    }
    fn heap(&self) -> Result<u64, RequestError> {
        match self {
            Self::Known(value) => value.heap(),
            Self::Unknown(reason) | Self::Unavailable(reason) | Self::Redacted(reason) => {
                reason.heap()
            }
        }
    }
}

/// Scalar evidence has no allocation beyond its containing enum.
macro_rules! scalar {
    ($($kind:ty),+ $(,)?) => {$(
        impl Heap for $kind {
            fn heap(&self) -> Result<u64, RequestError> { Ok(0) }
            fn owned(&self, _: &mut super::Budget) -> Result<Self, crate::recipe::RecipeError> {
                Ok(*self)
            }
        }
    )+};
}
scalar!(bool, BuildIntent, u64, crate::report::ObjectFormat);

/// Every owned dynamic field is named, so projection and private evidence add independently.
macro_rules! fields {
    ($kind:ty: $($field:ident),+ $(,)?) => {
        impl Heap for $kind {
            fn owned(&self, budget: &mut super::Budget) -> Result<Self, crate::recipe::RecipeError> {
                Ok(Self { $($field: self.$field.owned(budget)?,)+ })
            }
            fn heap(&self) -> Result<u64, RequestError> {
                let mut bytes = 0;
                $(bytes = add(bytes, self.$field.heap()?)?;)+
                Ok(bytes)
            }
        }
    };
}
fields!(crate::report::RawContentId: algorithm, digest);
fields!(LoweringRecord: inspected, artifact, symbols, scalar_vectorization, splitting, helpers, native_fma, notes);
fields!(Setting: requested, observed, method, offset_ns);
fields!(BuildRecord: compiler, source_commit, dirty, lockfile, target, profile, flags,
    intent, features, backend, dependencies, lowering);
fields!(EnvironmentRecord: machine_alias, cpu_model, runtime_isa, compile_isa, topology,
    caches, memory, smt, placement, numa, kernel, distribution, resource_limits, power, turbo, tracing, concurrent_load, core_classes, coordinator_placement, worker_placement, numa_allowed_nodes, process_limits, cgroup_cpu_limit, cgroup_memory_limit, cgroup_cpuset, virtual_machine, frequency_min, frequency_max);
fields!(PublicProjection: build, environment, command, redactions);

/// Charge all simultaneously retained capture owners before workload admission.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Used by crate siblings through the owning module boundary."
)]
pub(crate) fn bytes(
    build: &BuildRecord,
    environment: &EnvironmentRecord,
    command: &Vec<String>,
    timestamp: &String,
) -> Result<u64, RequestError> {
    [
        add(size::<BuildRecord>()?, build.heap()?)?,
        add(size::<EnvironmentRecord>()?, environment.heap()?)?,
        add(size::<Vec<String>>()?, command.heap()?)?,
        add(size::<String>()?, timestamp.heap()?)?,
    ]
    .into_iter()
    .try_fold(0, add)
}

impl BuildRecord {
    /// Actual decoded build owner transferred out of capture scratch before host probing.
    pub(crate) fn owned_bytes(&self) -> Result<u64, RequestError> {
        add(size::<Self>()?, self.heap()?)
    }
}

impl PublicProjection {
    /// Copy public evidence only after each actual capacity fits the shared report allowance.
    pub(crate) fn captured(
        build: &BuildRecord,
        environment: &EnvironmentRecord,
        command: &[String],
        budget: &mut super::Budget,
    ) -> Result<Self, crate::recipe::RecipeError> {
        budget.charge(size_of::<Self>())?;
        let mut redactions = budget.reserve(2)?;
        let flags = if let Evidence::Known(value) = &build.flags
            && !value.is_empty()
        {
            redactions.push(
                budget
                    .text("build.flags removed rather than publishing possible absolute paths")?,
            );
            Evidence::Redacted(budget.text(
                "effective flags retained privately; comparison eligibility requires review",
            )?)
        } else {
            build.flags.owned(budget)?
        };
        let public_build = BuildRecord {
            compiler: build.compiler.owned(budget)?,
            source_commit: build.source_commit.owned(budget)?,
            dirty: build.dirty.owned(budget)?,
            lockfile: build.lockfile.owned(budget)?,
            target: build.target.owned(budget)?,
            profile: build.profile.owned(budget)?,
            flags,
            intent: build.intent.owned(budget)?,
            features: build.features.owned(budget)?,
            backend: build.backend.owned(budget)?,
            dependencies: build.dependencies.owned(budget)?,
            lowering: build.lowering.owned(budget)?,
        };
        let mut arguments = budget.reserve(command.len())?;
        let mut redact_next = false;
        for (index, argument) in command.iter().enumerate() {
            let value = if index == 0 {
                "nrz-compute"
            } else if redact_next {
                "redacted-path"
            } else {
                argument
            };
            arguments.push(budget.text(value)?);
            redact_next = argument == "--request" || argument == "--output" || argument == "--raw";
        }
        if !command.is_empty() {
            redactions.push(budget.text("executable and input/output path arguments replaced by non-reversible public tokens")?);
        }
        Ok(Self {
            build: public_build,
            environment: environment.owned(budget)?,
            command: arguments,
            redactions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_accounts_capacity_and_nested_headers() -> Result<(), Box<dyn std::error::Error>> {
        let mut text = String::new();
        text.try_reserve_exact(64)?;
        text.push('x');
        let expected = u64::try_from(text.capacity())?;
        assert_eq!(text.heap()?, expected);
        let values = vec![
            Evidence::Known(text),
            Evidence::Unknown("missing".to_owned()),
        ];
        let headers = u64::try_from(values.capacity() * size_of::<Evidence<String>>())?;
        let reason = match &values[1] {
            Evidence::Unknown(value) => value.capacity(),
            _ => 0,
        };
        assert_eq!(values.heap()?, headers + expected + u64::try_from(reason)?);
        Ok(())
    }

    #[test]
    fn projection_charges_actual_capacities_and_redacts_before_copying()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut summary: crate::SummaryDataset = serde_json::from_slice(include_bytes!(
            "../../../../data/fixtures/compute/valid-complete.json"
        ))?;
        summary.payload.build.flags = Evidence::Known("x".repeat(1_048_576));
        let command = vec![
            "private-program".to_owned(),
            "--raw".to_owned(),
            "x".repeat(1_048_576),
        ];
        let mut budget = super::super::Budget::new(65_536);
        let projection = PublicProjection::captured(
            &summary.payload.build,
            &summary.payload.environment,
            &command,
            &mut budget,
        )?;
        assert_eq!(
            65_536 - budget.remaining(),
            size::<PublicProjection>()? + projection.heap()?
        );
        assert_eq!(
            projection.command,
            ["nrz-compute", "--raw", "redacted-path"]
        );
        assert!(matches!(projection.build.flags, Evidence::Redacted(_)));
        let mut budget = super::super::Budget::new(1);
        assert!(
            PublicProjection::captured(
                &summary.payload.build,
                &summary.payload.environment,
                &command,
                &mut budget
            )
            .is_err()
        );
        let command = vec!["program".to_owned(), "x".repeat(65_537)];
        let mut budget = super::super::Budget::new(65_536);
        assert!(
            PublicProjection::captured(
                &summary.payload.build,
                &summary.payload.environment,
                &command,
                &mut budget
            )
            .is_err()
        );
        Ok(())
    }
}
