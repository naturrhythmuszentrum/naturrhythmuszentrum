// SPDX-License-Identifier: AGPL-3.0-only
//! Print committed acceptance measurements; documentation generation never executes this binary.

#[path = "../tests/integration/acceptance/support/mod.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "the example shares private fixtures with the public acceptance harness"
)]
mod support;

use std::io::{self, Write};
use support::{Case, Parameter, ResultOf, Sample};

/// Run exactly one case per process so the server can measure its peak resident memory separately.
fn main() -> ResultOf {
    let selector = std::env::args().nth(1).ok_or("expected truss, resistor, certificate-edge, shared-a-b, stale-factor or bounded-refinement")?;
    let case = match selector.as_str() {
        "truss" => support::truss()?,
        "resistor" => support::resistor()?,
        "certificate-edge" => support::edge()?,
        "shared-a-b" => support::shared()?,
        "stale-factor" => support::stale()?,
        "bounded-refinement" => support::stagnation()?,
        _ => return Err("unknown acceptance case".into()),
    };
    let mut output = io::BufWriter::new(io::stdout().lock());
    write_case(&mut output, &case)?;
    output.flush()?;
    Ok(())
}

/// Wall times and host context are observations, while all numerical counts come from public records.
fn write_case(output: &mut impl Write, case: &Case) -> ResultOf {
    writeln!(
        output,
        "case={} arch={} os={} profile=dev(opt-level=2) crate={}",
        case.name,
        std::env::consts::ARCH,
        std::env::consts::OS,
        env!("CARGO_PKG_VERSION")
    )?;
    writeln!(
        output,
        "server_environment={}",
        std::env::var("NRZ_MEASUREMENT_ENV")
            .unwrap_or_else(|_| "record separately in server reply".into())
    )?;
    writeln!(
        output,
        "factors={} published={} refused={} provenance={:?}",
        case.factors,
        case.samples.len(),
        case.refusals.len(),
        case.provenance
    )?;
    writeln!(
        output,
        "input_region_entries={:?} retained_regions_descriptors={} retained_payload_entries={} retained_payload_bytes={}",
        case.input_regions,
        case.storage()[0],
        case.storage()[1],
        case.storage()[1] * size_of::<f64>()
    )?;
    let accepted = case
        .samples
        .iter()
        .filter(|s| {
            s.solve.as_ref().is_some_and(|p| {
                p.budget()
                    .is_some_and(|b| b.decision() == nrz_accuracy::BudgetDecision::Met)
            })
        })
        .count();
    let requested = case
        .samples
        .iter()
        .filter(|s| {
            s.solve.as_ref().is_some_and(|p| {
                p.budget()
                    .is_some_and(|b| b.decision() != nrz_accuracy::BudgetDecision::NotRequested)
            })
        })
        .count()
        + case
            .refusals
            .iter()
            .filter(|r| r.proof.as_ref().is_some_and(|p| p.budget().is_some()))
            .count();
    writeln!(output, "accepted_requested_budgets={accepted}/{requested}")?;
    for sample in &case.samples {
        write_sample(output, sample)?;
    }
    for (index, block) in case.cross.iter().enumerate() {
        writeln!(output, "cross={index}")?;
        write_parameter(output, block)?;
    }
    for refusal in &case.refusals {
        writeln!(
            output,
            "refusal={:?} stage={:?} attempt_work={}",
            refusal.cause, refusal.stage, refusal.work
        )?;
        if let Some(proof) = &refusal.proof {
            write_solve(output, proof)?;
        }
    }
    for (stage, seconds) in &case.timings {
        writeln!(output, "runtime_seconds {stage}={seconds:.9}")?;
    }
    Ok(())
}

/// Account visible output regions and descriptors without confusing them with allocator overhead.
fn write_sample(output: &mut impl Write, sample: &Sample) -> ResultOf {
    let entries = sample.centers.len()
        + sample.totals.len()
        + sample.errors.len()
        + sample
            .parameters
            .iter()
            .map(|p| p.centers.len() + p.errors.len())
            .sum::<usize>();
    let regions = 3 + 2 * sample.parameters.len();
    writeln!(
        output,
        "output={} entries={} regions={} payload_bytes={} quantity={:?}",
        sample.name,
        entries,
        regions,
        entries * size_of::<f64>(),
        sample.quantity
    )?;
    writeln!(
        output,
        "environment={} stage={:?} assurance={:?} statistical_work={}",
        sample.environment, sample.stage, sample.assurance, sample.statistical_work
    )?;
    writeln!(
        output,
        "centers={:?} total_radii={:?} evaluation_radii={:?}",
        sample.centers, sample.totals, sample.errors
    )?;
    writeln!(
        output,
        "maximum_component_width={:.17e}",
        2.0 * sample.totals.iter().copied().fold(0.0_f64, f64::max)
    )?;
    for parameter in &sample.parameters {
        write_parameter(output, parameter)?;
    }
    writeln!(output, "kernel_evidence={:?}", sample.kernel)?;
    if let Some(proof) = &sample.solve {
        write_solve(output, proof)?;
    }
    Ok(())
}

/// Dense covariance centers and their numerical errors occupy distinct equally sized regions.
fn write_parameter(output: &mut impl Write, parameter: &Parameter) -> ResultOf {
    writeln!(
        output,
        "parameter_shape={:?} entries={} payload_bytes={} first_dimension={:?} last_dimension={:?}",
        parameter.shape,
        parameter.centers.len(),
        (parameter.centers.len() + parameter.errors.len()) * size_of::<f64>(),
        parameter.dimensions.first(),
        parameter.dimensions.last()
    )?;
    writeln!(
        output,
        "maximum_parameter_radius={:.17e}",
        parameter.errors.iter().copied().fold(0.0_f64, f64::max)
    )?;
    Ok(())
}

/// Summarize every theorem stage without printing process-local identities or long parameter vectors.
fn write_solve(output: &mut impl Write, proof: &nrz_matrix::SolveEvidence) -> ResultOf {
    writeln!(
        output,
        "solve_work={} derivative_columns={} precision={:?} revision={} complete={}",
        proof.work(),
        proof.sensitivities().len(),
        proof.precision(),
        proof.arithmetic_revision(),
        proof.is_complete()
    )?;
    if let Some(inventory) = proof.allocation_inventory() {
        write_inventory(output, inventory)?;
    }
    writeln!(
        output,
        "q={:?} h={:?} inverse_margin={:?} g={:?} residual={:?} candidate_norm={:?}",
        proof.inverse_defect(),
        proof.witness_norm(),
        proof.inverse_margin(),
        proof.inverse_norm(),
        proof.residual_norm(),
        proof.candidate_norm()
    )?;
    writeln!(
        output,
        "r_A={:?} r_B={:?} gr_A={:?} family_margin={:?} total_radius={:?} output_radius={:?} numerical_radius={:?} budget={:?}",
        proof.operator_radius(),
        proof.rhs_radius(),
        proof.perturbation_product(),
        proof.perturbation_margin(),
        proof.total_radius(),
        proof.output_radius(),
        proof.numerical_radius(),
        proof.budget()
    )?;
    writeln!(
        output,
        "max_derivative_radius={:?} factor_reused={:?} recovery={:?}",
        proof
            .sensitivities()
            .iter()
            .map(nrz_matrix::SensitivityEvidence::radius)
            .fold(0.0_f64, f64::max),
        proof.factor_use().map(nrz_matrix::FactorUse::reused),
        proof.recovery()
    )?;
    Ok(())
}

/// Scoped observable capacities remain separate from the named shared backend allowance and process RSS.
fn write_inventory(output: &mut impl Write, inventory: &nrz_matrix::SolveInventory) -> ResultOf {
    let allocations = inventory.allocations();
    writeln!(
        output,
        "solve_array_requests={} logical_entries={} float_entries={} observable_cumulative_bytes={} observable_peak_bytes={} metadata_bytes={} backend_name={:?} shared_backend_allowance_bytes={}",
        allocations.len(),
        allocations
            .iter()
            .map(nrz_matrix::SolveAllocation::entries)
            .sum::<usize>(),
        allocations
            .iter()
            .map(nrz_matrix::SolveAllocation::float_entries)
            .sum::<usize>(),
        allocations
            .iter()
            .filter_map(nrz_matrix::SolveAllocation::owned_bytes)
            .sum::<usize>(),
        inventory.peak_payload_bytes(),
        inventory.metadata_bytes(),
        nrz_matrix::SolveInventory::BACKEND_NAME,
        inventory.backend_allowance_bytes()
    )?;
    Ok(())
}
