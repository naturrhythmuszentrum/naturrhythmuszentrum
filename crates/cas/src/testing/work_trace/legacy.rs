// SPDX-License-Identifier: AGPL-3.0-only
//! Named receipts preserve observer scope across explicitly measured tariff revisions.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::{Report, Row};
use crate::testing::common::TestResult;

/// Compare every positive bucket and retain coverage of successful zero-charge scopes.
pub(super) fn compare(report: &Report) -> TestResult {
    let thread = std::thread::current();
    let Some(name) = thread.name() else {
        return Ok(());
    };
    let Some(index) = NAMES.iter().position(|candidate| *candidate == name) else {
        return Ok(());
    };
    let ordinal = COUNTERS
        .get(index)
        .ok_or("legacy Work fixture counter")?
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| "legacy Work fixture counter overflow")?;
    let snapshot = SNAPSHOTS
        .split("\n\n")
        .map(str::trim)
        .filter(|snapshot| snapshot.lines().next() == Some(name))
        .nth(ordinal)
        .ok_or_else(|| format!("unexpected Work measurement {ordinal} for {name}"))?;
    let mut expected = Vec::new();
    let mut scopes = true;
    for line in snapshot.lines().skip(1) {
        let parts: Vec<_> = line.split('\t').collect();
        let [rule, phase, location, work] = parts.as_slice() else {
            return Err("invalid legacy Work fixture".into());
        };
        let work = work.parse::<u64>()?;
        if work == 0 {
            scopes &=
                report
                    .receipts
                    .keys()
                    .any(|(_, actual_rule, actual_phase, actual_location)| {
                        actual_rule.as_str() == *rule
                            && actual_phase.as_str() == *phase
                            && actual_location.as_str() == *location
                    });
        } else {
            expected.push(Row {
                rule: (*rule).into(),
                phase: (*phase).into(),
                location: (*location).into(),
                work,
            });
        }
    }
    // The captured fixture predates explicit rendering of operationless phase
    // intervals. Keep its named helper pins exact; ledger reconciliation checks
    // the additional phase rows independently.
    let named_rows: Vec<_> = report
        .rows
        .iter()
        .filter(|row| row.location != super::subscriber::PHASE_LOCATION)
        .cloned()
        .collect();
    if !scopes || expected != named_rows {
        return Err(format!(
            "legacy Work attribution differs for {name} measurement {ordinal}: {:?}",
            report.rows
        )
        .into());
    }
    Ok(())
}

/// One counter per named test preserves the order of cold, warm and refused measurements.
static COUNTERS: [AtomicUsize; NAMES.len()] = [const { AtomicUsize::new(0) }; NAMES.len()];

/// The finite captured test vocabulary bounds comparison state independently of run duration.
const NAMES: &[&str] = &[
    "tests::family_solver::lc::costs::lc_replay_work_attribution",
    "tests::work_trace::work_attribution_preserves_refusal_boundary",
    "tests::work_trace::work_attribution_restores_observation",
    "lemma_cache::tests::lemma_cache_checks_once_across_retries",
    "lemma_cache::tests::lemma_cache_context_is_stale",
    "lemma_cache::tests::lemma_cache_declarations_are_stale",
    "lemma_cache::tests::lemma_cache_premises_are_stale",
    "lemma_cache::tests::lemma_cache_recycled_proof_refusal",
    "lemma_cache::tests::lemma_cache_relation_is_stale",
    "lemma_cache::tests::lemma_cache_representation_is_stale",
    "lemma_cache::tests::lemma_cache_stratum_is_stale",
    "lemma_cache::tests::lifecycle::lemma_cache_committed_generation_lifetime",
    "lemma_cache::tests::lifecycle::lemma_cache_copied_owner_refusal",
    "lemma_cache::tests::lifecycle::lemma_cache_late_refusal",
    "lemma_cache::tests::lifecycle::lemma_cache_owner_refusal",
    "lemma_cache::tests::lifecycle::lemma_cache_scope_refusal",
    "lemma_cache::tests::literals::lemma_cache_literal_scan_growth",
    "tests::family_solver::empty::family_emptiness_resource_measurements",
    "tests::family_solver::lc::refusals::lc_root_factor_coverage_rejects_omissions",
    "tests::family_solver::lc::refusals::lc_root_replay_retains_original_premises",
];

/// The old observer emitted zero buckets only after a successful zero-sized charge.
/// Boundary receipts preserve those scopes without inventing charge events or consumption.
const SNAPSHOTS: &str = r"tests::family_solver::lc::costs::lc_replay_work_attribution
coeff.identity	exclusion.factor-preparation	exact coefficients	794
coeff.identity	exclusion.factor-search	exact coefficients	1040
coeff.identity	exclusion.factor-search	rational constructor	3072
coeff.identity	exclusion.factor-search	rational division	768
coeff.identity	exclusion.factor-search	rational multiplication	7680
coeff.identity	validation	exact coefficients	9085
coeff.identity	validation	local family evaluation	11
coeff.identity	validation	rational addition	6624
coeff.identity	validation	rational comparison	9568
coeff.identity	validation	rational constructor	5120
coeff.identity	validation	rational division	256
coeff.identity	validation	rational multiplication	5440
coeff.identity	validation	rational negation	800
coeff.identity	validation	scalar statement view	682
coeff.identity	validation	semantic header	4
coeff.identity	validation	solving object admission	74
coeff.identity	validation	statement object roles	1
coeff.identity	validation	typed statement	0
ctx.weaken	validation	exact coefficients	4
ctx.weaken	validation	local family evaluation	1036
ctx.weaken	validation	scalar statement view	1106
ctx.weaken	validation	semantic header	4
ctx.weaken	validation	solving object admission	3388
ctx.weaken	validation	statement object roles	2
ctx.weaken	validation	typed statement	0
ctx.weaken	validation	typed syntax descriptor	1856
extension.adjoin	exclusion.factor-preparation	exact coefficients	20
extension.adjoin	validation	exact coefficients	240
extension.adjoin	validation	rational addition	288
extension.adjoin	validation	rational comparison	224
extension.adjoin	validation	rational constructor	128
extension.adjoin	validation	rational negation	32
extension.adjoin	validation	scalar statement view	622
extension.adjoin	validation	semantic header	2
extension.adjoin	validation	solving object admission	38
extension.adjoin	validation	statement object roles	1
extension.adjoin	validation	typed statement	0
extension.unit	exclusion.factor-preparation	exact coefficients	140
extension.unit	validation	exact coefficients	4186
extension.unit	validation	rational addition	4608
extension.unit	validation	rational comparison	5824
extension.unit	validation	rational constructor	2048
extension.unit	validation	rational negation	512
extension.unit	validation	scalar statement view	1504
extension.unit	validation	semantic header	2
extension.unit	validation	solving object admission	148
extension.unit	validation	statement object roles	2
extension.unit	validation	typed statement	0
real.sign	exclusion.factor-preparation	exact coefficients	10
real.sign	exclusion.scalar-nonzero	elementary solving	1
real.sign	exclusion.scalar-nonzero	exact coefficients	18
real.sign	validation	elementary solving	31
real.sign	validation	exact coefficients	986
real.sign	validation	rational addition	288
real.sign	validation	rational comparison	2784
real.sign	validation	rational constructor	256
real.sign	validation	rational multiplication	320
real.sign	validation	rational negation	64
real.sign	validation	real solving certificates	1317
real.sign	validation	scalar family admission	1
real.sign	validation	scalar statement view	2382
real.sign	validation	semantic header	7
real.sign	validation	solving object admission	40
real.sign	validation	statement object roles	3
real.sign	validation	typed statement	0
region.construct	validation	region theorem	1
region.construct	validation	scalar statement view	552
region.construct	validation	semantic header	2
region.construct	validation	statement object roles	1
region.construct	validation	typed statement	0
replay	application reconstruction	interning lookup	2159
replay	application reconstruction	scalar statement view	69588
replay	application reconstruction	structural key	2135
replay	application reconstruction	witness decoding	24433
replay	conclusion reconstruction	scalar statement view	1331
replay	conclusion reconstruction	witness decoding	17
replay	overhead	proof transaction	256
replay	overhead	semantic header	3
replay	overhead	solving object admission	56
replay	overhead	statement object roles	1
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	8
replay	syntax admission	draft references	57
replay	syntax admission	draft validation	16649
replay	syntax admission	exact coefficients	179
replay	syntax admission	interning lookup	6879
replay	syntax admission	local family evaluation	9
replay	syntax admission	native case admission	10
replay	syntax admission	rational comparison	896
replay	syntax admission	rational constructor	2176
replay	syntax admission	rational division	192
replay	syntax admission	rational encoding	864
replay	syntax admission	rational multiplication	2560
replay	syntax admission	scalar family admission	3135
replay	syntax admission	semantic header	118
replay	syntax admission	solving object admission	14324
replay	syntax admission	structural key	6736
replay	syntax admission	transaction	75240
replay	syntax admission	typed object decoding	5
replay	syntax admission	typed syntax descriptor	34803
replay	syntax admission	witness decoding	20364
replay	wire admission	scalar family admission	1
replay	wire admission	semantic header	25
replay	wire admission	solving object admission	358
replay	wire admission	statement object roles	11
replay	wire admission	typed witness import	382553
replay	wire admission	witness decoding	91584
replay	witness encoding	rational encoding	288
replay	witness encoding	scalar statement view	14358
replay	witness encoding	typed witness DAG	265738
replay	witness encoding	typed witness record	4380
root.select	exclusion.component-nonzero	elementary solving	21
root.select	exclusion.component-nonzero	exact coefficients	108
root.select	existence	elementary solving	149
root.select	existence	exact coefficients	27703
root.select	existence	local family evaluation	76
root.select	existence	rational addition	576
root.select	existence	rational comparison	147456
root.select	existence	rational constructor	12032
root.select	existence	rational division	160
root.select	existence	rational multiplication	2240
root.select	existence	rational negation	128
root.select	validation	elementary solving	1
root.select	validation	exact coefficients	8
root.select	validation	scalar statement view	622
root.select	validation	semantic header	2
root.select	validation	solving object admission	38
root.select	validation	statement object roles	1
root.select	validation	typed statement	0
solve.product	exclusion.equation	elementary solving	23
solve.product	exclusion.equation	exact coefficients	1405
solve.product	exclusion.equation	local family evaluation	41
solve.product	exclusion.equation	rational comparison	4480
solve.product	exclusion.equation	rational constructor	128
solve.product	exclusion.equation	rational multiplication	320
solve.product	exclusion.equation	rational negation	64
solve.product	exclusion.factor-preparation	exact coefficients	616
solve.product	exclusion.factor-search	exact coefficients	520
solve.product	exclusion.factor-search	rational constructor	1536
solve.product	exclusion.factor-search	rational division	384
solve.product	exclusion.factor-search	rational multiplication	3840
solve.product	exclusion.factors	elementary solving	91
solve.product	exclusion.factors	exact coefficients	4390
solve.product	exclusion.factors	local family evaluation	300
solve.product	exclusion.factors	rational addition	1120
solve.product	exclusion.factors	rational comparison	14720
solve.product	exclusion.factors	rational constructor	640
solve.product	exclusion.factors	rational division	32
solve.product	exclusion.factors	rational multiplication	320
solve.product	exclusion.factors	rational negation	160
solve.product	exclusion.identity	elementary solving	105
solve.product	exclusion.identity	exact coefficients	3669
solve.product	exclusion.identity	local family evaluation	203
solve.product	exclusion.identity	rational addition	1440
solve.product	exclusion.identity	rational comparison	11808
solve.product	exclusion.identity	rational constructor	1280
solve.product	exclusion.identity	rational multiplication	960
solve.product	exclusion.identity	rational negation	192
solve.product	exclusion.scalar-nonzero	elementary solving	78
solve.product	exclusion.scalar-nonzero	exact coefficients	417
solve.product	exclusion.scalar-nonzero	rational comparison	6272
solve.product	exclusion.sign-leaves	elementary solving	53
solve.product	exclusion.sign-leaves	empty region DAG	7
solve.product	exclusion.sign-leaves	exact coefficients	253
solve.product	exclusion.sign-leaves	local family evaluation	7
solve.product	exclusion.sign-leaves	rational negation	32
solve.product	existence.membership	elementary solving	148
solve.product	existence.membership	exact coefficients	6421
solve.product	existence.membership	local family evaluation	248
solve.product	existence.membership	rational addition	1152
solve.product	existence.membership	rational comparison	18848
solve.product	existence.membership	rational constructor	1408
solve.product	existence.membership	rational multiplication	2240
solve.product	existence.membership	rational negation	256
solve.product	inventory	elementary solving	1
solve.product	inventory	exact coefficients	2
solve.product	validation	elementary solving	2
solve.product	validation	exact coefficients	13
solve.product	validation	scalar statement view	722
solve.product	validation	semantic header	3
solve.product	validation	solving object admission	56
solve.product	validation	statement object roles	1
solve.product	validation	typed statement	0

tests::work_trace::work_attribution_preserves_refusal_boundary
test.root	excluded-zero	inner	2

tests::work_trace::work_attribution_preserves_refusal_boundary

tests::work_trace::work_attribution_restores_observation
test.root	validation	outer	1

tests::work_trace::work_attribution_restores_observation
replay	overhead	later	2

lemma_cache::tests::lemma_cache_checks_once_across_retries
eq.refl	validation	primitive schema	56
eq.refl	validation	proof admission	579
eq.refl	validation	proof conditions	2
eq.refl	validation	scalar statement view	572
eq.refl	validation	semantic header	5
eq.refl	validation	statement object roles	1
eq.refl	validation	typed statement	0
replay	application reconstruction	interning lookup	94
replay	application reconstruction	structural key	92
replay	application reconstruction	witness decoding	17
replay	conclusion reconstruction	checked proof storage	83
replay	conclusion reconstruction	scalar statement view	1161
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	5864
replay	overhead	proof transaction	512
replay	overhead	semantic header	6
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	4
replay	syntax admission	draft references	0
replay	syntax admission	draft validation	783
replay	syntax admission	interning lookup	284
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	20
replay	syntax admission	structural key	275
replay	syntax admission	transaction	3492
replay	syntax admission	typed object decoding	1
replay	syntax admission	witness decoding	582
replay	wire admission	semantic header	3
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	1169
replay	wire admission	witness decoding	5576
replay	witness encoding	checked proof storage	166
replay	witness encoding	scalar statement view	2322
replay	witness encoding	typed witness DAG	8058
replay	witness encoding	typed witness record	192

lemma_cache::tests::lemma_cache_checks_once_across_retries
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	606
replay	overhead	semantic header	3
replay	overhead	statement object roles	1

lemma_cache::tests::lemma_cache_checks_once_across_retries
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	606
replay	overhead	semantic header	3
replay	overhead	statement object roles	1

lemma_cache::tests::lemma_cache_context_is_stale
eq.refl	validation	primitive schema	56
eq.refl	validation	proof admission	579
eq.refl	validation	proof conditions	2
eq.refl	validation	scalar statement view	572
eq.refl	validation	semantic header	5
eq.refl	validation	statement object roles	1
eq.refl	validation	typed statement	0
replay	application reconstruction	interning lookup	94
replay	application reconstruction	structural key	92
replay	application reconstruction	witness decoding	17
replay	conclusion reconstruction	checked proof storage	83
replay	conclusion reconstruction	scalar statement view	1161
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	typed witness DAG	8469
replay	judgment comparison	typed witness record	204
replay	overhead	anonymous lemma cache	7262
replay	overhead	proof transaction	512
replay	overhead	semantic header	6
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context frame	8
replay	syntax admission	context hypotheses	197
replay	syntax admission	draft references	1
replay	syntax admission	draft validation	835
replay	syntax admission	hypothesis atom	19
replay	syntax admission	interning lookup	353
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	22
replay	syntax admission	structural key	343
replay	syntax admission	transaction	4158
replay	syntax admission	typed object decoding	1
replay	syntax admission	witness decoding	791
replay	wire admission	semantic header	3
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	1509
replay	wire admission	witness decoding	6216
replay	witness encoding	checked proof storage	166
replay	witness encoding	scalar statement view	2322
replay	witness encoding	typed witness DAG	9525
replay	witness encoding	typed witness record	225

lemma_cache::tests::lemma_cache_declarations_are_stale
eq.refl	validation	primitive schema	56
eq.refl	validation	proof admission	605
eq.refl	validation	proof conditions	2
eq.refl	validation	scalar statement view	572
eq.refl	validation	semantic header	5
eq.refl	validation	statement object roles	1
eq.refl	validation	typed statement	0
replay	application reconstruction	interning lookup	94
replay	application reconstruction	structural key	92
replay	application reconstruction	witness decoding	17
replay	conclusion reconstruction	checked proof storage	83
replay	conclusion reconstruction	scalar statement view	1161
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	typed witness DAG	7250
replay	judgment comparison	typed witness record	184
replay	overhead	anonymous lemma cache	6704
replay	overhead	proof transaction	512
replay	overhead	semantic header	6
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	4
replay	syntax admission	draft references	0
replay	syntax admission	draft validation	1862
replay	syntax admission	interning lookup	310
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	33
replay	syntax admission	structural key	301
replay	syntax admission	transaction	3531
replay	syntax admission	typed object decoding	1
replay	syntax admission	witness decoding	855
replay	wire admission	semantic header	3
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	1208
replay	wire admission	witness decoding	5615
replay	witness encoding	checked proof storage	166
replay	witness encoding	scalar statement view	2322
replay	witness encoding	typed witness DAG	8305
replay	witness encoding	typed witness record	205

lemma_cache::tests::lemma_cache_premises_are_stale
eq.refl	validation	primitive schema	56
eq.refl	validation	proof admission	579
eq.refl	validation	proof conditions	2
eq.refl	validation	scalar statement view	572
eq.refl	validation	semantic header	5
eq.refl	validation	statement object roles	1
eq.refl	validation	typed statement	0
eq.sym	validation	checked proof storage	83
eq.sym	validation	exact coefficients	1
eq.sym	validation	primitive schema	56
eq.sym	validation	proof admission	589
eq.sym	validation	proof conditions	2
eq.sym	validation	scalar statement view	1077
eq.sym	validation	semantic header	5
eq.sym	validation	statement object roles	1
eq.sym	validation	typed statement	0
replay	application reconstruction	checked proof storage	166
replay	application reconstruction	interning lookup	196
replay	application reconstruction	scalar statement view	2322
replay	application reconstruction	structural key	192
replay	application reconstruction	witness decoding	550
replay	conclusion reconstruction	checked proof storage	91
replay	conclusion reconstruction	scalar statement view	1257
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	6884
replay	overhead	proof transaction	512
replay	overhead	semantic header	6
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	4
replay	syntax admission	draft references	0
replay	syntax admission	draft validation	783
replay	syntax admission	interning lookup	284
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	20
replay	syntax admission	structural key	275
replay	syntax admission	transaction	3492
replay	syntax admission	typed object decoding	1
replay	syntax admission	witness decoding	582
replay	wire admission	semantic header	3
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	1435
replay	wire admission	witness decoding	6270
replay	witness encoding	checked proof storage	265
replay	witness encoding	scalar statement view	3675
replay	witness encoding	typed witness DAG	9415
replay	witness encoding	typed witness record	213

lemma_cache::tests::lemma_cache_recycled_proof_refusal
eq.refl	validation	primitive schema	56
eq.refl	validation	proof admission	579
eq.refl	validation	proof conditions	2
eq.refl	validation	scalar statement view	572
eq.refl	validation	semantic header	5
eq.refl	validation	statement object roles	1
eq.refl	validation	typed statement	0
replay	application reconstruction	interning lookup	94
replay	application reconstruction	structural key	92
replay	application reconstruction	witness decoding	17
replay	conclusion reconstruction	checked proof storage	83
replay	conclusion reconstruction	scalar statement view	1161
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	6470
replay	overhead	proof transaction	512
replay	overhead	semantic header	6
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	4
replay	syntax admission	draft references	0
replay	syntax admission	draft validation	783
replay	syntax admission	interning lookup	284
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	20
replay	syntax admission	structural key	275
replay	syntax admission	transaction	3492
replay	syntax admission	typed object decoding	1
replay	syntax admission	witness decoding	582
replay	wire admission	semantic header	3
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	1169
replay	wire admission	witness decoding	5576
replay	witness encoding	checked proof storage	166
replay	witness encoding	scalar statement view	2322
replay	witness encoding	typed witness DAG	8058
replay	witness encoding	typed witness record	192

lemma_cache::tests::lemma_cache_relation_is_stale
region.construct	validation	region theorem	1
region.construct	validation	scalar statement view	552
region.construct	validation	semantic header	2
region.construct	validation	statement object roles	1
region.construct	validation	typed statement	0
replay	application reconstruction	interning lookup	93
replay	application reconstruction	structural key	91
replay	application reconstruction	witness decoding	17
replay	conclusion reconstruction	scalar statement view	651
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	typed witness DAG	5343
replay	judgment comparison	typed witness record	133
replay	overhead	anonymous lemma cache	5758
replay	overhead	proof transaction	512
replay	overhead	semantic header	4
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	68
replay	syntax admission	context hypotheses	4
replay	syntax admission	draft validation	418
replay	syntax admission	interning lookup	237
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	20
replay	syntax admission	structural key	229
replay	syntax admission	transaction	2892
replay	syntax admission	typed object decoding	1
replay	syntax admission	witness decoding	547
replay	wire admission	semantic header	2
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	854
replay	wire admission	witness decoding	4943
replay	witness encoding	scalar statement view	1302
replay	witness encoding	typed witness DAG	6386
replay	witness encoding	typed witness record	154

lemma_cache::tests::lemma_cache_representation_is_stale
eq.refl	validation	primitive schema	36
eq.refl	validation	proof admission	579
eq.refl	validation	proof conditions	12
eq.refl	validation	scalar statement view	572
eq.refl	validation	semantic header	5
eq.refl	validation	statement object roles	1
eq.refl	validation	typed statement	0
replay	application reconstruction	interning lookup	94
replay	application reconstruction	structural key	92
replay	application reconstruction	witness decoding	17
replay	conclusion reconstruction	checked proof storage	83
replay	conclusion reconstruction	scalar statement view	1161
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	rational encoding	32
replay	judgment comparison	typed witness DAG	10088
replay	judgment comparison	typed witness record	245
replay	overhead	anonymous lemma cache	8198
replay	overhead	proof transaction	512
replay	overhead	semantic header	6
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	4
replay	syntax admission	draft references	2
replay	syntax admission	draft validation	1139
replay	syntax admission	interning lookup	396
replay	syntax admission	rational constructor	128
replay	syntax admission	rational encoding	96
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	22
replay	syntax admission	structural key	385
replay	syntax admission	transaction	4752
replay	syntax admission	typed object decoding	1
replay	syntax admission	witness decoding	786
replay	wire admission	semantic header	3
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	1907
replay	wire admission	witness decoding	6880
replay	witness encoding	checked proof storage	166
replay	witness encoding	rational encoding	32
replay	witness encoding	scalar statement view	2322
replay	witness encoding	typed witness DAG	11145
replay	witness encoding	typed witness record	266

lemma_cache::tests::lemma_cache_stratum_is_stale
eq.refl	validation	primitive schema	56
eq.refl	validation	proof admission	579
eq.refl	validation	proof conditions	2
eq.refl	validation	scalar statement view	572
eq.refl	validation	semantic header	5
eq.refl	validation	statement object roles	1
eq.refl	validation	typed statement	0
replay	application reconstruction	interning lookup	94
replay	application reconstruction	structural key	92
replay	application reconstruction	witness decoding	17
replay	conclusion reconstruction	checked proof storage	83
replay	conclusion reconstruction	scalar statement view	1161
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	typed witness DAG	9205
replay	judgment comparison	typed witness record	212
replay	overhead	anonymous lemma cache	7604
replay	overhead	proof transaction	512
replay	overhead	semantic header	6
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	4
replay	syntax admission	draft references	1
replay	syntax admission	draft validation	854
replay	syntax admission	interning lookup	395
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	23
replay	syntax admission	structural key	383
replay	syntax admission	transaction	4728
replay	syntax admission	typed object decoding	2
replay	syntax admission	witness decoding	689
replay	wire admission	semantic header	3
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	1697
replay	wire admission	witness decoding	6781
replay	witness encoding	checked proof storage	166
replay	witness encoding	scalar statement view	2322
replay	witness encoding	typed witness DAG	10262
replay	witness encoding	typed witness record	233

lemma_cache::tests::lifecycle::lemma_cache_committed_generation_lifetime
eq.refl	validation	primitive schema	56
eq.refl	validation	proof admission	579
eq.refl	validation	proof conditions	2
eq.refl	validation	scalar statement view	572
eq.refl	validation	semantic header	5
eq.refl	validation	statement object roles	1
eq.refl	validation	typed statement	0
replay	application reconstruction	interning lookup	94
replay	application reconstruction	structural key	92
replay	application reconstruction	witness decoding	17
replay	conclusion reconstruction	checked proof storage	83
replay	conclusion reconstruction	scalar statement view	1161
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	6470
replay	overhead	proof transaction	512
replay	overhead	semantic header	6
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	4
replay	syntax admission	draft references	0
replay	syntax admission	draft validation	783
replay	syntax admission	interning lookup	284
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	20
replay	syntax admission	structural key	275
replay	syntax admission	transaction	3492
replay	syntax admission	typed object decoding	1
replay	syntax admission	witness decoding	582
replay	wire admission	semantic header	3
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	1169
replay	wire admission	witness decoding	5576
replay	witness encoding	checked proof storage	166
replay	witness encoding	scalar statement view	2322
replay	witness encoding	typed witness DAG	8058
replay	witness encoding	typed witness record	192

lemma_cache::tests::lifecycle::lemma_cache_committed_generation_lifetime
eq.refl	validation	primitive schema	56
eq.refl	validation	proof admission	579
eq.refl	validation	proof conditions	2
eq.refl	validation	scalar statement view	572
eq.refl	validation	semantic header	5
eq.refl	validation	statement object roles	1
eq.refl	validation	typed statement	0
replay	application reconstruction	interning lookup	94
replay	application reconstruction	structural key	92
replay	application reconstruction	witness decoding	17
replay	conclusion reconstruction	checked proof storage	83
replay	conclusion reconstruction	scalar statement view	1161
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	7076
replay	overhead	proof transaction	512
replay	overhead	semantic header	6
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	4
replay	syntax admission	draft references	0
replay	syntax admission	draft validation	783
replay	syntax admission	interning lookup	284
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	20
replay	syntax admission	structural key	275
replay	syntax admission	transaction	3492
replay	syntax admission	typed object decoding	1
replay	syntax admission	witness decoding	582
replay	wire admission	semantic header	3
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	1169
replay	wire admission	witness decoding	5576
replay	witness encoding	checked proof storage	166
replay	witness encoding	scalar statement view	2322
replay	witness encoding	typed witness DAG	8058
replay	witness encoding	typed witness record	192

lemma_cache::tests::lifecycle::lemma_cache_committed_generation_lifetime
eq.refl	validation	primitive schema	56
eq.refl	validation	proof admission	579
eq.refl	validation	proof conditions	2
eq.refl	validation	scalar statement view	572
eq.refl	validation	semantic header	5
eq.refl	validation	statement object roles	1
eq.refl	validation	typed statement	0
replay	application reconstruction	interning lookup	94
replay	application reconstruction	structural key	92
replay	application reconstruction	witness decoding	17
replay	conclusion reconstruction	checked proof storage	83
replay	conclusion reconstruction	scalar statement view	1161
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	7682
replay	overhead	proof transaction	512
replay	overhead	semantic header	6
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	4
replay	syntax admission	draft references	0
replay	syntax admission	draft validation	783
replay	syntax admission	interning lookup	284
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	20
replay	syntax admission	structural key	275
replay	syntax admission	transaction	3492
replay	syntax admission	typed object decoding	1
replay	syntax admission	witness decoding	582
replay	wire admission	semantic header	3
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	1169
replay	wire admission	witness decoding	5576
replay	witness encoding	checked proof storage	166
replay	witness encoding	scalar statement view	2322
replay	witness encoding	typed witness DAG	8058
replay	witness encoding	typed witness record	192

lemma_cache::tests::lifecycle::lemma_cache_copied_owner_refusal
eq.refl	validation	primitive schema	56
eq.refl	validation	proof admission	579
eq.refl	validation	proof conditions	2
eq.refl	validation	scalar statement view	572
eq.refl	validation	semantic header	5
eq.refl	validation	statement object roles	1
eq.refl	validation	typed statement	0
replay	application reconstruction	interning lookup	94
replay	application reconstruction	structural key	92
replay	application reconstruction	witness decoding	17
replay	conclusion reconstruction	checked proof storage	83
replay	conclusion reconstruction	scalar statement view	1161
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	6470
replay	overhead	proof transaction	512
replay	overhead	semantic header	6
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	4
replay	syntax admission	draft references	0
replay	syntax admission	draft validation	783
replay	syntax admission	interning lookup	284
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	20
replay	syntax admission	structural key	275
replay	syntax admission	transaction	3492
replay	syntax admission	typed object decoding	1
replay	syntax admission	witness decoding	582
replay	wire admission	semantic header	3
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	1169
replay	wire admission	witness decoding	5576
replay	witness encoding	checked proof storage	166
replay	witness encoding	scalar statement view	2322
replay	witness encoding	typed witness DAG	8058
replay	witness encoding	typed witness record	192

lemma_cache::tests::lifecycle::lemma_cache_late_refusal
eq.refl	validation	primitive schema	56
eq.refl	validation	proof admission	579
eq.refl	validation	proof conditions	2
eq.refl	validation	scalar statement view	572
eq.refl	validation	semantic header	5
eq.refl	validation	statement object roles	1
eq.refl	validation	typed statement	0
replay	application reconstruction	interning lookup	94
replay	application reconstruction	structural key	92
replay	application reconstruction	witness decoding	17
replay	conclusion reconstruction	checked proof storage	83
replay	conclusion reconstruction	scalar statement view	1161
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	6470
replay	overhead	proof transaction	512
replay	overhead	semantic header	6
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	4
replay	syntax admission	draft references	0
replay	syntax admission	draft validation	783
replay	syntax admission	interning lookup	284
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	20
replay	syntax admission	structural key	275
replay	syntax admission	transaction	3492
replay	syntax admission	typed object decoding	1
replay	syntax admission	witness decoding	582
replay	wire admission	semantic header	3
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	1169
replay	wire admission	witness decoding	5576
replay	witness encoding	checked proof storage	166
replay	witness encoding	scalar statement view	2322
replay	witness encoding	typed witness DAG	8058
replay	witness encoding	typed witness record	192

lemma_cache::tests::lifecycle::lemma_cache_owner_refusal
eq.refl	validation	primitive schema	56
eq.refl	validation	proof admission	579
eq.refl	validation	proof conditions	2
eq.refl	validation	scalar statement view	572
eq.refl	validation	semantic header	5
eq.refl	validation	statement object roles	1
eq.refl	validation	typed statement	0
replay	application reconstruction	interning lookup	94
replay	application reconstruction	structural key	92
replay	application reconstruction	witness decoding	17
replay	conclusion reconstruction	checked proof storage	83
replay	conclusion reconstruction	scalar statement view	1161
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	6470
replay	overhead	proof transaction	512
replay	overhead	semantic header	6
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	4
replay	syntax admission	draft references	0
replay	syntax admission	draft validation	783
replay	syntax admission	interning lookup	284
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	20
replay	syntax admission	structural key	275
replay	syntax admission	transaction	3492
replay	syntax admission	typed object decoding	1
replay	syntax admission	witness decoding	582
replay	wire admission	semantic header	3
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	1169
replay	wire admission	witness decoding	5576
replay	witness encoding	checked proof storage	166
replay	witness encoding	scalar statement view	2322
replay	witness encoding	typed witness DAG	8058
replay	witness encoding	typed witness record	192

lemma_cache::tests::lifecycle::lemma_cache_scope_refusal
eq.refl	validation	primitive schema	56
eq.refl	validation	proof admission	579
eq.refl	validation	proof conditions	2
eq.refl	validation	scalar statement view	572
eq.refl	validation	semantic header	5
eq.refl	validation	statement object roles	1
eq.refl	validation	typed statement	0
replay	application reconstruction	interning lookup	94
replay	application reconstruction	structural key	92
replay	application reconstruction	witness decoding	17
replay	conclusion reconstruction	checked proof storage	83
replay	conclusion reconstruction	scalar statement view	1161
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	6470
replay	overhead	proof transaction	512
replay	overhead	semantic header	6
replay	overhead	statement object roles	2
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	4
replay	syntax admission	draft references	0
replay	syntax admission	draft validation	783
replay	syntax admission	interning lookup	284
replay	syntax admission	scalar family admission	0
replay	syntax admission	semantic header	20
replay	syntax admission	structural key	275
replay	syntax admission	transaction	3492
replay	syntax admission	typed object decoding	1
replay	syntax admission	witness decoding	582
replay	wire admission	semantic header	3
replay	wire admission	statement object roles	1
replay	wire admission	typed witness import	1169
replay	wire admission	witness decoding	5576
replay	witness encoding	checked proof storage	166
replay	witness encoding	scalar statement view	2322
replay	witness encoding	typed witness DAG	8058
replay	witness encoding	typed witness record	192

lemma_cache::tests::literals::lemma_cache_literal_scan_growth
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	606
replay	overhead	semantic header	3
replay	overhead	statement object roles	1

lemma_cache::tests::literals::lemma_cache_literal_scan_growth
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	1212
replay	overhead	semantic header	3
replay	overhead	statement object roles	1

lemma_cache::tests::literals::lemma_cache_literal_scan_growth
replay	judgment comparison	typed witness DAG	7003
replay	judgment comparison	typed witness record	171
replay	overhead	anonymous lemma cache	1818
replay	overhead	semantic header	3
replay	overhead	statement object roles	1

tests::family_solver::empty::family_emptiness_resource_measurements
replay	overhead	draft validation	108
replay	overhead	interning lookup	241
replay	overhead	proof transaction	256
replay	overhead	scalar family admission	5
replay	overhead	scalar statement view	1632
replay	overhead	solving object admission	850
replay	overhead	structural key	236
replay	overhead	transaction	1878
replay	overhead	typed syntax descriptor	128
replay	witness encoding	rational encoding	64
replay	witness encoding	scalar statement view	3124
replay	witness encoding	typed witness DAG	57581
replay	witness encoding	typed witness record	1218
solve.split	validation	elementary solving	2
solve.split	validation	exact coefficients	3
solve.split	validation	scalar statement view	638
solve.split	validation	semantic header	3
solve.split	validation	solving object admission	56
solve.split	validation	statement object roles	1
solve.split	validation	typed statement	0

tests::family_solver::empty::family_emptiness_resource_measurements
region.construct	validation	region theorem	1
region.construct	validation	scalar statement view	534
region.construct	validation	semantic header	1
region.construct	validation	statement object roles	1
region.construct	validation	typed statement	0
region.decide	validation	empty region DAG	728
region.decide	validation	exact coefficients	2
region.decide	validation	scalar statement view	578
region.decide	validation	semantic header	1
region.decide	validation	statement object roles	1
region.decide	validation	typed statement	0
replay	application reconstruction	interning lookup	336
replay	application reconstruction	scalar statement view	2704
replay	application reconstruction	structural key	330
replay	application reconstruction	witness decoding	1251
replay	conclusion reconstruction	scalar statement view	886
replay	conclusion reconstruction	witness decoding	17
replay	overhead	proof transaction	256
replay	syntax admission	context hypotheses	4
replay	syntax admission	domain admission	27
replay	syntax admission	draft references	12
replay	syntax admission	draft validation	7995
replay	syntax admission	interning lookup	2600
replay	syntax admission	native case admission	8
replay	syntax admission	rational constructor	256
replay	syntax admission	rational encoding	256
replay	syntax admission	scalar family admission	1557
replay	syntax admission	semantic header	62
replay	syntax admission	solving object admission	3245
replay	syntax admission	structural key	2545
replay	syntax admission	transaction	23522
replay	syntax admission	typed syntax descriptor	8049
replay	syntax admission	witness decoding	7478
replay	wire admission	semantic header	4
replay	wire admission	solving object admission	56
replay	wire admission	statement object roles	2
replay	wire admission	typed witness import	30321
replay	wire admission	witness decoding	27300
replay	witness encoding	rational encoding	64
replay	witness encoding	scalar statement view	3124
replay	witness encoding	typed witness DAG	57581
replay	witness encoding	typed witness record	1218
solve.split	validation	elementary solving	2
solve.split	validation	exact coefficients	3
solve.split	validation	scalar statement view	638
solve.split	validation	semantic header	3
solve.split	validation	solving object admission	56
solve.split	validation	statement object roles	1
solve.split	validation	typed statement	0

tests::family_solver::lc::refusals::lc_root_factor_coverage_rejects_omissions
coeff.identity	exclusion.factor-preparation	exact coefficients	794
coeff.identity	exclusion.factor-search	exact coefficients	1040
coeff.identity	exclusion.factor-search	rational constructor	3072
coeff.identity	exclusion.factor-search	rational division	768
coeff.identity	exclusion.factor-search	rational multiplication	7680
coeff.identity	validation	exact coefficients	9085
coeff.identity	validation	local family evaluation	11
coeff.identity	validation	rational addition	6624
coeff.identity	validation	rational comparison	9568
coeff.identity	validation	rational constructor	5120
coeff.identity	validation	rational division	256
coeff.identity	validation	rational multiplication	5440
coeff.identity	validation	rational negation	800
coeff.identity	validation	scalar statement view	682
coeff.identity	validation	semantic header	4
coeff.identity	validation	solving object admission	74
coeff.identity	validation	statement object roles	1
coeff.identity	validation	typed statement	0
ctx.weaken	validation	exact coefficients	4
ctx.weaken	validation	local family evaluation	1036
ctx.weaken	validation	scalar statement view	1106
ctx.weaken	validation	semantic header	4
ctx.weaken	validation	solving object admission	3388
ctx.weaken	validation	statement object roles	2
ctx.weaken	validation	typed statement	0
ctx.weaken	validation	typed syntax descriptor	1856
extension.adjoin	exclusion.factor-preparation	exact coefficients	20
extension.adjoin	validation	exact coefficients	240
extension.adjoin	validation	rational addition	288
extension.adjoin	validation	rational comparison	224
extension.adjoin	validation	rational constructor	128
extension.adjoin	validation	rational negation	32
extension.adjoin	validation	scalar statement view	622
extension.adjoin	validation	semantic header	2
extension.adjoin	validation	solving object admission	38
extension.adjoin	validation	statement object roles	1
extension.adjoin	validation	typed statement	0
extension.unit	exclusion.factor-preparation	exact coefficients	140
extension.unit	validation	exact coefficients	4186
extension.unit	validation	rational addition	4608
extension.unit	validation	rational comparison	5824
extension.unit	validation	rational constructor	2048
extension.unit	validation	rational negation	512
extension.unit	validation	scalar statement view	1504
extension.unit	validation	semantic header	2
extension.unit	validation	solving object admission	148
extension.unit	validation	statement object roles	2
extension.unit	validation	typed statement	0
real.sign	exclusion.factor-preparation	exact coefficients	10
real.sign	exclusion.scalar-nonzero	elementary solving	1
real.sign	exclusion.scalar-nonzero	exact coefficients	18
real.sign	validation	elementary solving	31
real.sign	validation	exact coefficients	986
real.sign	validation	rational addition	288
real.sign	validation	rational comparison	2784
real.sign	validation	rational constructor	256
real.sign	validation	rational multiplication	320
real.sign	validation	rational negation	64
real.sign	validation	real solving certificates	1319
real.sign	validation	scalar family admission	1
real.sign	validation	scalar statement view	2382
real.sign	validation	semantic header	7
real.sign	validation	solving object admission	40
real.sign	validation	statement object roles	3
real.sign	validation	typed statement	0
region.construct	validation	region theorem	1
region.construct	validation	scalar statement view	552
region.construct	validation	semantic header	2
region.construct	validation	statement object roles	1
region.construct	validation	typed statement	0
replay	overhead	interning lookup	1930
replay	overhead	proof transaction	256
replay	overhead	scalar statement view	77780
replay	overhead	structural key	1908
replay	overhead	typed proof admission	420
root.select	exclusion.component-nonzero	elementary solving	21
root.select	exclusion.component-nonzero	exact coefficients	108
root.select	existence	elementary solving	149
root.select	existence	exact coefficients	27703
root.select	existence	local family evaluation	76
root.select	existence	rational addition	576
root.select	existence	rational comparison	147456
root.select	existence	rational constructor	12032
root.select	existence	rational division	160
root.select	existence	rational multiplication	2240
root.select	existence	rational negation	128
root.select	validation	elementary solving	1
root.select	validation	exact coefficients	8
root.select	validation	scalar statement view	622
root.select	validation	semantic header	2
root.select	validation	solving object admission	38
root.select	validation	statement object roles	1
root.select	validation	typed statement	0
solve.product	exclusion.factor-preparation	exact coefficients	368
solve.product	exclusion.factor-search	exact coefficients	260
solve.product	exclusion.factor-search	rational constructor	768
solve.product	exclusion.factor-search	rational division	192
solve.product	exclusion.factor-search	rational multiplication	1920
solve.product	exclusion.factors	elementary solving	152
solve.product	exclusion.factors	exact coefficients	4020
solve.product	exclusion.factors	local family evaluation	253
solve.product	exclusion.factors	rational addition	544
solve.product	exclusion.factors	rational comparison	9984
solve.product	exclusion.factors	rational constructor	384
solve.product	exclusion.factors	rational division	32
solve.product	exclusion.factors	rational multiplication	320
solve.product	exclusion.factors	rational negation	96
solve.product	exclusion.scalar-nonzero	elementary solving	34
solve.product	exclusion.scalar-nonzero	exact coefficients	189
solve.product	exclusion.scalar-nonzero	rational comparison	2688
solve.product	exclusion.sign-leaves	elementary solving	53
solve.product	exclusion.sign-leaves	empty region DAG	7
solve.product	exclusion.sign-leaves	exact coefficients	253
solve.product	exclusion.sign-leaves	local family evaluation	7
solve.product	exclusion.sign-leaves	rational negation	32
solve.product	existence.membership	elementary solving	74
solve.product	existence.membership	exact coefficients	3185
solve.product	existence.membership	local family evaluation	124
solve.product	existence.membership	rational addition	576
solve.product	existence.membership	rational comparison	9184
solve.product	existence.membership	rational constructor	640
solve.product	existence.membership	rational multiplication	960
solve.product	existence.membership	rational negation	128
solve.product	inventory	elementary solving	1
solve.product	inventory	exact coefficients	1
solve.product	validation	elementary solving	2
solve.product	validation	exact coefficients	13
solve.product	validation	scalar statement view	722
solve.product	validation	typed statement	0

tests::family_solver::lc::refusals::lc_root_factor_coverage_rejects_omissions
coeff.identity	exclusion.factor-preparation	exact coefficients	794
coeff.identity	exclusion.factor-search	exact coefficients	1040
coeff.identity	exclusion.factor-search	rational constructor	3072
coeff.identity	exclusion.factor-search	rational division	768
coeff.identity	exclusion.factor-search	rational multiplication	7680
coeff.identity	validation	exact coefficients	9085
coeff.identity	validation	local family evaluation	11
coeff.identity	validation	rational addition	6624
coeff.identity	validation	rational comparison	9568
coeff.identity	validation	rational constructor	5120
coeff.identity	validation	rational division	256
coeff.identity	validation	rational multiplication	5440
coeff.identity	validation	rational negation	800
coeff.identity	validation	scalar statement view	682
coeff.identity	validation	semantic header	4
coeff.identity	validation	solving object admission	74
coeff.identity	validation	statement object roles	1
coeff.identity	validation	typed statement	0
ctx.weaken	validation	exact coefficients	4
ctx.weaken	validation	local family evaluation	1036
ctx.weaken	validation	scalar statement view	1106
ctx.weaken	validation	semantic header	4
ctx.weaken	validation	solving object admission	3388
ctx.weaken	validation	statement object roles	2
ctx.weaken	validation	typed statement	0
ctx.weaken	validation	typed syntax descriptor	1856
extension.adjoin	exclusion.factor-preparation	exact coefficients	20
extension.adjoin	validation	exact coefficients	240
extension.adjoin	validation	rational addition	288
extension.adjoin	validation	rational comparison	224
extension.adjoin	validation	rational constructor	128
extension.adjoin	validation	rational negation	32
extension.adjoin	validation	scalar statement view	622
extension.adjoin	validation	semantic header	2
extension.adjoin	validation	solving object admission	38
extension.adjoin	validation	statement object roles	1
extension.adjoin	validation	typed statement	0
extension.unit	exclusion.factor-preparation	exact coefficients	140
extension.unit	validation	exact coefficients	4186
extension.unit	validation	rational addition	4608
extension.unit	validation	rational comparison	5824
extension.unit	validation	rational constructor	2048
extension.unit	validation	rational negation	512
extension.unit	validation	scalar statement view	1504
extension.unit	validation	semantic header	2
extension.unit	validation	solving object admission	148
extension.unit	validation	statement object roles	2
extension.unit	validation	typed statement	0
real.sign	exclusion.factor-preparation	exact coefficients	10
real.sign	exclusion.scalar-nonzero	elementary solving	1
real.sign	exclusion.scalar-nonzero	exact coefficients	18
real.sign	validation	elementary solving	31
real.sign	validation	exact coefficients	986
real.sign	validation	rational addition	288
real.sign	validation	rational comparison	2784
real.sign	validation	rational constructor	256
real.sign	validation	rational multiplication	320
real.sign	validation	rational negation	64
real.sign	validation	real solving certificates	1319
real.sign	validation	scalar family admission	1
real.sign	validation	scalar statement view	2382
real.sign	validation	semantic header	7
real.sign	validation	solving object admission	40
real.sign	validation	statement object roles	3
real.sign	validation	typed statement	0
region.construct	validation	region theorem	1
region.construct	validation	scalar statement view	552
region.construct	validation	semantic header	2
region.construct	validation	statement object roles	1
region.construct	validation	typed statement	0
replay	overhead	interning lookup	1930
replay	overhead	proof transaction	256
replay	overhead	scalar statement view	77754
replay	overhead	structural key	1908
replay	overhead	typed proof admission	420
root.select	exclusion.component-nonzero	elementary solving	21
root.select	exclusion.component-nonzero	exact coefficients	108
root.select	existence	elementary solving	149
root.select	existence	exact coefficients	27703
root.select	existence	local family evaluation	76
root.select	existence	rational addition	576
root.select	existence	rational comparison	147456
root.select	existence	rational constructor	12032
root.select	existence	rational division	160
root.select	existence	rational multiplication	2240
root.select	existence	rational negation	128
root.select	validation	elementary solving	1
root.select	validation	exact coefficients	8
root.select	validation	scalar statement view	622
root.select	validation	semantic header	2
root.select	validation	solving object admission	38
root.select	validation	statement object roles	1
root.select	validation	typed statement	0
solve.product	exclusion.equation	elementary solving	23
solve.product	exclusion.equation	exact coefficients	1405
solve.product	exclusion.equation	local family evaluation	41
solve.product	exclusion.equation	rational comparison	4480
solve.product	exclusion.equation	rational constructor	128
solve.product	exclusion.equation	rational multiplication	320
solve.product	exclusion.equation	rational negation	64
solve.product	exclusion.factor-preparation	exact coefficients	764
solve.product	exclusion.factor-search	exact coefficients	780
solve.product	exclusion.factor-search	rational constructor	2304
solve.product	exclusion.factor-search	rational division	576
solve.product	exclusion.factor-search	rational multiplication	5760
solve.product	exclusion.factors	elementary solving	30
solve.product	exclusion.factors	exact coefficients	1521
solve.product	exclusion.factors	local family evaluation	123
solve.product	exclusion.factors	rational addition	288
solve.product	exclusion.factors	rational comparison	4704
solve.product	exclusion.factors	rational constructor	128
solve.product	exclusion.factors	rational negation	64
solve.product	exclusion.identity	elementary solving	219
solve.product	exclusion.identity	exact coefficients	7031
solve.product	exclusion.identity	local family evaluation	311
solve.product	exclusion.identity	rational addition	1728
solve.product	exclusion.identity	rational comparison	21056
solve.product	exclusion.identity	rational constructor	1792
solve.product	exclusion.identity	rational multiplication	1920
solve.product	exclusion.identity	rational negation	384
solve.product	exclusion.scalar-nonzero	elementary solving	78
solve.product	exclusion.scalar-nonzero	exact coefficients	417
solve.product	exclusion.scalar-nonzero	rational comparison	6272
solve.product	exclusion.sign-leaves	elementary solving	53
solve.product	exclusion.sign-leaves	empty region DAG	7
solve.product	exclusion.sign-leaves	exact coefficients	253
solve.product	exclusion.sign-leaves	local family evaluation	7
solve.product	exclusion.sign-leaves	rational negation	32
solve.product	existence.membership	elementary solving	148
solve.product	existence.membership	exact coefficients	6421
solve.product	existence.membership	local family evaluation	248
solve.product	existence.membership	rational addition	1152
solve.product	existence.membership	rational comparison	18848
solve.product	existence.membership	rational constructor	1408
solve.product	existence.membership	rational multiplication	2240
solve.product	existence.membership	rational negation	256
solve.product	inventory	elementary solving	1
solve.product	inventory	exact coefficients	2
solve.product	validation	elementary solving	2
solve.product	validation	exact coefficients	13
solve.product	validation	scalar statement view	696
solve.product	validation	typed statement	0

tests::family_solver::lc::refusals::lc_root_replay_retains_original_premises
coeff.identity	exclusion.factor-preparation	exact coefficients	794
coeff.identity	exclusion.factor-search	exact coefficients	1040
coeff.identity	exclusion.factor-search	rational constructor	3072
coeff.identity	exclusion.factor-search	rational division	768
coeff.identity	exclusion.factor-search	rational multiplication	7680
coeff.identity	validation	exact coefficients	9085
coeff.identity	validation	local family evaluation	11
coeff.identity	validation	rational addition	6624
coeff.identity	validation	rational comparison	9568
coeff.identity	validation	rational constructor	5120
coeff.identity	validation	rational division	256
coeff.identity	validation	rational multiplication	5440
coeff.identity	validation	rational negation	800
coeff.identity	validation	scalar statement view	682
coeff.identity	validation	semantic header	4
coeff.identity	validation	solving object admission	74
coeff.identity	validation	statement object roles	1
coeff.identity	validation	typed statement	0
ctx.weaken	validation	exact coefficients	4
ctx.weaken	validation	local family evaluation	1036
ctx.weaken	validation	scalar statement view	1106
ctx.weaken	validation	semantic header	4
ctx.weaken	validation	solving object admission	3388
ctx.weaken	validation	statement object roles	2
ctx.weaken	validation	typed statement	0
ctx.weaken	validation	typed syntax descriptor	1856
extension.adjoin	exclusion.factor-preparation	exact coefficients	20
extension.adjoin	validation	exact coefficients	240
extension.adjoin	validation	rational addition	288
extension.adjoin	validation	rational comparison	224
extension.adjoin	validation	rational constructor	128
extension.adjoin	validation	rational negation	32
extension.adjoin	validation	scalar statement view	622
extension.adjoin	validation	semantic header	2
extension.adjoin	validation	solving object admission	38
extension.adjoin	validation	statement object roles	1
extension.adjoin	validation	typed statement	0
extension.unit	exclusion.factor-preparation	exact coefficients	140
extension.unit	validation	exact coefficients	4186
extension.unit	validation	rational addition	4608
extension.unit	validation	rational comparison	5824
extension.unit	validation	rational constructor	2048
extension.unit	validation	rational negation	512
extension.unit	validation	scalar statement view	1504
extension.unit	validation	semantic header	2
extension.unit	validation	solving object admission	148
extension.unit	validation	statement object roles	2
extension.unit	validation	typed statement	0
real.sign	exclusion.factor-preparation	exact coefficients	10
real.sign	exclusion.scalar-nonzero	elementary solving	1
real.sign	exclusion.scalar-nonzero	exact coefficients	18
real.sign	validation	elementary solving	31
real.sign	validation	exact coefficients	986
real.sign	validation	rational addition	288
real.sign	validation	rational comparison	2784
real.sign	validation	rational constructor	256
real.sign	validation	rational multiplication	320
real.sign	validation	rational negation	64
real.sign	validation	real solving certificates	1292
real.sign	validation	scalar family admission	1
real.sign	validation	scalar statement view	2382
real.sign	validation	semantic header	7
real.sign	validation	solving object admission	40
real.sign	validation	statement object roles	3
real.sign	validation	typed statement	0
region.construct	validation	region theorem	1
region.construct	validation	scalar statement view	552
region.construct	validation	semantic header	2
region.construct	validation	statement object roles	1
region.construct	validation	typed statement	0
replay	application reconstruction	interning lookup	2159
replay	application reconstruction	scalar statement view	69588
replay	application reconstruction	structural key	2135
replay	application reconstruction	witness decoding	24433
replay	conclusion reconstruction	scalar statement view	1331
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	rational encoding	448
replay	judgment comparison	typed witness DAG	214217
replay	judgment comparison	typed witness record	4298
replay	overhead	proof transaction	256
replay	overhead	semantic header	3
replay	overhead	solving object admission	56
replay	overhead	statement object roles	1
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	8
replay	syntax admission	draft references	57
replay	syntax admission	draft validation	16649
replay	syntax admission	exact coefficients	179
replay	syntax admission	interning lookup	6879
replay	syntax admission	local family evaluation	9
replay	syntax admission	native case admission	10
replay	syntax admission	rational comparison	896
replay	syntax admission	rational constructor	2176
replay	syntax admission	rational division	192
replay	syntax admission	rational encoding	864
replay	syntax admission	rational multiplication	2560
replay	syntax admission	scalar family admission	3135
replay	syntax admission	semantic header	118
replay	syntax admission	solving object admission	14324
replay	syntax admission	structural key	6736
replay	syntax admission	transaction	75246
replay	syntax admission	typed object decoding	5
replay	syntax admission	typed syntax descriptor	34803
replay	syntax admission	witness decoding	20364
replay	wire admission	scalar family admission	1
replay	wire admission	semantic header	25
replay	wire admission	solving object admission	358
replay	wire admission	statement object roles	11
replay	wire admission	typed witness import	382553
replay	wire admission	witness decoding	91584
replay	witness encoding	rational encoding	288
replay	witness encoding	scalar statement view	14358
replay	witness encoding	typed witness DAG	265738
replay	witness encoding	typed witness record	4380
root.select	exclusion.component-nonzero	elementary solving	21
root.select	exclusion.component-nonzero	exact coefficients	108
root.select	existence	elementary solving	149
root.select	existence	exact coefficients	27703
root.select	existence	local family evaluation	76
root.select	existence	rational addition	576
root.select	existence	rational comparison	147456
root.select	existence	rational constructor	12032
root.select	existence	rational division	160
root.select	existence	rational multiplication	2240
root.select	existence	rational negation	128
root.select	validation	elementary solving	1
root.select	validation	exact coefficients	8
root.select	validation	scalar statement view	622
root.select	validation	semantic header	2
root.select	validation	solving object admission	38
root.select	validation	statement object roles	1
root.select	validation	typed statement	0
solve.product	exclusion.equation	elementary solving	23
solve.product	exclusion.equation	exact coefficients	1405
solve.product	exclusion.equation	local family evaluation	41
solve.product	exclusion.equation	rational comparison	4480
solve.product	exclusion.equation	rational constructor	128
solve.product	exclusion.equation	rational multiplication	320
solve.product	exclusion.equation	rational negation	64
solve.product	exclusion.factor-preparation	exact coefficients	616
solve.product	exclusion.factor-search	exact coefficients	520
solve.product	exclusion.factor-search	rational constructor	1536
solve.product	exclusion.factor-search	rational division	384
solve.product	exclusion.factor-search	rational multiplication	3840
solve.product	exclusion.factors	elementary solving	91
solve.product	exclusion.factors	exact coefficients	4390
solve.product	exclusion.factors	local family evaluation	300
solve.product	exclusion.factors	rational addition	1120
solve.product	exclusion.factors	rational comparison	14720
solve.product	exclusion.factors	rational constructor	640
solve.product	exclusion.factors	rational division	32
solve.product	exclusion.factors	rational multiplication	320
solve.product	exclusion.factors	rational negation	160
solve.product	exclusion.identity	elementary solving	105
solve.product	exclusion.identity	exact coefficients	3669
solve.product	exclusion.identity	local family evaluation	203
solve.product	exclusion.identity	rational addition	1440
solve.product	exclusion.identity	rational comparison	11808
solve.product	exclusion.identity	rational constructor	1280
solve.product	exclusion.identity	rational multiplication	960
solve.product	exclusion.identity	rational negation	192
solve.product	exclusion.scalar-nonzero	elementary solving	78
solve.product	exclusion.scalar-nonzero	exact coefficients	417
solve.product	exclusion.scalar-nonzero	rational comparison	6272
solve.product	exclusion.sign-leaves	elementary solving	53
solve.product	exclusion.sign-leaves	empty region DAG	7
solve.product	exclusion.sign-leaves	exact coefficients	253
solve.product	exclusion.sign-leaves	local family evaluation	7
solve.product	exclusion.sign-leaves	rational negation	32
solve.product	existence.membership	elementary solving	148
solve.product	existence.membership	exact coefficients	6421
solve.product	existence.membership	local family evaluation	248
solve.product	existence.membership	rational addition	1152
solve.product	existence.membership	rational comparison	18848
solve.product	existence.membership	rational constructor	1408
solve.product	existence.membership	rational multiplication	2240
solve.product	existence.membership	rational negation	256
solve.product	inventory	elementary solving	1
solve.product	inventory	exact coefficients	2
solve.product	validation	elementary solving	2
solve.product	validation	exact coefficients	13
solve.product	validation	scalar statement view	722
solve.product	validation	semantic header	3
solve.product	validation	solving object admission	56
solve.product	validation	statement object roles	1
solve.product	validation	typed statement	0

tests::family_solver::lc::refusals::lc_root_replay_retains_original_premises
coeff.identity	exclusion.factor-preparation	exact coefficients	794
coeff.identity	exclusion.factor-search	exact coefficients	1040
coeff.identity	exclusion.factor-search	rational constructor	3072
coeff.identity	exclusion.factor-search	rational division	768
coeff.identity	exclusion.factor-search	rational multiplication	7680
coeff.identity	validation	exact coefficients	9085
coeff.identity	validation	local family evaluation	11
coeff.identity	validation	rational addition	6624
coeff.identity	validation	rational comparison	9568
coeff.identity	validation	rational constructor	5120
coeff.identity	validation	rational division	256
coeff.identity	validation	rational multiplication	5440
coeff.identity	validation	rational negation	800
coeff.identity	validation	scalar statement view	682
coeff.identity	validation	semantic header	4
coeff.identity	validation	solving object admission	74
coeff.identity	validation	statement object roles	1
coeff.identity	validation	typed statement	0
ctx.weaken	validation	exact coefficients	4
ctx.weaken	validation	local family evaluation	1036
ctx.weaken	validation	scalar statement view	1106
ctx.weaken	validation	semantic header	4
ctx.weaken	validation	solving object admission	3388
ctx.weaken	validation	statement object roles	2
ctx.weaken	validation	typed statement	0
ctx.weaken	validation	typed syntax descriptor	1856
extension.adjoin	exclusion.factor-preparation	exact coefficients	20
extension.adjoin	validation	exact coefficients	240
extension.adjoin	validation	rational addition	288
extension.adjoin	validation	rational comparison	224
extension.adjoin	validation	rational constructor	128
extension.adjoin	validation	rational negation	32
extension.adjoin	validation	scalar statement view	622
extension.adjoin	validation	semantic header	2
extension.adjoin	validation	solving object admission	38
extension.adjoin	validation	statement object roles	1
extension.adjoin	validation	typed statement	0
extension.unit	exclusion.factor-preparation	exact coefficients	140
extension.unit	validation	exact coefficients	4186
extension.unit	validation	rational addition	4608
extension.unit	validation	rational comparison	5824
extension.unit	validation	rational constructor	2048
extension.unit	validation	rational negation	512
extension.unit	validation	scalar statement view	1504
extension.unit	validation	semantic header	2
extension.unit	validation	solving object admission	148
extension.unit	validation	statement object roles	2
extension.unit	validation	typed statement	0
real.sign	exclusion.factor-preparation	exact coefficients	10
real.sign	exclusion.scalar-nonzero	elementary solving	1
real.sign	exclusion.scalar-nonzero	exact coefficients	18
real.sign	validation	elementary solving	31
real.sign	validation	exact coefficients	986
real.sign	validation	rational addition	288
real.sign	validation	rational comparison	2784
real.sign	validation	rational constructor	256
real.sign	validation	rational multiplication	320
real.sign	validation	rational negation	64
real.sign	validation	real solving certificates	1301
real.sign	validation	scalar family admission	1
real.sign	validation	scalar statement view	2382
real.sign	validation	semantic header	7
real.sign	validation	solving object admission	40
real.sign	validation	statement object roles	3
real.sign	validation	typed statement	0
region.construct	validation	region theorem	1
region.construct	validation	scalar statement view	552
region.construct	validation	semantic header	2
region.construct	validation	statement object roles	1
region.construct	validation	typed statement	0
replay	application reconstruction	interning lookup	2159
replay	application reconstruction	scalar statement view	69588
replay	application reconstruction	structural key	2135
replay	application reconstruction	witness decoding	24433
replay	conclusion reconstruction	scalar statement view	1331
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	rational encoding	448
replay	judgment comparison	typed witness DAG	212615
replay	judgment comparison	typed witness record	4275
replay	overhead	proof transaction	256
replay	overhead	semantic header	3
replay	overhead	solving object admission	56
replay	overhead	statement object roles	1
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	8
replay	syntax admission	draft references	57
replay	syntax admission	draft validation	16649
replay	syntax admission	exact coefficients	179
replay	syntax admission	interning lookup	6879
replay	syntax admission	local family evaluation	9
replay	syntax admission	native case admission	10
replay	syntax admission	rational comparison	896
replay	syntax admission	rational constructor	2176
replay	syntax admission	rational division	192
replay	syntax admission	rational encoding	864
replay	syntax admission	rational multiplication	2560
replay	syntax admission	scalar family admission	3135
replay	syntax admission	semantic header	118
replay	syntax admission	solving object admission	14324
replay	syntax admission	structural key	6736
replay	syntax admission	transaction	75246
replay	syntax admission	typed object decoding	5
replay	syntax admission	typed syntax descriptor	34803
replay	syntax admission	witness decoding	20364
replay	wire admission	scalar family admission	1
replay	wire admission	semantic header	25
replay	wire admission	solving object admission	358
replay	wire admission	statement object roles	11
replay	wire admission	typed witness import	382553
replay	wire admission	witness decoding	91584
replay	witness encoding	rational encoding	288
replay	witness encoding	scalar statement view	14358
replay	witness encoding	typed witness DAG	265738
replay	witness encoding	typed witness record	4380
root.select	exclusion.component-nonzero	elementary solving	21
root.select	exclusion.component-nonzero	exact coefficients	108
root.select	existence	elementary solving	149
root.select	existence	exact coefficients	27703
root.select	existence	local family evaluation	76
root.select	existence	rational addition	576
root.select	existence	rational comparison	147456
root.select	existence	rational constructor	12032
root.select	existence	rational division	160
root.select	existence	rational multiplication	2240
root.select	existence	rational negation	128
root.select	validation	elementary solving	1
root.select	validation	exact coefficients	8
root.select	validation	scalar statement view	622
root.select	validation	semantic header	2
root.select	validation	solving object admission	38
root.select	validation	statement object roles	1
root.select	validation	typed statement	0
solve.product	exclusion.equation	elementary solving	23
solve.product	exclusion.equation	exact coefficients	1405
solve.product	exclusion.equation	local family evaluation	41
solve.product	exclusion.equation	rational comparison	4480
solve.product	exclusion.equation	rational constructor	128
solve.product	exclusion.equation	rational multiplication	320
solve.product	exclusion.equation	rational negation	64
solve.product	exclusion.factor-preparation	exact coefficients	616
solve.product	exclusion.factor-search	exact coefficients	520
solve.product	exclusion.factor-search	rational constructor	1536
solve.product	exclusion.factor-search	rational division	384
solve.product	exclusion.factor-search	rational multiplication	3840
solve.product	exclusion.factors	elementary solving	91
solve.product	exclusion.factors	exact coefficients	4390
solve.product	exclusion.factors	local family evaluation	300
solve.product	exclusion.factors	rational addition	1120
solve.product	exclusion.factors	rational comparison	14720
solve.product	exclusion.factors	rational constructor	640
solve.product	exclusion.factors	rational division	32
solve.product	exclusion.factors	rational multiplication	320
solve.product	exclusion.factors	rational negation	160
solve.product	exclusion.identity	elementary solving	105
solve.product	exclusion.identity	exact coefficients	3669
solve.product	exclusion.identity	local family evaluation	203
solve.product	exclusion.identity	rational addition	1440
solve.product	exclusion.identity	rational comparison	11808
solve.product	exclusion.identity	rational constructor	1280
solve.product	exclusion.identity	rational multiplication	960
solve.product	exclusion.identity	rational negation	192
solve.product	exclusion.scalar-nonzero	elementary solving	78
solve.product	exclusion.scalar-nonzero	exact coefficients	417
solve.product	exclusion.scalar-nonzero	rational comparison	6272
solve.product	exclusion.sign-leaves	elementary solving	53
solve.product	exclusion.sign-leaves	empty region DAG	7
solve.product	exclusion.sign-leaves	exact coefficients	253
solve.product	exclusion.sign-leaves	local family evaluation	7
solve.product	exclusion.sign-leaves	rational negation	32
solve.product	existence.membership	elementary solving	148
solve.product	existence.membership	exact coefficients	6421
solve.product	existence.membership	local family evaluation	248
solve.product	existence.membership	rational addition	1152
solve.product	existence.membership	rational comparison	18848
solve.product	existence.membership	rational constructor	1408
solve.product	existence.membership	rational multiplication	2240
solve.product	existence.membership	rational negation	256
solve.product	inventory	elementary solving	1
solve.product	inventory	exact coefficients	2
solve.product	validation	elementary solving	2
solve.product	validation	exact coefficients	13
solve.product	validation	scalar statement view	722
solve.product	validation	semantic header	3
solve.product	validation	solving object admission	56
solve.product	validation	statement object roles	1
solve.product	validation	typed statement	0

tests::family_solver::lc::refusals::lc_root_replay_retains_original_premises
coeff.identity	exclusion.factor-preparation	exact coefficients	794
coeff.identity	exclusion.factor-search	exact coefficients	1040
coeff.identity	exclusion.factor-search	rational constructor	3072
coeff.identity	exclusion.factor-search	rational division	768
coeff.identity	exclusion.factor-search	rational multiplication	7680
coeff.identity	validation	exact coefficients	9085
coeff.identity	validation	local family evaluation	11
coeff.identity	validation	rational addition	6624
coeff.identity	validation	rational comparison	9568
coeff.identity	validation	rational constructor	5120
coeff.identity	validation	rational division	256
coeff.identity	validation	rational multiplication	5440
coeff.identity	validation	rational negation	800
coeff.identity	validation	scalar statement view	682
coeff.identity	validation	semantic header	4
coeff.identity	validation	solving object admission	74
coeff.identity	validation	statement object roles	1
coeff.identity	validation	typed statement	0
ctx.weaken	validation	exact coefficients	4
ctx.weaken	validation	local family evaluation	1036
ctx.weaken	validation	scalar statement view	1106
ctx.weaken	validation	semantic header	4
ctx.weaken	validation	solving object admission	3388
ctx.weaken	validation	statement object roles	2
ctx.weaken	validation	typed statement	0
ctx.weaken	validation	typed syntax descriptor	1856
extension.adjoin	exclusion.factor-preparation	exact coefficients	20
extension.adjoin	validation	exact coefficients	240
extension.adjoin	validation	rational addition	288
extension.adjoin	validation	rational comparison	224
extension.adjoin	validation	rational constructor	128
extension.adjoin	validation	rational negation	32
extension.adjoin	validation	scalar statement view	622
extension.adjoin	validation	semantic header	2
extension.adjoin	validation	solving object admission	38
extension.adjoin	validation	statement object roles	1
extension.adjoin	validation	typed statement	0
extension.unit	exclusion.factor-preparation	exact coefficients	140
extension.unit	validation	exact coefficients	4186
extension.unit	validation	rational addition	4608
extension.unit	validation	rational comparison	5824
extension.unit	validation	rational constructor	2048
extension.unit	validation	rational negation	512
extension.unit	validation	scalar statement view	1504
extension.unit	validation	semantic header	2
extension.unit	validation	solving object admission	148
extension.unit	validation	statement object roles	2
extension.unit	validation	typed statement	0
real.sign	exclusion.factor-preparation	exact coefficients	10
real.sign	exclusion.scalar-nonzero	elementary solving	1
real.sign	exclusion.scalar-nonzero	exact coefficients	18
real.sign	validation	elementary solving	31
real.sign	validation	exact coefficients	986
real.sign	validation	rational addition	288
real.sign	validation	rational comparison	2784
real.sign	validation	rational constructor	256
real.sign	validation	rational multiplication	320
real.sign	validation	rational negation	64
real.sign	validation	real solving certificates	1292
real.sign	validation	scalar family admission	1
real.sign	validation	scalar statement view	2382
real.sign	validation	semantic header	7
real.sign	validation	solving object admission	40
real.sign	validation	statement object roles	3
real.sign	validation	typed statement	0
region.construct	validation	region theorem	1
region.construct	validation	scalar statement view	552
region.construct	validation	semantic header	2
region.construct	validation	statement object roles	1
region.construct	validation	typed statement	0
replay	application reconstruction	interning lookup	2159
replay	application reconstruction	scalar statement view	69588
replay	application reconstruction	structural key	2135
replay	application reconstruction	witness decoding	24433
replay	conclusion reconstruction	scalar statement view	1331
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	rational encoding	448
replay	judgment comparison	typed witness DAG	218318
replay	judgment comparison	typed witness record	4370
replay	overhead	proof transaction	256
replay	overhead	semantic header	3
replay	overhead	solving object admission	56
replay	overhead	statement object roles	1
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	8
replay	syntax admission	draft references	57
replay	syntax admission	draft validation	16649
replay	syntax admission	exact coefficients	179
replay	syntax admission	interning lookup	6879
replay	syntax admission	local family evaluation	9
replay	syntax admission	native case admission	10
replay	syntax admission	rational comparison	896
replay	syntax admission	rational constructor	2176
replay	syntax admission	rational division	192
replay	syntax admission	rational encoding	864
replay	syntax admission	rational multiplication	2560
replay	syntax admission	scalar family admission	3135
replay	syntax admission	semantic header	118
replay	syntax admission	solving object admission	14324
replay	syntax admission	structural key	6736
replay	syntax admission	transaction	75246
replay	syntax admission	typed object decoding	5
replay	syntax admission	typed syntax descriptor	34803
replay	syntax admission	witness decoding	20364
replay	wire admission	scalar family admission	1
replay	wire admission	semantic header	25
replay	wire admission	solving object admission	358
replay	wire admission	statement object roles	11
replay	wire admission	typed witness import	382553
replay	wire admission	witness decoding	91584
replay	witness encoding	rational encoding	288
replay	witness encoding	scalar statement view	14358
replay	witness encoding	typed witness DAG	265738
replay	witness encoding	typed witness record	4380
root.select	exclusion.component-nonzero	elementary solving	21
root.select	exclusion.component-nonzero	exact coefficients	108
root.select	existence	elementary solving	149
root.select	existence	exact coefficients	27703
root.select	existence	local family evaluation	76
root.select	existence	rational addition	576
root.select	existence	rational comparison	147456
root.select	existence	rational constructor	12032
root.select	existence	rational division	160
root.select	existence	rational multiplication	2240
root.select	existence	rational negation	128
root.select	validation	elementary solving	1
root.select	validation	exact coefficients	8
root.select	validation	scalar statement view	622
root.select	validation	semantic header	2
root.select	validation	solving object admission	38
root.select	validation	statement object roles	1
root.select	validation	typed statement	0
solve.product	exclusion.equation	elementary solving	23
solve.product	exclusion.equation	exact coefficients	1405
solve.product	exclusion.equation	local family evaluation	41
solve.product	exclusion.equation	rational comparison	4480
solve.product	exclusion.equation	rational constructor	128
solve.product	exclusion.equation	rational multiplication	320
solve.product	exclusion.equation	rational negation	64
solve.product	exclusion.factor-preparation	exact coefficients	616
solve.product	exclusion.factor-search	exact coefficients	520
solve.product	exclusion.factor-search	rational constructor	1536
solve.product	exclusion.factor-search	rational division	384
solve.product	exclusion.factor-search	rational multiplication	3840
solve.product	exclusion.factors	elementary solving	91
solve.product	exclusion.factors	exact coefficients	4390
solve.product	exclusion.factors	local family evaluation	300
solve.product	exclusion.factors	rational addition	1120
solve.product	exclusion.factors	rational comparison	14720
solve.product	exclusion.factors	rational constructor	640
solve.product	exclusion.factors	rational division	32
solve.product	exclusion.factors	rational multiplication	320
solve.product	exclusion.factors	rational negation	160
solve.product	exclusion.identity	elementary solving	105
solve.product	exclusion.identity	exact coefficients	3669
solve.product	exclusion.identity	local family evaluation	203
solve.product	exclusion.identity	rational addition	1440
solve.product	exclusion.identity	rational comparison	11808
solve.product	exclusion.identity	rational constructor	1280
solve.product	exclusion.identity	rational multiplication	960
solve.product	exclusion.identity	rational negation	192
solve.product	exclusion.scalar-nonzero	elementary solving	78
solve.product	exclusion.scalar-nonzero	exact coefficients	417
solve.product	exclusion.scalar-nonzero	rational comparison	6272
solve.product	exclusion.sign-leaves	elementary solving	53
solve.product	exclusion.sign-leaves	empty region DAG	7
solve.product	exclusion.sign-leaves	exact coefficients	253
solve.product	exclusion.sign-leaves	local family evaluation	7
solve.product	exclusion.sign-leaves	rational negation	32
solve.product	existence.membership	elementary solving	148
solve.product	existence.membership	exact coefficients	6421
solve.product	existence.membership	local family evaluation	248
solve.product	existence.membership	rational addition	1152
solve.product	existence.membership	rational comparison	18848
solve.product	existence.membership	rational constructor	1408
solve.product	existence.membership	rational multiplication	2240
solve.product	existence.membership	rational negation	256
solve.product	inventory	elementary solving	1
solve.product	inventory	exact coefficients	2
solve.product	validation	elementary solving	2
solve.product	validation	exact coefficients	13
solve.product	validation	scalar statement view	722
solve.product	validation	semantic header	3
solve.product	validation	solving object admission	56
solve.product	validation	statement object roles	1
solve.product	validation	typed statement	0

tests::family_solver::lc::refusals::lc_root_replay_retains_original_premises
coeff.identity	exclusion.factor-preparation	exact coefficients	794
coeff.identity	exclusion.factor-search	exact coefficients	1040
coeff.identity	exclusion.factor-search	rational constructor	3072
coeff.identity	exclusion.factor-search	rational division	768
coeff.identity	exclusion.factor-search	rational multiplication	7680
coeff.identity	validation	exact coefficients	9085
coeff.identity	validation	local family evaluation	11
coeff.identity	validation	rational addition	6624
coeff.identity	validation	rational comparison	9568
coeff.identity	validation	rational constructor	5120
coeff.identity	validation	rational division	256
coeff.identity	validation	rational multiplication	5440
coeff.identity	validation	rational negation	800
coeff.identity	validation	scalar statement view	682
coeff.identity	validation	semantic header	4
coeff.identity	validation	solving object admission	74
coeff.identity	validation	statement object roles	1
coeff.identity	validation	typed statement	0
ctx.weaken	validation	exact coefficients	4
ctx.weaken	validation	local family evaluation	1036
ctx.weaken	validation	scalar statement view	1106
ctx.weaken	validation	semantic header	4
ctx.weaken	validation	solving object admission	3388
ctx.weaken	validation	statement object roles	2
ctx.weaken	validation	typed statement	0
ctx.weaken	validation	typed syntax descriptor	1856
extension.adjoin	exclusion.factor-preparation	exact coefficients	20
extension.adjoin	validation	exact coefficients	240
extension.adjoin	validation	rational addition	288
extension.adjoin	validation	rational comparison	224
extension.adjoin	validation	rational constructor	128
extension.adjoin	validation	rational negation	32
extension.adjoin	validation	scalar statement view	622
extension.adjoin	validation	semantic header	2
extension.adjoin	validation	solving object admission	38
extension.adjoin	validation	statement object roles	1
extension.adjoin	validation	typed statement	0
extension.unit	exclusion.factor-preparation	exact coefficients	140
extension.unit	validation	exact coefficients	4186
extension.unit	validation	rational addition	4608
extension.unit	validation	rational comparison	5824
extension.unit	validation	rational constructor	2048
extension.unit	validation	rational negation	512
extension.unit	validation	scalar statement view	1504
extension.unit	validation	semantic header	2
extension.unit	validation	solving object admission	148
extension.unit	validation	statement object roles	2
extension.unit	validation	typed statement	0
real.sign	exclusion.factor-preparation	exact coefficients	10
real.sign	exclusion.scalar-nonzero	elementary solving	1
real.sign	exclusion.scalar-nonzero	exact coefficients	18
real.sign	validation	elementary solving	31
real.sign	validation	exact coefficients	986
real.sign	validation	rational addition	288
real.sign	validation	rational comparison	2784
real.sign	validation	rational constructor	256
real.sign	validation	rational multiplication	320
real.sign	validation	rational negation	64
real.sign	validation	real solving certificates	1296
real.sign	validation	scalar family admission	1
real.sign	validation	scalar statement view	2382
real.sign	validation	semantic header	7
real.sign	validation	solving object admission	40
real.sign	validation	statement object roles	3
real.sign	validation	typed statement	0
region.construct	validation	region theorem	1
region.construct	validation	scalar statement view	552
region.construct	validation	semantic header	2
region.construct	validation	statement object roles	1
region.construct	validation	typed statement	0
replay	application reconstruction	interning lookup	2159
replay	application reconstruction	scalar statement view	69588
replay	application reconstruction	structural key	2135
replay	application reconstruction	witness decoding	24433
replay	conclusion reconstruction	scalar statement view	1331
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	rational encoding	448
replay	judgment comparison	typed witness DAG	216713
replay	judgment comparison	typed witness record	4347
replay	overhead	proof transaction	256
replay	overhead	semantic header	3
replay	overhead	solving object admission	56
replay	overhead	statement object roles	1
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	8
replay	syntax admission	draft references	57
replay	syntax admission	draft validation	16649
replay	syntax admission	exact coefficients	179
replay	syntax admission	interning lookup	6879
replay	syntax admission	local family evaluation	9
replay	syntax admission	native case admission	10
replay	syntax admission	rational comparison	896
replay	syntax admission	rational constructor	2176
replay	syntax admission	rational division	192
replay	syntax admission	rational encoding	864
replay	syntax admission	rational multiplication	2560
replay	syntax admission	scalar family admission	3135
replay	syntax admission	semantic header	118
replay	syntax admission	solving object admission	14324
replay	syntax admission	structural key	6736
replay	syntax admission	transaction	75246
replay	syntax admission	typed object decoding	5
replay	syntax admission	typed syntax descriptor	34803
replay	syntax admission	witness decoding	20364
replay	wire admission	scalar family admission	1
replay	wire admission	semantic header	25
replay	wire admission	solving object admission	358
replay	wire admission	statement object roles	11
replay	wire admission	typed witness import	382553
replay	wire admission	witness decoding	91584
replay	witness encoding	rational encoding	288
replay	witness encoding	scalar statement view	14358
replay	witness encoding	typed witness DAG	265738
replay	witness encoding	typed witness record	4380
root.select	exclusion.component-nonzero	elementary solving	21
root.select	exclusion.component-nonzero	exact coefficients	108
root.select	existence	elementary solving	149
root.select	existence	exact coefficients	27703
root.select	existence	local family evaluation	76
root.select	existence	rational addition	576
root.select	existence	rational comparison	147456
root.select	existence	rational constructor	12032
root.select	existence	rational division	160
root.select	existence	rational multiplication	2240
root.select	existence	rational negation	128
root.select	validation	elementary solving	1
root.select	validation	exact coefficients	8
root.select	validation	scalar statement view	622
root.select	validation	semantic header	2
root.select	validation	solving object admission	38
root.select	validation	statement object roles	1
root.select	validation	typed statement	0
solve.product	exclusion.equation	elementary solving	23
solve.product	exclusion.equation	exact coefficients	1405
solve.product	exclusion.equation	local family evaluation	41
solve.product	exclusion.equation	rational comparison	4480
solve.product	exclusion.equation	rational constructor	128
solve.product	exclusion.equation	rational multiplication	320
solve.product	exclusion.equation	rational negation	64
solve.product	exclusion.factor-preparation	exact coefficients	616
solve.product	exclusion.factor-search	exact coefficients	520
solve.product	exclusion.factor-search	rational constructor	1536
solve.product	exclusion.factor-search	rational division	384
solve.product	exclusion.factor-search	rational multiplication	3840
solve.product	exclusion.factors	elementary solving	91
solve.product	exclusion.factors	exact coefficients	4390
solve.product	exclusion.factors	local family evaluation	300
solve.product	exclusion.factors	rational addition	1120
solve.product	exclusion.factors	rational comparison	14720
solve.product	exclusion.factors	rational constructor	640
solve.product	exclusion.factors	rational division	32
solve.product	exclusion.factors	rational multiplication	320
solve.product	exclusion.factors	rational negation	160
solve.product	exclusion.identity	elementary solving	105
solve.product	exclusion.identity	exact coefficients	3669
solve.product	exclusion.identity	local family evaluation	203
solve.product	exclusion.identity	rational addition	1440
solve.product	exclusion.identity	rational comparison	11808
solve.product	exclusion.identity	rational constructor	1280
solve.product	exclusion.identity	rational multiplication	960
solve.product	exclusion.identity	rational negation	192
solve.product	exclusion.scalar-nonzero	elementary solving	78
solve.product	exclusion.scalar-nonzero	exact coefficients	417
solve.product	exclusion.scalar-nonzero	rational comparison	6272
solve.product	exclusion.sign-leaves	elementary solving	53
solve.product	exclusion.sign-leaves	empty region DAG	7
solve.product	exclusion.sign-leaves	exact coefficients	253
solve.product	exclusion.sign-leaves	local family evaluation	7
solve.product	exclusion.sign-leaves	rational negation	32
solve.product	existence.membership	elementary solving	148
solve.product	existence.membership	exact coefficients	6421
solve.product	existence.membership	local family evaluation	248
solve.product	existence.membership	rational addition	1152
solve.product	existence.membership	rational comparison	18848
solve.product	existence.membership	rational constructor	1408
solve.product	existence.membership	rational multiplication	2240
solve.product	existence.membership	rational negation	256
solve.product	inventory	elementary solving	1
solve.product	inventory	exact coefficients	2
solve.product	validation	elementary solving	2
solve.product	validation	exact coefficients	13
solve.product	validation	scalar statement view	722
solve.product	validation	semantic header	3
solve.product	validation	solving object admission	56
solve.product	validation	statement object roles	1
solve.product	validation	typed statement	0

tests::family_solver::lc::refusals::lc_root_replay_retains_original_premises
coeff.identity	exclusion.factor-preparation	exact coefficients	794
coeff.identity	exclusion.factor-search	exact coefficients	1040
coeff.identity	exclusion.factor-search	rational constructor	3072
coeff.identity	exclusion.factor-search	rational division	768
coeff.identity	exclusion.factor-search	rational multiplication	7680
coeff.identity	validation	exact coefficients	9085
coeff.identity	validation	local family evaluation	11
coeff.identity	validation	rational addition	6624
coeff.identity	validation	rational comparison	9568
coeff.identity	validation	rational constructor	5120
coeff.identity	validation	rational division	256
coeff.identity	validation	rational multiplication	5440
coeff.identity	validation	rational negation	800
coeff.identity	validation	scalar statement view	682
coeff.identity	validation	semantic header	4
coeff.identity	validation	solving object admission	74
coeff.identity	validation	statement object roles	1
coeff.identity	validation	typed statement	0
ctx.weaken	validation	exact coefficients	4
ctx.weaken	validation	local family evaluation	1036
ctx.weaken	validation	scalar statement view	1106
ctx.weaken	validation	semantic header	4
ctx.weaken	validation	solving object admission	3388
ctx.weaken	validation	statement object roles	2
ctx.weaken	validation	typed statement	0
ctx.weaken	validation	typed syntax descriptor	1856
extension.adjoin	exclusion.factor-preparation	exact coefficients	20
extension.adjoin	validation	exact coefficients	240
extension.adjoin	validation	rational addition	288
extension.adjoin	validation	rational comparison	224
extension.adjoin	validation	rational constructor	128
extension.adjoin	validation	rational negation	32
extension.adjoin	validation	scalar statement view	622
extension.adjoin	validation	semantic header	2
extension.adjoin	validation	solving object admission	38
extension.adjoin	validation	statement object roles	1
extension.adjoin	validation	typed statement	0
extension.unit	exclusion.factor-preparation	exact coefficients	140
extension.unit	validation	exact coefficients	4186
extension.unit	validation	rational addition	4608
extension.unit	validation	rational comparison	5824
extension.unit	validation	rational constructor	2048
extension.unit	validation	rational negation	512
extension.unit	validation	scalar statement view	1504
extension.unit	validation	semantic header	2
extension.unit	validation	solving object admission	148
extension.unit	validation	statement object roles	2
extension.unit	validation	typed statement	0
real.sign	exclusion.factor-preparation	exact coefficients	10
real.sign	exclusion.scalar-nonzero	elementary solving	1
real.sign	exclusion.scalar-nonzero	exact coefficients	18
real.sign	validation	elementary solving	31
real.sign	validation	exact coefficients	986
real.sign	validation	rational addition	288
real.sign	validation	rational comparison	2784
real.sign	validation	rational constructor	256
real.sign	validation	rational multiplication	320
real.sign	validation	rational negation	64
real.sign	validation	real solving certificates	1317
real.sign	validation	scalar family admission	1
real.sign	validation	scalar statement view	2382
real.sign	validation	semantic header	7
real.sign	validation	solving object admission	40
real.sign	validation	statement object roles	3
real.sign	validation	typed statement	0
region.construct	validation	region theorem	1
region.construct	validation	scalar statement view	552
region.construct	validation	semantic header	2
region.construct	validation	statement object roles	1
region.construct	validation	typed statement	0
replay	application reconstruction	interning lookup	2159
replay	application reconstruction	scalar statement view	69588
replay	application reconstruction	structural key	2135
replay	application reconstruction	witness decoding	24433
replay	conclusion reconstruction	scalar statement view	1331
replay	conclusion reconstruction	witness decoding	17
replay	judgment comparison	rational encoding	448
replay	judgment comparison	typed witness DAG	209022
replay	judgment comparison	typed witness record	4228
replay	overhead	proof transaction	256
replay	overhead	semantic header	3
replay	overhead	solving object admission	56
replay	overhead	statement object roles	1
replay	overhead.judgment-comparison	typed proof admission	88
replay	syntax admission	context hypotheses	8
replay	syntax admission	draft references	57
replay	syntax admission	draft validation	16649
replay	syntax admission	exact coefficients	179
replay	syntax admission	interning lookup	6879
replay	syntax admission	local family evaluation	9
replay	syntax admission	native case admission	10
replay	syntax admission	rational comparison	896
replay	syntax admission	rational constructor	2176
replay	syntax admission	rational division	192
replay	syntax admission	rational encoding	864
replay	syntax admission	rational multiplication	2560
replay	syntax admission	scalar family admission	3135
replay	syntax admission	semantic header	118
replay	syntax admission	solving object admission	14324
replay	syntax admission	structural key	6736
replay	syntax admission	transaction	75240
replay	syntax admission	typed object decoding	5
replay	syntax admission	typed syntax descriptor	34803
replay	syntax admission	witness decoding	20364
replay	wire admission	scalar family admission	1
replay	wire admission	semantic header	25
replay	wire admission	solving object admission	358
replay	wire admission	statement object roles	11
replay	wire admission	typed witness import	382553
replay	wire admission	witness decoding	91584
replay	witness encoding	rational encoding	288
replay	witness encoding	scalar statement view	14358
replay	witness encoding	typed witness DAG	265738
replay	witness encoding	typed witness record	4380
root.select	exclusion.component-nonzero	elementary solving	21
root.select	exclusion.component-nonzero	exact coefficients	108
root.select	existence	elementary solving	149
root.select	existence	exact coefficients	27703
root.select	existence	local family evaluation	76
root.select	existence	rational addition	576
root.select	existence	rational comparison	147456
root.select	existence	rational constructor	12032
root.select	existence	rational division	160
root.select	existence	rational multiplication	2240
root.select	existence	rational negation	128
root.select	validation	elementary solving	1
root.select	validation	exact coefficients	8
root.select	validation	scalar statement view	622
root.select	validation	semantic header	2
root.select	validation	solving object admission	38
root.select	validation	statement object roles	1
root.select	validation	typed statement	0
solve.product	exclusion.equation	elementary solving	23
solve.product	exclusion.equation	exact coefficients	1405
solve.product	exclusion.equation	local family evaluation	41
solve.product	exclusion.equation	rational comparison	4480
solve.product	exclusion.equation	rational constructor	128
solve.product	exclusion.equation	rational multiplication	320
solve.product	exclusion.equation	rational negation	64
solve.product	exclusion.factor-preparation	exact coefficients	616
solve.product	exclusion.factor-search	exact coefficients	520
solve.product	exclusion.factor-search	rational constructor	1536
solve.product	exclusion.factor-search	rational division	384
solve.product	exclusion.factor-search	rational multiplication	3840
solve.product	exclusion.factors	elementary solving	91
solve.product	exclusion.factors	exact coefficients	4390
solve.product	exclusion.factors	local family evaluation	300
solve.product	exclusion.factors	rational addition	1120
solve.product	exclusion.factors	rational comparison	14720
solve.product	exclusion.factors	rational constructor	640
solve.product	exclusion.factors	rational division	32
solve.product	exclusion.factors	rational multiplication	320
solve.product	exclusion.factors	rational negation	160
solve.product	exclusion.identity	elementary solving	105
solve.product	exclusion.identity	exact coefficients	3669
solve.product	exclusion.identity	local family evaluation	203
solve.product	exclusion.identity	rational addition	1440
solve.product	exclusion.identity	rational comparison	11808
solve.product	exclusion.identity	rational constructor	1280
solve.product	exclusion.identity	rational multiplication	960
solve.product	exclusion.identity	rational negation	192
solve.product	exclusion.scalar-nonzero	elementary solving	78
solve.product	exclusion.scalar-nonzero	exact coefficients	417
solve.product	exclusion.scalar-nonzero	rational comparison	6272
solve.product	exclusion.sign-leaves	elementary solving	53
solve.product	exclusion.sign-leaves	empty region DAG	7
solve.product	exclusion.sign-leaves	exact coefficients	253
solve.product	exclusion.sign-leaves	local family evaluation	7
solve.product	exclusion.sign-leaves	rational negation	32
solve.product	existence.membership	elementary solving	148
solve.product	existence.membership	exact coefficients	6421
solve.product	existence.membership	local family evaluation	248
solve.product	existence.membership	rational addition	1152
solve.product	existence.membership	rational comparison	18848
solve.product	existence.membership	rational constructor	1408
solve.product	existence.membership	rational multiplication	2240
solve.product	existence.membership	rational negation	256
solve.product	inventory	elementary solving	1
solve.product	inventory	exact coefficients	2
solve.product	validation	elementary solving	2
solve.product	validation	exact coefficients	13
solve.product	validation	scalar statement view	722
solve.product	validation	semantic header	3
solve.product	validation	solving object admission	56
solve.product	validation	statement object roles	1
solve.product	validation	typed statement	0";
