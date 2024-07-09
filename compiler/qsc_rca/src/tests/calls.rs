// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#![allow(clippy::needless_raw_string_hashes)]

use super::{check_last_statement_compute_properties, CompilationContext};
use expect_test::expect;

#[test]
fn check_rca_for_call_to_cyclic_function_with_classical_argument() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        function GaussSum(n : Int) : Int {
            if n == 0 {
                0
            } else {
                n + GaussSum(n - 1)
            }
        }
        GaussSum(10)"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Classical
                dynamic_param_applications: <empty>"#]],
    );
}

#[test]
fn check_rca_for_call_to_cyclic_function_with_dynamic_argument() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        function GaussSum(n : Int) : Int {
            if n == 0 {
                0
            } else {
                n + GaussSum(n - 1)
            }
        }
        use q = Qubit();
        GaussSum(M(q) == Zero ? 10 | 20)"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(UseOfDynamicBool | UseOfDynamicInt | CallToCyclicFunctionWithDynamicArg)
                    value_kind: Element(Dynamic)
                dynamic_param_applications: <empty>"#]],
    );
}

#[test]
fn check_rca_for_call_to_cyclic_operation_with_classical_argument() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        operation GaussSum(n : Int) : Int {
            if n == 0 {
                0
            } else {
                n + GaussSum(n - 1)
            }
        }
        GaussSum(10)"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(UseOfDynamicInt | CallToCyclicOperation)
                    value_kind: Element(Dynamic)
                dynamic_param_applications: <empty>"#]],
    );
}

#[test]
fn check_rca_for_call_to_cyclic_operation_with_dynamic_argument() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        operation GaussSum(n : Int) : Int {
            if n == 0 {
                0
            } else {
                n + GaussSum(n - 1)
            }
        }
        use q = Qubit();
        GaussSum(M(q) == Zero ? 10 | 20)"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(UseOfDynamicBool | UseOfDynamicInt | CallToCyclicOperation)
                    value_kind: Element(Dynamic)
                dynamic_param_applications: <empty>"#]],
    );
}

#[test]
fn check_rca_for_call_to_static_closure_function() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        open Microsoft.Quantum.Math;
        let f = i -> IsCoprimeI(11, i);
        f(13)"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();

    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Classical
                dynamic_param_applications: <empty>"#]],
    );
}

#[test]
fn check_rca_for_call_to_dynamic_closure_function() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        open Microsoft.Quantum.Math;
        use q = Qubit();
        let dynamicInt = M(q) == Zero ? 11 | 13;
        let f = i -> IsCoprimeI(dynamicInt, i);
        f(17)"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();

    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(UseOfDynamicBool | UseOfDynamicInt | LoopWithDynamicCondition)
                    value_kind: Element(Dynamic)
                dynamic_param_applications: <empty>"#]],
    );
}

#[test]
fn check_rca_for_call_to_static_closure_operation() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        open Microsoft.Quantum.Math;
        use qubit = Qubit();
        let theta = PI();
        let f = q => Rx(theta, q);
        f(qubit)"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();

    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(0x0)
                    value_kind: Element(Static)
                dynamic_param_applications: <empty>"#]],
    );
}

#[test]
fn check_rca_for_call_to_dynamic_closure_operation() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        open Microsoft.Quantum.Math;
        use qubit = Qubit();
        let theta = M(qubit) == Zero ? PI() | PI() / 2.0;
        let f = q => Rx(theta, q);
        f(qubit)"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();

    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(UseOfDynamicDouble)
                    value_kind: Element(Static)
                dynamic_param_applications: <empty>"#]],
    );
}

#[test]
fn check_rca_for_call_to_operation_with_one_classical_return_and_one_dynamic_return() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        operation Foo() : Int {
            use q = Qubit();
            if M(q) == Zero {
                return 0;
            }
            return 1;
        }
        Foo()"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(UseOfDynamicBool | UseOfDynamicInt | ReturnWithinDynamicScope)
                    value_kind: Element(Dynamic)
                dynamic_param_applications: <empty>"#]],
    );
}

#[test]
fn check_rca_for_call_to_operation_with_codegen_intrinsic_override_treated_as_intrinsic() {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        @SimulatableIntrinsic()
        operation Foo() : Unit {
            mutable a = 0;
            use q = Qubit();
            if M(q) == Zero {
                set a = 1;
            }
            Message($"a = {a}");
        }
        Foo()"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(0x0)
                    value_kind: Element(Static)
                dynamic_param_applications: <empty>"#]],
    );
}

#[test]
fn check_rca_for_call_to_operation_with_codegen_intrinsic_override_treated_as_intrinsic_that_takes_qubit_arg(
) {
    let mut compilation_context = CompilationContext::default();
    compilation_context.update(
        r#"
        @SimulatableIntrinsic()
        operation Foo(q : Qubit) : Unit {
            mutable a = 0;
            if M(q) == Zero {
                set a = 1;
            }
            Message($"a = {a}");
        }
        use q = Qubit();
        Foo(q)"#,
    );
    let package_store_compute_properties = compilation_context.get_compute_properties();
    check_last_statement_compute_properties(
        package_store_compute_properties,
        &expect![[r#"
            ApplicationsGeneratorSet:
                inherent: Quantum: QuantumProperties:
                    runtime_features: RuntimeFeatureFlags(0x0)
                    value_kind: Element(Static)
                dynamic_param_applications: <empty>"#]],
    );
}
