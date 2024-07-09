// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#![allow(clippy::needless_raw_string_hashes)]

use super::{check_last_statement_compute_properties, CompilationContext};
use expect_test::expect;

#[test]
fn check_rca_for_bin_op_with_classical_lhs_and_classical_rhs() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(r#"1 + 1"#);
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
fn check_rca_for_bin_op_with_dynamic_lhs_and_classical_rhs() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        use q = Qubit();
        M(q) != Zero"#,
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
fn check_rca_for_bin_op_with_classical_lhs_and_dynamic_rhs() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        use q = Qubit();
        One == M(q)"#,
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
fn check_rca_for_bin_op_with_dynamic_lhs_and_dynamic_rhs() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        use (a, b) = (Qubit(), Qubit());
        let (c, d) = (M(a) == Zero, M(b) == Zero);
        c and d"#,
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
fn check_rca_for_nested_bin_ops_with_classic_operands() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        open Microsoft.Quantum.Math;
        Sin(PI() / 2.0) ^ 2.0 + Cos(PI() / 2.0) ^ 2.0"#,
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
fn check_rca_for_nested_bin_ops_with_a_dynamic_operand() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        use q = Qubit();
        let i = M(q) == Zero ? 0 | 1;
        i * 1 / 1"#,
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
fn check_rca_for_exp_op_with_classical_lhs_and_classical_rhs() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(r#"2 ^ 3"#);
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
fn check_rca_for_exp_op_with_dynamic_lhs_and_classical_rhs() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"{
            use q = Qubit();
            let i = M(q) == Zero ? 0 | 1;
            i ^ 2
        }"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(UseOfDynamicBool | UseOfDynamicInt)
                    value_kind: Element(Dynamic)
                dynamic_param_applications: <empty>"#]],
    );
}

#[test]
fn check_rca_for_exp_op_with_classical_lhs_and_dynamic_rhs() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"{
            use q = Qubit();
            let i = M(q) == Zero ? 0 | 1;
            2 ^ i
        }"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(UseOfDynamicBool | UseOfDynamicInt | UseOfDynamicExponent)
                    value_kind: Element(Dynamic)
                dynamic_param_applications: <empty>"#]],
    );
}

#[test]
fn check_rca_for_exp_op_with_dynamic_lhs_and_dynamic_rhs() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"{
            use (a, b) = (Qubit(), Qubit());
            let (c, d) = (M(a) == Zero, M(b) == Zero);
            let i = c ? 0 | 1;
            let j = d ? 0 | 1;
            i ^ j
        }"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(UseOfDynamicBool | UseOfDynamicInt | UseOfDynamicExponent)
                    value_kind: Element(Dynamic)
                dynamic_param_applications: <empty>"#]],
    );
}
