// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use crate::{
    linter::{ast::run_ast_lints, hir::run_hir_lints, Compilation},
    Lint, LintConfig, LintLevel,
};
use expect_test::{expect, Expect};
use indoc::indoc;
use qsc_data_structures::{
    language_features::LanguageFeatures, span::Span, target::TargetCapabilityFlags,
};
use qsc_frontend::compile::{self, CompileUnit, PackageStore, SourceMap};
use qsc_hir::hir::CallableKind;
use qsc_passes::PackageType;

#[test]
fn multiple_lints() {
    check(
        &wrap_in_callable("let x = ((1 + 2)) / 0;;;;", CallableKind::Operation),
        &expect![[r#"
            [
                SrcLint {
                    source: ";;;",
                    level: Warn,
                    message: "redundant semicolons",
                    help: "remove the redundant semicolons",
                    code_action_edits: [
                        (
                            "",
                            Span {
                                lo: 94,
                                hi: 97,
                            },
                        ),
                    ],
                },
                SrcLint {
                    source: "((1 + 2)) / 0",
                    level: Error,
                    message: "attempt to divide by zero",
                    help: "division by zero will fail at runtime",
                    code_action_edits: [],
                },
                SrcLint {
                    source: "((1 + 2))",
                    level: Allow,
                    message: "unnecessary parentheses",
                    help: "remove the extra parentheses for clarity",
                    code_action_edits: [
                        (
                            "",
                            Span {
                                lo: 80,
                                hi: 81,
                            },
                        ),
                        (
                            "",
                            Span {
                                lo: 88,
                                hi: 89,
                            },
                        ),
                    ],
                },
                SrcLint {
                    source: "RunProgram",
                    level: Allow,
                    message: "operation does not contain any quantum operations",
                    help: "this callable can be declared as a function instead",
                    code_action_edits: [],
                },
            ]
        "#]],
    );
}

#[test]
fn double_parens() {
    check(
        &wrap_in_callable("let x = ((1 + 2));", CallableKind::Function),
        &expect![[r#"
            [
                SrcLint {
                    source: "((1 + 2))",
                    level: Allow,
                    message: "unnecessary parentheses",
                    help: "remove the extra parentheses for clarity",
                    code_action_edits: [
                        (
                            "",
                            Span {
                                lo: 79,
                                hi: 80,
                            },
                        ),
                        (
                            "",
                            Span {
                                lo: 87,
                                hi: 88,
                            },
                        ),
                    ],
                },
            ]
        "#]],
    );
}

#[test]
fn division_by_zero() {
    check(
        &wrap_in_callable("let x = 2 / 0;", CallableKind::Function),
        &expect![[r#"
            [
                SrcLint {
                    source: "2 / 0",
                    level: Error,
                    message: "attempt to divide by zero",
                    help: "division by zero will fail at runtime",
                    code_action_edits: [],
                },
            ]
        "#]],
    );
}

#[test]
fn needless_parens_in_assignment() {
    check(
        &wrap_in_callable("let x = (42);", CallableKind::Function),
        &expect![[r#"
            [
                SrcLint {
                    source: "(42)",
                    level: Allow,
                    message: "unnecessary parentheses",
                    help: "remove the extra parentheses for clarity",
                    code_action_edits: [
                        (
                            "",
                            Span {
                                lo: 79,
                                hi: 80,
                            },
                        ),
                        (
                            "",
                            Span {
                                lo: 82,
                                hi: 83,
                            },
                        ),
                    ],
                },
            ]
        "#]],
    );
}

#[test]
fn needless_parens() {
    check(
        &wrap_in_callable("let x = (2) + (5 * 4 * (2 ^ 10));", CallableKind::Function),
        &expect![[r#"
            [
                SrcLint {
                    source: "(2)",
                    level: Allow,
                    message: "unnecessary parentheses",
                    help: "remove the extra parentheses for clarity",
                    code_action_edits: [
                        (
                            "",
                            Span {
                                lo: 79,
                                hi: 80,
                            },
                        ),
                        (
                            "",
                            Span {
                                lo: 81,
                                hi: 82,
                            },
                        ),
                    ],
                },
                SrcLint {
                    source: "(5 * 4 * (2 ^ 10))",
                    level: Allow,
                    message: "unnecessary parentheses",
                    help: "remove the extra parentheses for clarity",
                    code_action_edits: [
                        (
                            "",
                            Span {
                                lo: 85,
                                hi: 86,
                            },
                        ),
                        (
                            "",
                            Span {
                                lo: 102,
                                hi: 103,
                            },
                        ),
                    ],
                },
                SrcLint {
                    source: "(2 ^ 10)",
                    level: Allow,
                    message: "unnecessary parentheses",
                    help: "remove the extra parentheses for clarity",
                    code_action_edits: [
                        (
                            "",
                            Span {
                                lo: 94,
                                hi: 95,
                            },
                        ),
                        (
                            "",
                            Span {
                                lo: 101,
                                hi: 102,
                            },
                        ),
                    ],
                },
            ]
        "#]],
    );
}

#[test]
fn redundant_semicolons() {
    check(
        &wrap_in_callable("let x = 2;;;;;", CallableKind::Function),
        &expect![[r#"
            [
                SrcLint {
                    source: ";;;;",
                    level: Warn,
                    message: "redundant semicolons",
                    help: "remove the redundant semicolons",
                    code_action_edits: [
                        (
                            "",
                            Span {
                                lo: 81,
                                hi: 85,
                            },
                        ),
                    ],
                },
            ]
        "#]],
    );
}

#[test]
fn needless_operation_lambda_operations() {
    check(
        &wrap_in_callable("let a = (a) => a + 1;", CallableKind::Function),
        &expect![[r#"
            [
                SrcLint {
                    source: "",
                    level: Allow,
                    message: "operation does not contain any quantum operations",
                    help: "this callable can be declared as a function instead",
                    code_action_edits: [],
                },
            ]
        "#]],
    );
}

#[test]
fn needless_operation_no_lint_for_valid_lambda_operations() {
    check(
        &wrap_in_callable("let op = (q) => H(q);", CallableKind::Function),
        &expect![[r"
            []
        "]],
    );
}

#[test]
fn needless_operation_non_empty_op_and_no_specialization() {
    check(
        &wrap_in_callable("let x = 2;", CallableKind::Operation),
        &expect![[r#"
            [
                SrcLint {
                    source: "RunProgram",
                    level: Allow,
                    message: "operation does not contain any quantum operations",
                    help: "this callable can be declared as a function instead",
                    code_action_edits: [],
                },
            ]
        "#]],
    );
}

#[test]
fn needless_operation_non_empty_op_and_specialization() {
    check(
        indoc! {"
        operation Run(target : Qubit) : Unit is Adj {
            body ... {
                Message(\"hi\");
            }
            adjoint self;
        }
    "},
        &expect![[r#"
            [
                SrcLint {
                    source: "Run",
                    level: Allow,
                    message: "operation does not contain any quantum operations",
                    help: "this callable can be declared as a function instead",
                    code_action_edits: [],
                },
            ]
        "#]],
    );
}

#[test]
fn needless_operation_no_lint_for_empty_op_explicit_specialization() {
    check(
        indoc! {"
        operation I(target : Qubit) : Unit {
            body ... {}
            adjoint self;
        }

    "},
        &expect![[r"
            []
        "]],
    );
}

#[test]
fn needless_operation_no_lint_for_empty_op_implicit_specialization() {
    check(
        indoc! {"
        operation DoNothing() : Unit is Adj + Ctl {}
    "},
        &expect![[r"
            []
        "]],
    );
}

#[test]
fn needless_operation_partial_application() {
    check(
        indoc! {"
        operation PrepareBellState(q1 : Qubit, q2 : Qubit) : Unit {
            H(q1);
            CNOT(q1, q2);
        }

        operation PartialApplication(q1 : Qubit) : Qubit => Unit {
            return PrepareBellState(q1, _);
        }
    "},
        &expect![[r#"
            [
                SrcLint {
                    source: "PartialApplication",
                    level: Allow,
                    message: "operation does not contain any quantum operations",
                    help: "this callable can be declared as a function instead",
                    code_action_edits: [],
                },
            ]
        "#]],
    );
}
#[test]
fn deprecated_newtype_usage() {
    check(
        indoc! {"
        newtype Foo = ();
    "},
        &expect![[r#"
            [
                SrcLint {
                    source: "newtype Foo = ();",
                    level: Allow,
                    message: "deprecated `newtype` declarations",
                    help: "`newtype` declarations are deprecated, use `struct` instead",
                    code_action_edits: [],
                },
            ]
        "#]],
    );
}

#[test]
fn deprecated_function_cons() {
    check(
        indoc! {"
        struct Foo {}
        function Bar() : Foo { Foo() }
    "},
        &expect![[r#"
            [
                SrcLint {
                    source: "Foo",
                    level: Allow,
                    message: "deprecated function constructors",
                    help: "function constructors for struct types are deprecated, use `new` instead",
                    code_action_edits: [],
                },
            ]
        "#]],
    );
}

#[test]
fn deprecated_with_op_for_structs() {
    check(
        indoc! {"
        struct Foo { x : Int }
        function Bar() : Foo {
            let foo = new Foo { x = 2 };
            foo w/ x <- 3
        }
    "},
        &expect![[r#"
            [
                SrcLint {
                    source: "foo w/ x <- 3",
                    level: Allow,
                    message: "deprecated `w/` and `w/=` operators for structs",
                    help: "`w/` and `w/=` operators for structs are deprecated, use `new` instead",
                    code_action_edits: [
                        (
                            "new Foo {\n        ...foo,\n        x = 3,\n    }",
                            Span {
                                lo: 111,
                                hi: 124,
                            },
                        ),
                    ],
                },
            ]
        "#]],
    );
}

#[test]
fn deprecated_with_eq_op_for_structs() {
    check(
        indoc! {"
        struct Foo { x : Int }
        function Bar() : Foo {
            mutable foo = new Foo { x = 2 };
            set foo w/= x <- 3;
            foo
        }
    "},
        &expect![[r#"
            [
                SrcLint {
                    source: "set foo w/= x <- 3",
                    level: Allow,
                    message: "deprecated `w/` and `w/=` operators for structs",
                    help: "`w/` and `w/=` operators for structs are deprecated, use `new` instead",
                    code_action_edits: [
                        (
                            "set foo = new Foo {\n        ...foo,\n        x = 3,\n    }",
                            Span {
                                lo: 115,
                                hi: 133,
                            },
                        ),
                    ],
                },
            ]
        "#]],
    );
}

#[test]
fn deprecated_double_colon_op() {
    check(
        indoc! {"
        struct A { b : B }
        struct B { c : C }
        struct C { i : Int }
        function Bar(a : A) : Unit {
            a::b.c::i
        }
    "},
        &expect![[r#"
            [
                SrcLint {
                    source: "a::b.c::i",
                    level: Allow,
                    message: "deprecated `::` for field access",
                    help: "`::` operator is deprecated, use `.` instead",
                    code_action_edits: [
                        (
                            ".",
                            Span {
                                lo: 126,
                                hi: 128,
                            },
                        ),
                        (
                            ".",
                            Span {
                                lo: 121,
                                hi: 123,
                            },
                        ),
                    ],
                },
            ]
        "#]],
    );
}

#[test]
fn deprecated_double_colon_op_with_spacing() {
    check(
        indoc! {"
        struct A { b : B }
        struct B { c : C }
        struct C { i : Int }
        function Bar(a : A) : Unit {
            a  ::  b.c
            ::
            i
        }
    "},
        &expect![[r#"
            [
                SrcLint {
                    source: "a  ::  b.c\n    ::\n    i",
                    level: Allow,
                    message: "deprecated `::` for field access",
                    help: "`::` operator is deprecated, use `.` instead",
                    code_action_edits: [
                        (
                            ".",
                            Span {
                                lo: 135,
                                hi: 137,
                            },
                        ),
                        (
                            ".",
                            Span {
                                lo: 123,
                                hi: 125,
                            },
                        ),
                    ],
                },
            ]
        "#]],
    );
}

#[test]
fn needless_operation_inside_function_call() {
    check(
        indoc! {"
    operation Main() : Unit {
        Wrapper(A());
    }

    function Wrapper(_: Unit) : Unit {}

    operation A() : Unit {
        use q = Qubit();
        M(q);
    }
    "},
        &expect![[r"
            []
        "]],
    );
}

fn check(source: &str, expected: &Expect) {
    let source = wrap_in_namespace(source);
    let mut store = PackageStore::new(compile::core());
    let std = store.insert(compile::std(&store, TargetCapabilityFlags::all()));
    let sources = SourceMap::new([("source.qs".into(), source.clone().into())], None);
    let (unit, _) = qsc::compile::compile(
        &store,
        &[(std, None)],
        sources,
        PackageType::Exe,
        TargetCapabilityFlags::all(),
        LanguageFeatures::default(),
    );

    let id = store.insert(unit);
    let unit = store.get(id).expect("user package should exist");

    let actual: Vec<SrcLint> = run_lints(&store, unit, None)
        .into_iter()
        .map(|lint| SrcLint::from(&lint, &source))
        .collect();

    expected.assert_debug_eq(&actual);
}

/// Wraps some source code into a namespace, to make testing easier.
fn wrap_in_namespace(source: &str) -> String {
    format!(
        "namespace Foo {{
            {source}
        }}"
    )
}

fn wrap_in_callable(source: &str, callable_type: CallableKind) -> String {
    format!(
        "{callable_type} RunProgram() : Unit {{
            {source}
        }}"
    )
}

/// A version of Lint that replaces the span by source code
/// to make unit tests easier to write and verify.
#[allow(dead_code)]
#[derive(Debug)]
struct SrcLint {
    source: String,
    level: LintLevel,
    message: &'static str,
    help: &'static str,
    code_action_edits: Vec<(String, Span)>,
}

impl SrcLint {
    fn from(lint: &Lint, source: &str) -> Self {
        Self {
            source: source[lint.span].into(),
            level: lint.level,
            message: lint.message,
            help: lint.help,
            code_action_edits: lint
                .code_action_edits
                .iter()
                .map(|(edit, span)| (edit.clone(), *span))
                .collect(),
        }
    }
}

fn run_lints(
    package_store: &PackageStore,
    compile_unit: &CompileUnit,
    config: Option<&[LintConfig]>,
) -> Vec<Lint> {
    let compilation = Compilation {
        package_store,
        compile_unit,
    };

    let mut ast_lints = run_ast_lints(&compile_unit.ast.package, config);
    let mut hir_lints = run_hir_lints(&compile_unit.package, config, compilation);
    let mut lints = Vec::new();
    lints.append(&mut ast_lints);
    lints.append(&mut hir_lints);
    lints
}
