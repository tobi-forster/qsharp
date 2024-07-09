// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#![allow(clippy::needless_raw_string_hashes)]

use super::{check_last_statement_compute_properties, CompilationContext};
use expect_test::expect;

#[test]
fn check_rca_for_static_single_qubit_measurement() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
            use q = Qubit();
            M(q)"#,
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
fn check_rca_for_dynamic_single_measurement() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
            use (condition, target) = (Qubit(), Qubit());
            mutable r = Zero;
            if M(condition) == Zero {
                set r = M(target);
            }
            r"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(MeasurementWithinDynamicScope | UseOfDynamicResult)
                    value_kind: Element(Dynamic)
                dynamic_param_applications: <empty>"#]],
    );
}

#[test]
fn check_rca_for_static_single_measurement_and_reset() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
            open Microsoft.Quantum.Measurement;
            use q = Qubit();
            MResetZ(q)"#,
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
fn check_rca_for_dynamic_single_measurement_and_reset() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
            open Microsoft.Quantum.Measurement;
            use (condition, target) = (Qubit(), Qubit());
            mutable r = Zero;
            if MResetZ(condition) == Zero {
                set r = MResetZ(target);
            }
            r"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(MeasurementWithinDynamicScope | UseOfDynamicResult)
                    value_kind: Element(Dynamic)
                dynamic_param_applications: <empty>"#]],
    );
}

#[test]
fn check_rca_for_static_multi_qubit_measurement() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
            open Microsoft.Quantum.Measurement;
            use register = Qubit[2];
            MeasureEachZ(register)"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![
            r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(0x0)
                    value_kind: Array(Content: Dynamic, Size: Static)
                dynamic_param_applications: <empty>"#
        ],
    );
}
