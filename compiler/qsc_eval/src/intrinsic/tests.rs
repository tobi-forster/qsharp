// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#![allow(clippy::needless_raw_string_hashes)]

use std::f64::consts;

use crate::backend::{Backend, SparseSim};
use crate::tests::eval_graph;
use crate::Env;
use crate::{
    output::{GenericReceiver, Receiver},
    val::Value,
    Error,
};
use expect_test::{expect, Expect};
use indoc::indoc;
use num_bigint::BigInt;
use qsc_data_structures::language_features::LanguageFeatures;
use qsc_data_structures::target::TargetCapabilityFlags;
use qsc_fir::fir;
use qsc_frontend::compile::{self, compile, PackageStore, SourceMap};
use qsc_lowerer::map_hir_package_to_fir;
use qsc_passes::{run_core_passes, run_default_passes, PackageType};

#[derive(Default)]
struct CustomSim {
    sim: SparseSim,
}

impl Backend for CustomSim {
    type ResultType = bool;

    fn ccx(&mut self, ctl0: usize, ctl1: usize, q: usize) {
        self.sim.ccx(ctl0, ctl1, q);
    }

    fn cx(&mut self, ctl: usize, q: usize) {
        self.sim.cx(ctl, q);
    }

    fn cy(&mut self, ctl: usize, q: usize) {
        self.sim.cy(ctl, q);
    }

    fn cz(&mut self, ctl: usize, q: usize) {
        self.sim.cz(ctl, q);
    }

    fn h(&mut self, q: usize) {
        self.sim.h(q);
    }

    fn m(&mut self, q: usize) -> Self::ResultType {
        self.sim.m(q)
    }

    fn mresetz(&mut self, q: usize) -> Self::ResultType {
        self.sim.mresetz(q)
    }

    fn reset(&mut self, q: usize) {
        self.sim.reset(q);
    }

    fn rx(&mut self, theta: f64, q: usize) {
        self.sim.rx(theta, q);
    }

    fn rxx(&mut self, theta: f64, q0: usize, q1: usize) {
        self.sim.rxx(theta, q0, q1);
    }

    fn ry(&mut self, theta: f64, q: usize) {
        self.sim.ry(theta, q);
    }

    fn ryy(&mut self, theta: f64, q0: usize, q1: usize) {
        self.sim.ryy(theta, q0, q1);
    }

    fn rz(&mut self, theta: f64, q: usize) {
        self.sim.rz(theta, q);
    }

    fn rzz(&mut self, theta: f64, q0: usize, q1: usize) {
        self.sim.rzz(theta, q0, q1);
    }

    fn sadj(&mut self, q: usize) {
        self.sim.sadj(q);
    }

    fn s(&mut self, q: usize) {
        self.sim.s(q);
    }

    fn swap(&mut self, q0: usize, q1: usize) {
        self.sim.swap(q0, q1);
    }

    fn tadj(&mut self, q: usize) {
        self.sim.tadj(q);
    }

    fn t(&mut self, q: usize) {
        self.sim.t(q);
    }

    fn x(&mut self, q: usize) {
        self.sim.x(q);
    }

    fn y(&mut self, q: usize) {
        self.sim.y(q);
    }

    fn z(&mut self, q: usize) {
        self.sim.z(q);
    }

    fn qubit_allocate(&mut self) -> usize {
        self.sim.qubit_allocate()
    }

    fn qubit_release(&mut self, q: usize) {
        self.sim.qubit_release(q);
    }

    fn capture_quantum_state(
        &mut self,
    ) -> (Vec<(num_bigint::BigUint, num_complex::Complex<f64>)>, usize) {
        self.sim.capture_quantum_state()
    }

    fn qubit_is_zero(&mut self, q: usize) -> bool {
        self.sim.qubit_is_zero(q)
    }

    fn custom_intrinsic(&mut self, name: &str, arg: Value) -> Option<Result<Value, String>> {
        match name {
            "Add1" => Some(Ok(Value::Int(arg.unwrap_int() + 1))),
            "Check" => Some(Err("cannot verify input".to_string())),
            _ => None,
        }
    }
}

fn check_intrinsic(file: &str, expr: &str, out: &mut impl Receiver) -> Result<Value, Error> {
    let mut core = compile::core();
    run_core_passes(&mut core);
    let core_fir = qsc_lowerer::Lowerer::new().lower_package(&core.package);
    let mut store = PackageStore::new(core);

    let mut std = compile::std(&store, TargetCapabilityFlags::all());
    assert!(std.errors.is_empty());
    assert!(run_default_passes(store.core(), &mut std, PackageType::Lib).is_empty());
    let std_fir = qsc_lowerer::Lowerer::new().lower_package(&std.package);
    let std_id = store.insert(std);

    let sources = SourceMap::new([("test".into(), file.into())], Some(expr.into()));
    let mut unit = compile(
        &store,
        &[std_id],
        sources,
        TargetCapabilityFlags::all(),
        LanguageFeatures::default(),
    );
    assert!(unit.errors.is_empty());
    assert!(run_default_passes(store.core(), &mut unit, PackageType::Lib).is_empty());
    let unit_fir = qsc_lowerer::Lowerer::new().lower_package(&unit.package);
    let entry = unit_fir.entry_exec_graph.clone();

    let id = store.insert(unit);

    let mut fir_store = fir::PackageStore::new();
    fir_store.insert(
        map_hir_package_to_fir(qsc_hir::hir::PackageId::CORE),
        core_fir,
    );
    fir_store.insert(map_hir_package_to_fir(std_id), std_fir);
    fir_store.insert(map_hir_package_to_fir(id), unit_fir);

    eval_graph(
        entry,
        &mut CustomSim::default(),
        &fir_store,
        map_hir_package_to_fir(id),
        &mut Env::default(),
        out,
    )
    .map_err(|e| e.0)
}

fn check_intrinsic_result(file: &str, expr: &str, expect: &Expect) {
    let mut stdout = vec![];
    let mut out = GenericReceiver::new(&mut stdout);
    match check_intrinsic(file, expr, &mut out) {
        Ok(result) => expect.assert_eq(&result.to_string()),
        Err(e) => expect.assert_eq(&e.to_string()),
    }
}

fn check_intrinsic_output(file: &str, expr: &str, expect: &Expect) {
    let mut stdout = vec![];
    let mut out = GenericReceiver::new(&mut stdout);
    match check_intrinsic(file, expr, &mut out) {
        Ok(..) => expect.assert_eq(
            &String::from_utf8(stdout).expect("content should be convertable to string"),
        ),
        Err(e) => expect.assert_eq(&e.to_string()),
    }
}

fn check_intrinsic_value(file: &str, expr: &str, val: &Value) {
    let mut stdout = vec![];
    let mut out = GenericReceiver::new(&mut stdout);
    match check_intrinsic(file, expr, &mut out) {
        Ok(result) => assert_eq!(&result, val),
        Err(e) => panic!("{e:?}"),
    }
}

#[test]
fn int_as_double() {
    check_intrinsic_result(
        "",
        "Microsoft.Quantum.Convert.IntAsDouble(2)",
        &expect!["2.0"],
    );
}

#[test]
fn int_as_double_precision_loss() {
    check_intrinsic_result(
        "",
        "Microsoft.Quantum.Convert.IntAsDouble(9_223_372_036_854_775_807)",
        &expect!["9223372036854775808.0"],
    );
}

#[test]
fn dump_machine() {
    check_intrinsic_output(
        "",
        "Microsoft.Quantum.Diagnostics.DumpMachine()",
        &expect![[r#"
            STATE:
            |0⟩: 1.0000+0.0000𝑖
        "#]],
    );
}

#[test]
fn dump_machine_qubit_count() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use qs = Qubit[4];
            Microsoft.Quantum.Diagnostics.DumpMachine();
        }"},
        &expect![[r#"
            STATE:
            |0000⟩: 1.0000+0.0000𝑖
        "#]],
    );
}

#[test]
fn dump_machine_endianness() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use qs = Qubit[4];
            X(qs[1]);
            Microsoft.Quantum.Diagnostics.DumpMachine();
            X(qs[1]);
        }"},
        &expect![[r#"
            STATE:
            |0100⟩: 1.0000+0.0000𝑖
        "#]],
    );
}

#[test]
fn dump_register_all_qubits() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use qs = Qubit[4];
            X(qs[1]);
            Microsoft.Quantum.Diagnostics.DumpRegister(qs);
            X(qs[1]);
        }"},
        &expect![[r#"
            STATE:
            |0100⟩: 1.0000+0.0000𝑖
        "#]],
    );
}

#[test]
fn dump_register_subset_qubits() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use qs = Qubit[4];
            X(qs[1]);
            Microsoft.Quantum.Diagnostics.DumpRegister([qs[1], qs[2]]);
            X(qs[1]);
        }"},
        &expect![[r#"
            STATE:
            |10⟩: 1.0000+0.0000𝑖
        "#]],
    );
}

#[test]
fn dump_register_subset_entangled_within_subset_is_separable() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use (q1, q2, q3) = (Qubit(), Qubit(), Qubit());
            H(q1);
            CNOT(q1, q3);
            Microsoft.Quantum.Diagnostics.DumpRegister([q1, q3]);
            Reset(q1);
            Reset(q2);
            Reset(q3);
        }"},
        &expect![[r#"
            STATE:
            |00⟩: 0.7071+0.0000𝑖
            |11⟩: 0.7071+0.0000𝑖
        "#]],
    );
}

#[test]
fn dump_register_subset_entangled_with_other_qubits_not_separable() {
    check_intrinsic_result(
        "",
        indoc! {"{
            use (q1, q2, q3) = (Qubit(), Qubit(), Qubit());
            H(q1);
            CNOT(q1, q3);
            Microsoft.Quantum.Diagnostics.DumpRegister([q1, q2]);
        }"},
        &expect!["qubits are not separable"],
    );
}

#[test]
fn dump_register_other_qubits_superposition_is_separable() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use qs = Qubit[3];
            H(qs[0]);
            H(qs[2]);
            Microsoft.Quantum.Diagnostics.DumpRegister(qs[...1]);
            ResetAll(qs);
        }"},
        &expect![[r#"
            STATE:
            |00⟩: 0.7071+0.0000𝑖
            |10⟩: 0.7071+0.0000𝑖
        "#]],
    );
}

#[test]
fn dump_register_other_qubits_one_state_is_separable() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use qs = Qubit[3];
            H(qs[0]);
            X(qs[2]);
            Microsoft.Quantum.Diagnostics.DumpRegister(qs[...1]);
            ResetAll(qs);
        }"},
        &expect![[r#"
            STATE:
            |00⟩: 0.7071+0.0000𝑖
            |10⟩: 0.7071+0.0000𝑖
        "#]],
    );
}

#[test]
fn dump_register_qubits_reorder_output() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use qs = Qubit[5];
            H(qs[0]);
            X(qs[2]);
            Microsoft.Quantum.Diagnostics.DumpMachine();
            Microsoft.Quantum.Diagnostics.DumpRegister(qs[2..-1...]);
            ResetAll(qs);
        }"},
        &expect![[r#"
            STATE:
            |00100⟩: 0.7071+0.0000𝑖
            |10100⟩: 0.7071+0.0000𝑖
            STATE:
            |100⟩: 0.7071+0.0000𝑖
            |101⟩: 0.7071+0.0000𝑖
        "#]],
    );
}

#[test]
fn dump_register_qubits_reorder_output_should_be_sorted() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use qs = Qubit[5];
            H(qs[0]);
            H(qs[2]);
            Microsoft.Quantum.Diagnostics.DumpMachine();
            Microsoft.Quantum.Diagnostics.DumpRegister(qs[0..2..3]);
            ResetAll(qs);
        }"},
        &expect![[r#"
            STATE:
            |00000⟩: 0.5000+0.0000𝑖
            |00100⟩: 0.5000+0.0000𝑖
            |10000⟩: 0.5000+0.0000𝑖
            |10100⟩: 0.5000+0.0000𝑖
            STATE:
            |00⟩: 0.5000+0.0000𝑖
            |01⟩: 0.5000+0.0000𝑖
            |10⟩: 0.5000+0.0000𝑖
            |11⟩: 0.5000+0.0000𝑖
        "#]],
    );
}

#[test]
fn dump_register_qubits_not_unique_fails() {
    check_intrinsic_result(
        "",
        indoc! {"{
            use qs = Qubit[3];
            H(qs[0]);
            Microsoft.Quantum.Diagnostics.DumpRegister([qs[0], qs[0]]);
        }"},
        &expect!["qubits in invocation are not unique"],
    );
}

#[test]
fn dump_register_target_in_minus_with_other_in_zero() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use qs = Qubit[2];
            X(qs[0]);
            H(qs[0]);
            Microsoft.Quantum.Diagnostics.DumpRegister([qs[0]]);
            ResetAll(qs);
        }"},
        &expect![[r#"
            STATE:
            |0⟩: 0.7071+0.0000𝑖
            |1⟩: −0.7071+0.0000𝑖
        "#]],
    );
}

#[test]
fn dump_register_target_in_minus_with_other_in_one() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use qs = Qubit[2];
            X(qs[1]);
            X(qs[0]);
            H(qs[0]);
            Microsoft.Quantum.Diagnostics.DumpRegister([qs[0]]);
            ResetAll(qs);
        }"},
        &expect![[r#"
            STATE:
            |0⟩: 0.7071+0.0000𝑖
            |1⟩: −0.7071+0.0000𝑖
        "#]],
    );
}

#[test]
fn dump_register_all_qubits_normalized_is_same_as_dump_machine() {
    check_intrinsic_output(
        "",
        indoc! {
        "{
            open Microsoft.Quantum.Diagnostics;
            use qs = Qubit[2];

            let alpha = -4.20025;
            let beta = 2.04776;
            let gamma = -5.47097;

            within{
                Ry(alpha, qs[0]);
                Ry(beta, qs[1]);
                CNOT(qs[0], qs[1]);
                Ry(gamma, qs[1]);
            }
            apply{
                DumpRegister(qs);
                DumpMachine();
            }
        }"
        },
        &expect![[r#"
            STATE:
            |00⟩: 0.0709+0.0000𝑖
            |01⟩: 0.5000+0.0000𝑖
            |10⟩: 0.5000+0.0000𝑖
            |11⟩: 0.7036+0.0000𝑖
            STATE:
            |00⟩: 0.0709+0.0000𝑖
            |01⟩: 0.5000+0.0000𝑖
            |10⟩: 0.5000+0.0000𝑖
            |11⟩: 0.7036+0.0000𝑖
        "#]],
    );
}

#[test]
fn message() {
    check_intrinsic_output(
        "",
        r#"Message("Hello, World!")"#,
        &expect![[r#"
            Hello, World!
        "#]],
    );
}

#[test]
fn check_zero() {
    check_intrinsic_result(
        "",
        "{use q = Qubit(); Microsoft.Quantum.Diagnostics.CheckZero(q)}",
        &expect!["true"],
    );
}

#[test]
fn check_zero_false() {
    check_intrinsic_result(
        "",
        indoc! {"{
            use q = Qubit();
            X(q);
            let isZero = Microsoft.Quantum.Diagnostics.CheckZero(q);
            X(q);
            isZero
        }"},
        &expect!["false"],
    );
}

#[test]
fn length() {
    check_intrinsic_value("", "Length([1, 2, 3])", &Value::Int(3));
}

#[test]
fn arccos() {
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Math.ArcCos(0.3)",
        &Value::Double((0.3f64).acos()),
    );
}

#[test]
fn arcsin() {
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Math.ArcSin(0.3)",
        &Value::Double((0.3f64).asin()),
    );
}

#[test]
fn arctan() {
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Math.ArcTan(0.3)",
        &Value::Double((0.3f64).atan()),
    );
}

#[test]
fn arctan2() {
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Math.ArcTan2(0.3, 0.7)",
        &Value::Double((0.3f64).atan2(0.7)),
    );
}

#[test]
fn cos() {
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Math.Cos(Microsoft.Quantum.Math.PI())",
        &Value::Double((consts::PI).cos()),
    );
}

#[test]
fn cosh() {
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Math.Cosh(Microsoft.Quantum.Math.PI())",
        &Value::Double((consts::PI).cosh()),
    );
}

#[test]
fn sin() {
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Math.Sin(Microsoft.Quantum.Math.PI())",
        &Value::Double((consts::PI).sin()),
    );
}

#[test]
fn sinh() {
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Math.Sinh(Microsoft.Quantum.Math.PI())",
        &Value::Double((consts::PI).sinh()),
    );
}

#[test]
fn tan() {
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Math.Tan(Microsoft.Quantum.Math.PI())",
        &Value::Double((consts::PI).tan()),
    );
}

#[test]
fn tanh() {
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Math.Tanh(Microsoft.Quantum.Math.PI())",
        &Value::Double((consts::PI).tanh()),
    );
}

#[test]
fn draw_random_int() {
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Random.DrawRandomInt(5,5)",
        &Value::Int(5),
    );
}

#[test]
fn draw_random_double() {
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Random.DrawRandomDouble(5.0,5.0)",
        &Value::Double(5.0),
    );
}

#[test]
fn truncate() {
    check_intrinsic_value("", "Microsoft.Quantum.Math.Truncate(3.1)", &Value::Int(3));
    check_intrinsic_value("", "Microsoft.Quantum.Math.Truncate(3.9)", &Value::Int(3));
    check_intrinsic_value("", "Microsoft.Quantum.Math.Truncate(-3.1)", &Value::Int(-3));
    check_intrinsic_value("", "Microsoft.Quantum.Math.Truncate(-3.9)", &Value::Int(-3));
}

#[test]
fn sqrt() {
    check_intrinsic_value("", "Microsoft.Quantum.Math.Sqrt(0.0)", &Value::Double(0.0));
    check_intrinsic_value("", "Microsoft.Quantum.Math.Sqrt(81.0)", &Value::Double(9.0));
}

#[test]
fn log() {
    check_intrinsic_value("", "Microsoft.Quantum.Math.Log(1.0)", &Value::Double(0.0));
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Math.Log(Microsoft.Quantum.Math.E())",
        &Value::Double(1.0),
    );
}

#[test]
fn int_as_bigint() {
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Convert.IntAsBigInt(0)",
        &Value::BigInt(BigInt::from(0)),
    );
    check_intrinsic_value(
        "",
        "Microsoft.Quantum.Convert.IntAsBigInt(-10000)",
        &Value::BigInt(BigInt::from(-10000)),
    );
}

#[test]
fn ccx() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use (q1, q2, q3) = (Qubit(), Qubit(), Qubit());
            QIR.Intrinsic.__quantum__qis__ccx__body(q1, q2, q3);
            if not Microsoft.Quantum.Diagnostics.CheckZero(q3) {
                fail "Qubit should still be in zero state.";
            }
            X(q1);
            X(q2);
            QIR.Intrinsic.__quantum__qis__ccx__body(q1, q2, q3);
            if Microsoft.Quantum.Diagnostics.CheckZero(q3) {
                fail "Qubit should be in one state.";
            }
            X(q3);
            X(q2);
            X(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q3)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn cx() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use (q1, q2) = (Qubit(), Qubit());
            QIR.Intrinsic.__quantum__qis__cx__body(q1, q2);
            if not Microsoft.Quantum.Diagnostics.CheckZero(q2) {
                fail "Qubit should still be in zero state.";
            }
            X(q1);
            QIR.Intrinsic.__quantum__qis__cx__body(q1, q2);
            if Microsoft.Quantum.Diagnostics.CheckZero(q2) {
                fail "Qubit should be in one state.";
            }
            X(q2);
            X(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q2)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn cy() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use (q1, q2) = (Qubit(), Qubit());
            QIR.Intrinsic.__quantum__qis__cy__body(q1, q2);
            if not Microsoft.Quantum.Diagnostics.CheckZero(q2) {
                fail "Qubit should still be in zero state.";
            }
            X(q1);
            QIR.Intrinsic.__quantum__qis__cy__body(q1, q2);
            if Microsoft.Quantum.Diagnostics.CheckZero(q2) {
                fail "Qubit should be in one state.";
            }
            Y(q2);
            X(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q2)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn cz() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use (q1, q2) = (Qubit(), Qubit());
            H(q2);
            QIR.Intrinsic.__quantum__qis__cz__body(q1, q2);
            H(q2);
            if not Microsoft.Quantum.Diagnostics.CheckZero(q2) {
                fail "Qubit should still be in zero state.";
            }
            X(q1);
            H(q2);
            QIR.Intrinsic.__quantum__qis__cz__body(q1, q2);
            H(q2);
            if Microsoft.Quantum.Diagnostics.CheckZero(q2) {
                fail "Qubit should be in one state.";
            }
            X(q2);
            X(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q2)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn rx() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use q1 = Qubit();
            let pi = Microsoft.Quantum.Math.PI();
            QIR.Intrinsic.__quantum__qis__rx__body(pi, q1);
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in one state.";
            }
            X(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q1)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn rxx() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use (q1, q2) = (Qubit(), Qubit());
            let pi = Microsoft.Quantum.Math.PI();
            QIR.Intrinsic.__quantum__qis__rxx__body(pi, q1, q2);
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in one state.";
            }
            if Microsoft.Quantum.Diagnostics.CheckZero(q2) {
                fail "Qubit 2 should be in one state.";
            }
            X(q2);
            X(q1);
            (Microsoft.Quantum.Diagnostics.CheckZero(q1), Microsoft.Quantum.Diagnostics.CheckZero(q2))
        }"#},
        &expect!["(true, true)"],
    );
}

#[test]
fn ry() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use q1 = Qubit();
            let pi = Microsoft.Quantum.Math.PI();
            QIR.Intrinsic.__quantum__qis__ry__body(pi, q1);
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in one state.";
            }
            Y(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q1)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn ryy() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use (q1, q2) = (Qubit(), Qubit());
            let pi = Microsoft.Quantum.Math.PI();
            QIR.Intrinsic.__quantum__qis__ryy__body(pi, q1, q2);
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in one state.";
            }
            if Microsoft.Quantum.Diagnostics.CheckZero(q2) {
                fail "Qubit 2 should be in one state.";
            }
            Y(q2);
            Y(q1);
            (Microsoft.Quantum.Diagnostics.CheckZero(q1), Microsoft.Quantum.Diagnostics.CheckZero(q2))
        }"#},
        &expect!["(true, true)"],
    );
}

#[test]
fn rz() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use q1 = Qubit();
            let pi = Microsoft.Quantum.Math.PI();
            H(q1);
            QIR.Intrinsic.__quantum__qis__rz__body(pi, q1);
            H(q1);
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in one state.";
            }
            H(q1);
            Z(q1);
            H(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q1)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn rzz() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use (q1, q2) = (Qubit(), Qubit());
            let pi = Microsoft.Quantum.Math.PI();
            H(q1);
            H(q2);
            QIR.Intrinsic.__quantum__qis__rzz__body(pi, q1, q2);
            H(q1);
            H(q2);
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in one state.";
            }
            if Microsoft.Quantum.Diagnostics.CheckZero(q2) {
                fail "Qubit 2 should be in one state.";
            }
            H(q2);
            H(q1);
            Z(q2);
            Z(q1);
            H(q2);
            H(q1);
            (Microsoft.Quantum.Diagnostics.CheckZero(q1), Microsoft.Quantum.Diagnostics.CheckZero(q2))
        }"#},
        &expect!["(true, true)"],
    );
}

#[test]
fn h() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use q1 = Qubit();
            QIR.Intrinsic.__quantum__qis__h__body(q1);
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in one state.";
            }
            H(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q1)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn s() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use q1 = Qubit();
            H(q1);
            QIR.Intrinsic.__quantum__qis__s__body(q1);
            H(q1);
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in one state.";
            }
            H(q1);
            QIR.Intrinsic.__quantum__qis__s__body(q1);
            QIR.Intrinsic.__quantum__qis__s__body(q1);
            QIR.Intrinsic.__quantum__qis__s__body(q1);
            H(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q1)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn sadj() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use q1 = Qubit();
            H(q1);
            QIR.Intrinsic.__quantum__qis__s__adj(q1);
            H(q1);
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in one state.";
            }
            H(q1);
            QIR.Intrinsic.__quantum__qis__s__adj(q1);
            QIR.Intrinsic.__quantum__qis__s__adj(q1);
            QIR.Intrinsic.__quantum__qis__s__adj(q1);
            H(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q1)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn t() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use q1 = Qubit();
            H(q1);
            QIR.Intrinsic.__quantum__qis__t__body(q1);
            QIR.Intrinsic.__quantum__qis__t__body(q1);
            H(q1);
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in one state.";
            }
            H(q1);
            QIR.Intrinsic.__quantum__qis__t__body(q1);
            QIR.Intrinsic.__quantum__qis__t__body(q1);
            QIR.Intrinsic.__quantum__qis__t__body(q1);
            QIR.Intrinsic.__quantum__qis__t__body(q1);
            QIR.Intrinsic.__quantum__qis__t__body(q1);
            QIR.Intrinsic.__quantum__qis__t__body(q1);
            H(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q1)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn tadj() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use q1 = Qubit();
            H(q1);
            QIR.Intrinsic.__quantum__qis__t__adj(q1);
            QIR.Intrinsic.__quantum__qis__t__adj(q1);
            H(q1);
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in one state.";
            }
            H(q1);
            QIR.Intrinsic.__quantum__qis__t__adj(q1);
            QIR.Intrinsic.__quantum__qis__t__adj(q1);
            QIR.Intrinsic.__quantum__qis__t__adj(q1);
            QIR.Intrinsic.__quantum__qis__t__adj(q1);
            QIR.Intrinsic.__quantum__qis__t__adj(q1);
            QIR.Intrinsic.__quantum__qis__t__adj(q1);
            H(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q1)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn x() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use q1 = Qubit();
            QIR.Intrinsic.__quantum__qis__x__body(q1);
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in one state.";
            }
            QIR.Intrinsic.__quantum__qis__x__body(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q1)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn y() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use q1 = Qubit();
            QIR.Intrinsic.__quantum__qis__y__body(q1);
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in one state.";
            }
            QIR.Intrinsic.__quantum__qis__y__body(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q1)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn z() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use q1 = Qubit();
            H(q1);
            QIR.Intrinsic.__quantum__qis__z__body(q1);
            H(q1);
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in one state.";
            }
            H(q1);
            QIR.Intrinsic.__quantum__qis__z__body(q1);
            H(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q1)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn swap() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use (q1, q2) = (Qubit(), Qubit());
            X(q2);
            QIR.Intrinsic.__quantum__qis__swap__body(q1, q2);
            if not Microsoft.Quantum.Diagnostics.CheckZero(q2) {
                fail "Qubit should be swapped to zero state.";
            }
            if Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should swapped to one state.";
            }
            X(q1);
            (Microsoft.Quantum.Diagnostics.CheckZero(q2), Microsoft.Quantum.Diagnostics.CheckZero(q2))
        }"#},
        &expect!["(true, true)"],
    );
}

#[test]
fn reset() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use q1 = Qubit();
            QIR.Intrinsic.__quantum__qis__reset__body(q1);
            if not Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in zero state.";
            }
            X(q1);
            QIR.Intrinsic.__quantum__qis__reset__body(q1);
            Microsoft.Quantum.Diagnostics.CheckZero(q1)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn reset_all() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use register = Qubit[2];
            ResetAll(register);
            if not Microsoft.Quantum.Diagnostics.CheckAllZero(register) {
                fail "Qubits should be in zero state.";
            }

            for q in register {
                X(q);
            }

            ResetAll(register);
            Microsoft.Quantum.Diagnostics.CheckAllZero(register)
        }"#},
        &expect!["true"],
    );
}

#[test]
fn m() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use q1 = Qubit();
            if not Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in zero state.";
            }
            let res1 = QIR.Intrinsic.__quantum__qis__m__body(q1);
            if One == res1 {
                fail "Qubit should measure Zero"
            }
            if not Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in zero state.";
            }
            X(q1);
            let res2 = (QIR.Intrinsic.__quantum__qis__m__body(q1), Microsoft.Quantum.Diagnostics.CheckZero(q1));
            X(q1);
            res2
        }"#},
        &expect!["(One, false)"],
    );
}

#[test]
fn mresetz() {
    check_intrinsic_result(
        "",
        indoc! {r#"{
            use q1 = Qubit();
            if not Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in zero state.";
            }
            let res1 = QIR.Intrinsic.__quantum__qis__mresetz__body(q1);
            if One == res1 {
                fail "Qubit should measure Zero"
            }
            if not Microsoft.Quantum.Diagnostics.CheckZero(q1) {
                fail "Qubit should be in zero state.";
            }
            X(q1);
            let res2 = QIR.Intrinsic.__quantum__qis__mresetz__body(q1);
            (res2, Microsoft.Quantum.Diagnostics.CheckZero(q1))
        }"#},
        &expect!["(One, true)"],
    );
}

#[test]
fn unknown_intrinsic() {
    check_intrinsic_result(
        indoc! {"
            namespace Test {
                function Foo() : Int {
                    body intrinsic;
                }
            }
        "},
        "Test.Foo()",
        &expect!["unknown intrinsic `Foo`"],
    );
}

#[test]
fn custom_intrinsic_success() {
    check_intrinsic_result(
        indoc! {"
            namespace Test {
                function Add1(input : Int) : Int {
                    body intrinsic;
                }
            }
        "},
        "Test.Add1(1)",
        &expect!["2"],
    );
}

#[test]
fn custom_intrinsic_failure() {
    check_intrinsic_result(
        indoc! {"
            namespace Test {
                function Check(input : Int) : Int {
                    body intrinsic;
                }
            }
        "},
        "Test.Check(1)",
        &expect!["intrinsic callable `Check` failed: cannot verify input"],
    );
}

#[test]
fn qubit_nested_bind_not_released() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use aux = Qubit();
            use q = Qubit();
            {
                let temp = q;
                X(temp);
            }
            Microsoft.Quantum.Diagnostics.DumpMachine();
            X(q);
        }"},
        &expect![[r#"
            STATE:
            |01⟩: 1.0000+0.0000𝑖
        "#]],
    );
}

#[test]
fn qubit_release_non_zero_failure() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use q = Qubit();
            X(q);
        }"},
        &expect!["Qubit0 released while not in |0⟩ state"],
    );
}

#[test]
fn qubit_not_unique_two_qubit_error() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use q = Qubit();
            CNOT(q , q);
        }"},
        &expect!["qubits in invocation are not unique"],
    );
}

#[test]
fn qubit_not_unique_two_qubit_rotation_error() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use q = Qubit();
            Rxx(0.1, q, q);
        }"},
        &expect!["qubits in invocation are not unique"],
    );
}

#[test]
fn qubit_not_unique_three_qubit_error_first_second() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use q = Qubit();
            use a = Qubit();
            CCNOT(q , q, a);
        }"},
        &expect!["qubits in invocation are not unique"],
    );
}

#[test]
fn qubit_not_unique_three_qubit_error_first_third() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use q = Qubit();
            use a = Qubit();
            CCNOT(q , a, q);
        }"},
        &expect!["qubits in invocation are not unique"],
    );
}

#[test]
fn qubit_not_unique_three_qubit_error_second_third() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use q = Qubit();
            use a = Qubit();
            CCNOT(a , q, q);
        }"},
        &expect!["qubits in invocation are not unique"],
    );
}

#[test]
fn single_qubit_rotation_nan_error() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use q = Qubit();
            Rx(Microsoft.Quantum.Math.ArcSin(2.0), q);
        }"},
        &expect!["invalid rotation angle: NaN"],
    );
}

#[test]
fn two_qubit_rotation_nan_error() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use (q1, q2) = (Qubit(), Qubit());
            Rxx(Microsoft.Quantum.Math.ArcSin(2.0), q1, q2);
        }"},
        &expect!["invalid rotation angle: NaN"],
    );
}

#[test]
fn single_qubit_rotation_inf_error() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use q = Qubit();
            Rx(-Microsoft.Quantum.Math.Log(0.0), q);
        }"},
        &expect!["invalid rotation angle: inf"],
    );
}

#[test]
fn two_qubit_rotation_inf_error() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use (q1, q2) = (Qubit(), Qubit());
            Rxx(-Microsoft.Quantum.Math.Log(0.0), q1, q2);
        }"},
        &expect!["invalid rotation angle: inf"],
    );
}

#[test]
fn single_qubit_rotation_neg_inf_error() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use q = Qubit();
            Rx(Microsoft.Quantum.Math.Log(0.0), q);
        }"},
        &expect!["invalid rotation angle: -inf"],
    );
}

#[test]
fn two_qubit_rotation_neg_inf_error() {
    check_intrinsic_output(
        "",
        indoc! {"{
            use (q1, q2) = (Qubit(), Qubit());
            Rxx(Microsoft.Quantum.Math.Log(0.0), q1, q2);
        }"},
        &expect!["invalid rotation angle: -inf"],
    );
}
