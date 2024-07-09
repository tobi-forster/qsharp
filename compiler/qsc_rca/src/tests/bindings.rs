// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! These tests use bindings to check that RCA for the use of different static and dynamic types is correct.

#![allow(clippy::needless_raw_string_hashes)]

use super::{check_last_statement_compute_properties, CompilationContext};
use expect_test::expect;

#[test]
fn check_rca_for_immutable_classical_result_binding() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        let r = Zero;
        r"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![
            r#"
            ApplicationsGeneratorSet:
                inherent: Classical
                dynamic_param_applications: <empty>"#
        ],
    );
}

#[test]
fn check_rca_for_immutable_dynamic_result_binding() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        use q = Qubit();
        let r = M(q);
        r"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![
            r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(0x0)
                    value_kind: Element(Dynamic)
                dynamic_param_applications: <empty>"#
        ],
    );
}

#[test]
fn check_rca_for_mutable_classical_bool_binding() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        mutable b = true;
        b"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![
            r#"
            ApplicationsGeneratorSet:
                inherent: Classical
                dynamic_param_applications: <empty>"#
        ],
    );
}

#[test]
fn check_rca_for_mutable_dynamic_bool_binding() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        open Microsoft.Quantum.Convert;
        use q = Qubit();
        mutable b = ResultAsBool(M(q));
        b"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![
            r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(UseOfDynamicBool)
                    value_kind: Element(Dynamic)
                dynamic_param_applications: <empty>"#
        ],
    );
}

#[test]
fn check_rca_for_immutable_classical_int_binding() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        mutable i = 42;
        i"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![
            r#"
            ApplicationsGeneratorSet:
                inherent: Classical
                dynamic_param_applications: <empty>"#
        ],
    );
}

#[test]
fn check_rca_for_immutable_dynamic_int_binding() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        open Microsoft.Quantum.Convert;
        open Microsoft.Quantum.Measurement;
        use register = Qubit[8];
        let results = MeasureEachZ(register);
        let i = ResultArrayAsInt(results);
        i"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![
            r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(UseOfDynamicBool | UseOfDynamicInt)
                    value_kind: Element(Dynamic)
                dynamic_param_applications: <empty>"#
        ],
    );
}

#[test]
fn check_rca_for_mutable_classical_double_binding() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        mutable d = 42.0;
        d"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![
            r#"
            ApplicationsGeneratorSet:
                inherent: Classical
                dynamic_param_applications: <empty>"#
        ],
    );
}

#[test]
fn check_rca_for_mutable_dynamic_double_binding() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        open Microsoft.Quantum.Convert;
        open Microsoft.Quantum.Measurement;
        use register = Qubit[8];
        let results = MeasureEachZ(register);
        let i = ResultArrayAsInt(results);
        mutable d = IntAsDouble(i);
        d"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![
            r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(UseOfDynamicBool | UseOfDynamicInt | UseOfDynamicDouble)
                    value_kind: Element(Dynamic)
                dynamic_param_applications: <empty>"#
        ],
    );
}
