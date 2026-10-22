// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`separates_counts_ratios_and_metadata`] | docgen | Accept quantities, counts and metadata separately and refuse role erasure. |
//! | [`checks_affine_origins_and_companion_measures`] | docgen | Preserve affine conversions and independently scaled uncertainty differences. |
//! | [`refuses_malformed_declarations_and_hidden_invalid_operands`] | docgen | Refuse absent or extra fields, invalid exact literals, scales and unknown symbols. |
//! | [`checks_rational_policy_and_forward_reference_cycles`] | docgen | Admit rational exponent rules only under their explicit policy and reject cycles. |
//! | [`checks_every_variant_replacement_contract`] | docgen | Admit variant coordinates while refusing category, count and nonzero violations. |
//! | [`rechecks_evidence_and_isolates_unsupported_results`] | docgen | Preserve deliberately unsupported formulas and refuse manufactured downstream evidence. |
//! | [`checks_original_equation_sides`] | docgen | Check equation sides and refuse a dimensionally invalid right side with its path. |
//! | [`checks_variance_and_relative_reference_evidence`] | docgen | Derive squared and relative uncertainty contracts without rebinding their targets. |
//! | [`checks_covariance_targets_and_scale_products`] | docgen | Covariance uses both target dimensions and their scale product, with signed magnitudes. |
//! | [`refuses_kind_substitution_and_fractional_count_overrides`] | docgen | Equal dimensions never authorize another kind, and count overrides retain their domain. |
//! | [`keeps_independent_uncertainty_components_distinct`] | docgen | Measurement and computational components retain distinct modes, sources and assurances. |
//! | [`irrational_descriptor_schema_export_refuses`] | docgen | The rational schema names the missing enclosure instead of printing an approximate measure. |
//! | [`irrational_output_schema_export_refuses`] | docgen | Output conversion evidence obeys the same version-two rational export boundary. |

//! Independent version-two fixtures; no migrated repository data is required.

use serde_json::{Value, json};

use super::{revalidate, validate};
use crate::error::Error;

/// Explicit coherent descriptor with a caller-selected kind and dimension vector.
fn descriptor(kind: &str, dimensions: [&str; 7]) -> Value {
    json!({
        "dimensions": dimensions, "kind": kind,
        "measure": {"display": "test measure", "scale": "1", "offset": "0"},
        "affine_role": "difference",
    })
}

/// Small independently authored source entry with a dimensional identity observable.
fn fixture() -> Value {
    let length = ["1", "0", "0", "0", "0", "0", "0"];
    json!({
        "problem": {
            "slug": "sample", "title": "Sample", "register": "prob:exemplars:sample",
            "kind": "problem", "model": "An independent length", "instance": "Two metres",
        },
        "exponent_policy": "rational",
        "kinds": [
            {"name": "ratio", "type": "ratio", "dimensions": ["0", "0", "0", "0", "0", "0", "0"]},
            {"name": "length", "type": "named", "dimensions": length},
        ],
        "kind_rules": [],
        "parameters": [{
            "name": "x", "description": "Length", "value": "2", "role": "quantity",
            "descriptor": descriptor("length", length), "nonzero": false,
            "uncertainty": {"kind": "none"},
        }],
        "observables": [{
            "name": "result", "description": "Identity", "descriptor": descriptor("length", length),
            "expression": {"kind": "algebra", "text": "x"}, "closed_form": "x",
            "value": "2", "value_kind": "exact", "derivation": "Identity", "second_route": "Direct",
        }],
        "errors": {"bounded": [], "estimated": [], "excluded": []},
        "stress": {"claims": ["req:vision:unit-data-migration"], "bites": []},
    })
}

/// Run the public source admission boundary with a stable located identity.
fn check(value: &Value) -> Result<Value, Error> {
    validate(
        value,
        "data/problems/sample/problem.toml",
        "sample",
        "problem",
        "prob:exemplars:sample",
    )
}

/// Add a synthetic metadata selector, independently of any production input.
fn with_metadata(mut value: Value) -> Value {
    value["parameters"]
        .as_array_mut()
        .expect("fixture parameters")
        .push(json!({
            "name": "selector", "description": "Topology", "value": "0", "role": "metadata",
            "metadata": {"kind": "category", "values": ["0", "1"]}, "uncertainty": {"kind": "none"},
        }));
    value
}

/// Install an explicitly typed standard uncertainty and make it the result.
fn with_standard(mut value: Value) -> Value {
    let dimensions = value["parameters"][0]["descriptor"]["dimensions"].clone();
    let value_kind = value["parameters"][0]["descriptor"]["kind"].clone();
    value["kinds"]
        .as_array_mut()
        .expect("fixture kinds")
        .push(json!({
            "name": "standard_x", "type": "uncertainty", "dimensions": dimensions,
            "mode": "standard", "value_kind": value_kind,
        }));
    let mut own = value["parameters"][0]["descriptor"].clone();
    own["kind"] = json!("standard_x");
    own["affine_role"] = json!("difference");
    own["measure"]["offset"] = json!("0");
    value["parameters"][0]["uncertainty"] = json!({"kind": "components", "components": [{
        "name": "sigma_x", "class": "measurement", "mode": "standard", "target": "x",
        "descriptor": own, "source": "instrument", "assurance": "Standard uncertainty, not a deterministic radius",
        "magnitude": {"sigma": "1/2"},
    }]});
    value["observables"][0]["descriptor"] = own;
    value["observables"][0]["expression"]["text"] = json!("sigma_x");
    value["observables"][0]["companion"] = json!({
        "class": "measurement", "mode": "standard", "target": "x",
        "source": "instrument", "assurance": "Identity of the declared standard uncertainty",
    });
    value
}

/// Accept quantities, counts and metadata separately and refuse role erasure.
/// ´claim:docgen:separates-counts-ratios-and-metadata´
/// ´test:unit:separates-counts-ratios-and-metadata´
#[test]
fn separates_counts_ratios_and_metadata() -> Result<(), Error> {
    check(&with_metadata(fixture()))?;
    let mut ratio = fixture();
    for group in ["parameters", "observables"] {
        ratio[group][0]["descriptor"] = descriptor("ratio", ["0", "0", "0", "0", "0", "0", "0"]);
        ratio[group][0]["value"] = json!("1/2");
    }
    check(&ratio)?;
    let mut metadata_operand = with_metadata(fixture());
    metadata_operand["observables"][0]["expression"]["text"] = json!("selector*x");
    assert!(check(&metadata_operand).is_err());
    let mut count = fixture();
    count["kinds"][1] = json!({
        "name": "length", "type": "count", "thing": "particles",
        "dimensions": ["0", "0", "0", "0", "0", "0", "0"], "domain": {"minimum": 0},
    });
    for group in ["parameters", "observables"] {
        count[group][0]["descriptor"]["dimensions"] = json!(["0", "0", "0", "0", "0", "0", "0"]);
    }
    count["parameters"][0]["role"] = json!("count");
    check(&count)?;
    check(&with_standard(count.clone()))?;
    let mut fractional = count.clone();
    fractional["parameters"][0]["value"] = json!("1/2");
    assert!(check(&fractional).is_err());
    let mut ratio_role = count.clone();
    ratio_role["parameters"][0]["role"] = json!("quantity");
    assert!(check(&ratio_role).is_err());
    let mut count_ratio = count;
    count_ratio["observables"][0]["expression"]["text"] = json!("x+1");
    assert!(check(&count_ratio).is_err());
    Ok(())
}

/// Preserve affine conversions and independently scaled uncertainty differences.
/// ´claim:docgen:checks-affine-origins-and-companion-measures´
/// ´test:unit:checks-affine-origins-and-companion-measures´
#[test]
fn checks_affine_origins_and_companion_measures() -> Result<(), Error> {
    let mut point = fixture();
    for group in ["parameters", "observables"] {
        point[group][0]["descriptor"]["affine_role"] = json!("point");
    }
    point["parameters"][0]["descriptor"]["measure"]["offset"] = json!("273.15");
    let checked = check(&point)?;
    assert_eq!(
        checked["validation"]["records"][1]["output_conversion"]["shift"],
        "0"
    );
    check(&with_standard(point.clone()))?;
    point["observables"][0]["expression"]["text"] = json!("x+x");
    assert!(check(&point).is_err());
    let mut difference = fixture();
    difference["parameters"][0]["descriptor"]["measure"]["offset"] = json!("273.15");
    assert!(check(&difference).is_err());
    let mut standard = with_standard(fixture());
    check(&standard)?;
    standard["parameters"][0]["uncertainty"]["components"][0]["descriptor"]["measure"]["scale"] =
        json!("2");
    assert!(check(&standard).is_err());
    Ok(())
}

/// Refuse absent or extra fields, invalid exact literals, scales and unknown symbols.
/// ´claim:docgen:refuses-malformed-declarations-and-hidden-invalid-operands´
/// ´test:unit:refuses-malformed-declarations-and-hidden-invalid-operands´
#[test]
fn refuses_malformed_declarations_and_hidden_invalid_operands() -> Result<(), Error> {
    check(&fixture())?;
    for field in ["scale", "offset"] {
        let mut missing = fixture();
        missing["parameters"][0]["descriptor"]["measure"]
            .as_object_mut()
            .expect("measure")
            .remove(field);
        assert!(check(&missing).is_err());
    }
    for field in ["uncertainty", "nonzero"] {
        let mut missing = fixture();
        missing["parameters"][0]
            .as_object_mut()
            .expect("parameter")
            .remove(field);
        assert!(check(&missing).is_err());
    }
    for expression in ["absent", "0*(x+1)", "(x+1)-(x+1)", "(x+1)^0", "x^x", "x+1"] {
        let mut bad = fixture();
        bad["observables"][0]["expression"]["text"] = json!(expression);
        let error = check(&bad)
            .expect_err("invalid original operation")
            .to_string();
        assert!(error.contains("data/problems/sample/problem.toml"));
    }
    for value in ["1/0", "1+2", "9223372036854775808", "1e100", " 2"] {
        let mut bad = fixture();
        bad["parameters"][0]["value"] = json!(value);
        assert!(check(&bad).is_err());
    }
    for scale in ["0", "-1"] {
        let mut bad = fixture();
        bad["parameters"][0]["descriptor"]["measure"]["scale"] = json!(scale);
        assert!(check(&bad).is_err());
    }
    let mut extra = fixture();
    extra["parameters"][0]["ignored"] = json!(true);
    assert!(check(&extra).is_err());
    Ok(())
}

/// Admit rational exponent rules only under their explicit policy and reject cycles.
/// ´claim:docgen:checks-rational-policy-and-forward-reference-cycles´
/// ´test:unit:checks-rational-policy-and-forward-reference-cycles´
#[test]
fn checks_rational_policy_and_forward_reference_cycles() -> Result<(), Error> {
    let mut root = fixture();
    root["kinds"].as_array_mut().expect("kinds").push(json!({
        "name": "root_length", "type": "named", "dimensions": ["1/2", "0", "0", "0", "0", "0", "0"],
    }));
    root["kind_rules"] = json!([{
        "operation": "power",
        "base": "length",
        "exponent": "1/2",
        "result": "root_length",
        "reason": "Declared root kind"
    }]);
    root["observables"][0]["descriptor"] =
        descriptor("root_length", ["1/2", "0", "0", "0", "0", "0", "0"]);
    root["observables"][0]["expression"]["text"] = json!("sqrt(x)");
    check(&root)?;
    root["exponent_policy"] = json!("integral");
    assert!(check(&root).is_err());
    let mut forward = fixture();
    forward["symbols"] = json!([{
        "kind": "derived", "name": "helper", "description": "Forward helper",
        "descriptor": forward["parameters"][0]["descriptor"],
        "source": "Synthetic identity", "assurance": "Descriptor only",
        "expression": {"kind": "algebra", "text": "x"},
    }]);
    forward["observables"][0]["expression"]["text"] = json!("helper");
    check(&forward)?;
    forward["symbols"][0]["expression"]["text"] = json!("result");
    assert!(check(&forward).is_err());
    Ok(())
}

/// Admit variant coordinates while refusing category, count and nonzero violations.
/// ´claim:docgen:checks-every-variant-replacement-contract´
/// ´test:unit:checks-every-variant-replacement-contract´
#[test]
fn checks_every_variant_replacement_contract() -> Result<(), Error> {
    let mut value = with_metadata(fixture());
    value["variants"] = json!([{
        "name": "alternate",
        "description": "Another value",
        "expected": "Documentary only",
        "overrides": {"selector": "1", "x": "3"}
    }]);
    check(&value)?;
    value["variants"][0]["overrides"]["selector"] = json!("2");
    assert!(check(&value).is_err());
    value["variants"][0]["overrides"]["selector"] = json!("1");
    value["parameters"][0]["nonzero"] = json!(true);
    value["variants"][0]["overrides"]["x"] = json!("0");
    assert!(check(&value).is_err());
    value["variants"][0]["overrides"]["x"] = json!("1/0");
    assert!(check(&value).is_err());
    value["parameters"][1]["metadata"] = json!({"kind": "integer_range", "minimum": 1});
    value["parameters"][1]["value"] = json!("1");
    value["variants"][0]["overrides"]["x"] = json!("3");
    check(&value)?;
    value["variants"][0]["overrides"]["selector"] = json!("1/2");
    assert!(check(&value).is_err());
    Ok(())
}

/// Preserve deliberately unsupported formulas and refuse manufactured downstream evidence.
/// ´claim:docgen:rechecks-evidence-and-isolates-unsupported-results´
/// ´test:unit:rechecks-evidence-and-isolates-unsupported-results´
#[test]
fn rechecks_evidence_and_isolates_unsupported_results() -> Result<(), Error> {
    let mut source = fixture();
    source["observables"][0]["expression"] = json!({
        "kind": "unsupported", "text": "sum(x_i)", "code": "indexed-sum",
        "reason": "Requires indexed declarations and a binder", "dependencies": ["x"], "scope": ["expression"],
    });
    let mut dataset = check(&source)?;
    assert_eq!(dataset["validation"]["unsupported_observables"], 1);
    revalidate(&dataset, "problems.json:sample", "sample", "problem")?;
    dataset["validation"]["unsupported_observables"] = json!(0);
    assert!(revalidate(&dataset, "problems.json:sample", "sample", "problem").is_err());
    source["symbols"] = json!([{
        "kind": "derived", "name": "consumer", "description": "Forbidden consumer",
        "descriptor": source["parameters"][0]["descriptor"], "source": "Synthetic", "assurance": "None",
        "expression": {"kind": "algebra", "text": "result"},
    }]);
    assert!(check(&source).is_err());
    source["symbols"] = json!([]);
    source["observables"][0]["expression"]["dependencies"] = json!(["result"]);
    assert!(check(&source).is_err());
    Ok(())
}

/// Check equation sides and refuse a dimensionally invalid right side with its path.
/// ´claim:docgen:checks-original-equation-sides´
/// ´test:unit:checks-original-equation-sides´
#[test]
fn checks_original_equation_sides() -> Result<(), Error> {
    let mut source = fixture();
    source["equations"] = json!([{
        "kind": "algebra", "name": "identity", "source": "Synthetic", "assurance": "Units only",
        "left": {"kind": "algebra", "text": "x"}, "right": {"kind": "algebra", "text": "result"},
    }]);
    check(&source)?;
    source["equations"][0]["right"]["text"] = json!("1");
    let error = check(&source).expect_err("dimension mismatch").to_string();
    assert!(error.contains("equations"));
    assert!(error.contains('1'));
    Ok(())
}

/// Derive squared and relative uncertainty contracts without rebinding their targets.
/// ´claim:docgen:checks-variance-and-relative-reference-evidence´
/// ´test:unit:checks-variance-and-relative-reference-evidence´
#[test]
fn checks_variance_and_relative_reference_evidence() -> Result<(), Error> {
    let mut source = with_standard(fixture());
    source["parameters"][0]["descriptor"]["measure"]["scale"] = json!("2");
    source["parameters"][0]["uncertainty"]["components"][0]["descriptor"]["measure"]["scale"] =
        json!("2");
    source["kinds"].as_array_mut().expect("kinds").push(json!({
        "name": "variance_x", "type": "uncertainty", "mode": "variance", "value_kind": "length",
        "dimensions": ["2", "0", "0", "0", "0", "0", "0"],
    }));
    source["observables"][0]["descriptor"] =
        descriptor("variance_x", ["2", "0", "0", "0", "0", "0", "0"]);
    source["observables"][0]["descriptor"]["measure"]["scale"] = json!("4");
    source["observables"][0]["expression"]["text"] = json!("sigma_x^2");
    source["observables"][0]["companion"]["mode"] = json!("variance");
    check(&source)?;
    source["observables"][0]["descriptor"]["measure"]["scale"] = json!("2");
    assert!(check(&source).is_err());
    let mut relative = with_standard(fixture());
    relative["parameters"][0]["nonzero"] = json!(true);
    relative["kinds"].as_array_mut().expect("kinds").push(json!({
        "name": "relative_x", "type": "uncertainty", "mode": "relative-standard", "value_kind": "length",
        "dimensions": ["0", "0", "0", "0", "0", "0", "0"],
    }));
    relative["observables"][0]["descriptor"] =
        descriptor("relative_x", ["0", "0", "0", "0", "0", "0", "0"]);
    relative["observables"][0]["expression"]["text"] = json!("sigma_x/x");
    relative["observables"][0]["companion"]["mode"] = json!("relative-standard");
    relative["observables"][0]["companion"]["reference"] = json!("x");
    check(&relative)?;
    relative["parameters"][0]["nonzero"] = json!(false);
    assert!(check(&relative).is_err());
    relative["observables"][0]["expression"] = json!({
        "kind": "unsupported", "text": "sigma_x/x", "code": "missing-nonzero-reference",
        "reason": "Requires an explicit nonzero difference reference", "dependencies": [
            "sigma_x",
            "x"
        ],
        "scope": ["expression", "relationship", "descriptor"],
    });
    let checked = check(&relative)?;
    assert_eq!(
        checked["validation"]["records"][2]["descriptor_status"],
        "unsupported"
    );
    assert!(
        checked["validation"]["records"][2]
            .get("checked_descriptor")
            .is_none()
    );
    Ok(())
}

/// Covariance uses both target dimensions and their scale product, with signed magnitudes.
/// ´claim:docgen:checks-covariance-targets-and-scale-products´
/// ´test:unit:checks-covariance-targets-and-scale-products´
#[test]
fn checks_covariance_targets_and_scale_products() -> Result<(), Error> {
    let mut value = fixture();
    value["parameters"][0]["descriptor"]["measure"]["scale"] = json!("2");
    value["kinds"].as_array_mut().expect("kinds").extend([
        json!({"name": "time", "type": "named", "dimensions": ["0", "0", "1", "0", "0", "0", "0"]}),
        json!({
            "name": "covariance_xy",
            "type": "uncertainty",
            "dimensions": ["1", "0", "1", "0", "0", "0", "0"],
            "mode": "covariance",
            "value_kind": "length",
            "other_kind": "time"
        }),
    ]);
    let mut time = descriptor("time", ["0", "0", "1", "0", "0", "0", "0"]);
    time["measure"]["scale"] = json!("3");
    value["parameters"]
        .as_array_mut()
        .expect("parameters")
        .push(json!({
            "name": "t", "description": "Time", "role": "quantity", "value": "4",
            "descriptor": time, "nonzero": false, "uncertainty": {"kind": "none"},
        }));
    let mut covariance = descriptor("covariance_xy", ["1", "0", "1", "0", "0", "0", "0"]);
    covariance["measure"]["scale"] = json!("6");
    value["parameters"][0]["uncertainty"] = json!({"kind": "components", "components": [{
        "name": "cov_xt", "class": "measurement", "mode": "covariance", "target": "x", "other": "t",
        "descriptor": covariance, "magnitude": {"covariance": "-1/2"},
        "source": "paired observation", "assurance": "Declared covariance, not a bound",
    }]});
    value["observables"][0]["descriptor"] = covariance;
    value["observables"][0]["expression"]["text"] = json!("cov_xt");
    value["observables"][0]["companion"] = json!({
        "class": "measurement", "mode": "covariance", "target": "x", "other": "t",
        "source": "paired observation", "assurance": "Identity",
    });
    check(&value)?;
    value["observables"][0]["descriptor"]["measure"]["scale"] = json!("4");
    assert!(check(&value).is_err());
    Ok(())
}

/// Equal dimensions never authorize another kind, and count overrides retain their domain.
/// ´claim:docgen:refuses-kind-substitution-and-fractional-count-overrides´
/// ´test:unit:refuses-kind-substitution-and-fractional-count-overrides´
#[test]
fn refuses_kind_substitution_and_fractional_count_overrides() -> Result<(), Error> {
    let mut source = fixture();
    source["kinds"].as_array_mut().expect("kinds").push(json!({
        "name": "other_length", "type": "named",
        "dimensions": ["1", "0", "0", "0", "0", "0", "0"],
    }));
    check(&source)?;
    source["observables"][0]["descriptor"]["kind"] = json!("other_length");
    assert!(check(&source).is_err());
    let mut count = fixture();
    count["kinds"][1] = json!({
        "name": "length", "type": "count", "thing": "particles",
        "dimensions": ["0", "0", "0", "0", "0", "0", "0"], "domain": {"minimum": 0, "maximum": 10},
    });
    for group in ["parameters", "observables"] {
        count[group][0]["descriptor"]["dimensions"] = json!(["0", "0", "0", "0", "0", "0", "0"]);
    }
    count["parameters"][0]["role"] = json!("count");
    count["variants"] = json!([{
        "name": "replacement", "description": "Count coordinate", "expected": "Admission only",
        "overrides": {"x": "3"},
    }]);
    check(&count)?;
    for replacement in ["1/2", "-1", "11"] {
        count["variants"][0]["overrides"]["x"] = json!(replacement);
        assert!(check(&count).is_err());
    }
    Ok(())
}

/// Measurement and computational components retain distinct modes, sources and assurances.
/// ´claim:docgen:keeps-independent-uncertainty-components-distinct´
/// ´test:unit:keeps-independent-uncertainty-components-distinct´
#[test]
fn keeps_independent_uncertainty_components_distinct() -> Result<(), Error> {
    let mut source = with_standard(fixture());
    source["kinds"].as_array_mut().expect("kinds").push(json!({
        "name": "radius_x", "type": "uncertainty", "mode": "radius", "value_kind": "length",
        "dimensions": ["1", "0", "0", "0", "0", "0", "0"],
    }));
    source["parameters"][0]["uncertainty"]["components"]
        .as_array_mut()
        .expect("components")
        .push(json!({
            "name": "radius_x", "class": "computational", "mode": "radius", "target": "x",
            "descriptor": descriptor("radius_x", ["1", "0", "0", "0", "0", "0", "0"]),
            "source": "Synthetic rounding model", "assurance": "Assumed deterministic bound",
            "magnitude": {"half_width": "1/3"},
        }));
    let result = check(&source)?;
    assert_eq!(
        result["validation"]["records"][1]["companion_evidence"]["companion"]["mode"],
        "standard"
    );
    assert_eq!(
        result["validation"]["records"][2]["companion_evidence"]["companion"]["mode"],
        "radius"
    );
    source["parameters"][0]["uncertainty"]["components"][1]["magnitude"]["half_width"] =
        json!("-1/3");
    assert!(check(&source).is_err());
    Ok(())
}

/// The rational schema names the missing enclosure instead of printing an approximate measure.
/// ´claim:docgen:irrational-descriptor-schema-export-refuses´
/// ´test:unit:irrational-descriptor-schema-export-refuses´
#[test]
fn irrational_descriptor_schema_export_refuses() -> Result<(), Box<dyn std::error::Error>> {
    use nrz_bindings::{Descriptor, Kind, Measure, Role, Scale, UnitError};
    let quantity = Descriptor::new(
        Kind::Ratio,
        Measure::new("pi", Scale::pi()?, Measure::coherent().offset().clone())?,
        Role::Difference,
    )?;
    let kinds = std::collections::BTreeMap::from([("ratio".into(), Kind::Ratio)]);
    let value = Value::Null;
    let record = super::Record::new(&value, "irrational measure");
    let result = super::kinds::evidence(&quantity, &kinds, &record);
    assert!(matches!(result, Err(Error::Payload { detail, .. })
        if detail == UnitError::UnsupportedExactConstantExport.to_string()));
    Ok(())
}

/// Output conversion evidence obeys the same version-two rational export boundary.
/// ´claim:docgen:irrational-output-schema-export-refuses´
/// ´test:unit:irrational-output-schema-export-refuses´
#[test]
fn irrational_output_schema_export_refuses() -> Result<(), Box<dyn std::error::Error>> {
    use nrz_bindings::{
        Context, Descriptor, ExponentPolicy, Kind, Measure, Registry, Role, Scale, UnitError,
    };
    let builder = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        nrz_bindings::Limits::default(),
        &[],
    )?;
    let target = Descriptor::new(
        Kind::Ratio,
        Measure::new("pi", Scale::pi()?, Measure::coherent().offset().clone())?,
        Role::Difference,
    )?;
    let original = builder.parse("literal", "1")?;
    let checked = builder.check_against(&original, &target)?;
    let value = Value::Null;
    let source = super::Record::new(&value, "irrational output");
    let mut output = json!({});
    let result = super::checked_fields(&mut output, &checked, &source);
    assert!(matches!(result, Err(Error::Payload { detail, .. })
        if detail == UnitError::UnsupportedExactConstantExport.to_string()));
    Ok(())
}
