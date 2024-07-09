// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.
use crate::{compile, LanguageFeatures, TargetCapabilityFlags};
use expect_test::expect;
use qsc_frontend::compile::{CompileUnit, SourceMap};
use qsc_passes::PackageType;
use qsc_project::{PackageInfo, Project};
use rustc_hash::FxHashMap;
use std::sync::Arc;

fn mock_program() -> Project {
    Project {
        // Mock data for the ProgramConfig
        package_graph_sources: qsc_project::PackageGraphSources {
            root: qsc_project::PackageInfo {
                sources: vec![(
                    Arc::from("test"),
                    Arc::from("@EntryPoint() operation Main() : Unit {}"),
                )],
                language_features: LanguageFeatures::default(),
                dependencies: FxHashMap::from_iter(vec![(
                    Arc::from("SomeLibraryAlias"),
                    Arc::from("SomeLibraryKey"),
                )]),
                package_type: Some(qsc_project::PackageType::Exe),
            },
            packages: FxHashMap::from_iter(vec![(
                Arc::from("SomeLibraryKey"),
                PackageInfo {
                    sources: vec![(
                        Arc::from("librarymain"),
                        Arc::from("operation LibraryMain() : Unit {} export LibraryMain;"),
                    )],
                    language_features: LanguageFeatures::default(),
                    dependencies: FxHashMap::default(),
                    package_type: Some(qsc_project::PackageType::Lib),
                },
            )]),
        },
        lints: vec![],
        errors: vec![],
        path: "project/qsharp.json".into(),
        name: "project".into(),
    }
}

#[test]
fn test_prepare_package_store() {
    let program = mock_program();
    let buildable_program = super::prepare_package_store(
        TargetCapabilityFlags::default(),
        program.package_graph_sources,
    );

    expect![[r"
            []
        "]]
    .assert_debug_eq(&buildable_program.dependency_errors);

    // compile the user code
    let compiled = compile::compile(
        &buildable_program.store,
        &buildable_program.user_code_dependencies[..],
        SourceMap::new(buildable_program.user_code.sources, None),
        PackageType::Exe,
        TargetCapabilityFlags::default(),
        LanguageFeatures::default(),
    );

    let CompileUnit {
        package,
        ast,
        errors,
        ..
    } = compiled.0;

    expect![[r#"
            Package:
                entry expression: Expr 8 [0-0] [Type Unit]: Call:
                    Expr 7 [24-28] [Type Unit]: Var: Item 1
                    Expr 6 [28-30] [Type Unit]: Unit
                Item 0 [0-40] (Public):
                    Namespace (Ident 5 [0-40] "test"): Item 1
                Item 1 [0-40] (Internal):
                    Parent: 0
                    EntryPoint
                    Callable 0 [14-40] (operation):
                        name: Ident 1 [24-28] "Main"
                        input: Pat 2 [28-30] [Type Unit]: Unit
                        output: Unit
                        functors: empty set
                        body: SpecDecl 3 [14-40]: Impl:
                            Block 4 [38-40]: <empty>
                        adj: <none>
                        ctl: <none>
                        ctl-adj: <none>"#]]
    .assert_eq(&package.to_string());
    expect![[r#"
            Package 0:
                Namespace 1 [0-40] (Ident 2 [0-40] "test"):
                    Item 3 [0-40]:
                        Attr 4 [0-13] (Ident 5 [1-11] "EntryPoint"):
                            Expr 6 [11-13]: Unit
                        Callable 7 [14-40] (Operation):
                            name: Ident 8 [24-28] "Main"
                            input: Pat 9 [28-30]: Unit
                            output: Type 10 [33-37]: Path: Path 11 [33-37] (Ident 12 [33-37] "Unit")
                            body: Block: Block 13 [38-40]: <empty>"#]]
    .assert_eq(&ast.package.to_string());
    expect![[r"
            []
        "]]
    .assert_debug_eq(&errors);
}

// if there are inconsequential errors in the dependency compilation process, we don't want to
// abort compilation. This way, we can still show the user some diagnostics.

#[test]
fn missing_dependency_doesnt_force_failure() {
    let mut program = mock_program();
    program
        .package_graph_sources
        .root
        .dependencies
        .insert("NonExistent".into(), "nonexistent-dep-key".into());

    let buildable_program = super::prepare_package_store(
        TargetCapabilityFlags::default(),
        program.package_graph_sources,
    );

    expect![[r"
            []
        "]]
    .assert_debug_eq(&buildable_program.dependency_errors);

    // compile the user code
    let compiled = compile::compile(
        &buildable_program.store,
        &buildable_program.user_code_dependencies[..],
        SourceMap::new(buildable_program.user_code.sources, None),
        PackageType::Exe,
        TargetCapabilityFlags::default(),
        LanguageFeatures::default(),
    );

    let CompileUnit {
        package,
        ast,
        errors,
        ..
    } = compiled.0;

    expect![[r#"
            Package:
                entry expression: Expr 8 [0-0] [Type Unit]: Call:
                    Expr 7 [24-28] [Type Unit]: Var: Item 1
                    Expr 6 [28-30] [Type Unit]: Unit
                Item 0 [0-40] (Public):
                    Namespace (Ident 5 [0-40] "test"): Item 1
                Item 1 [0-40] (Internal):
                    Parent: 0
                    EntryPoint
                    Callable 0 [14-40] (operation):
                        name: Ident 1 [24-28] "Main"
                        input: Pat 2 [28-30] [Type Unit]: Unit
                        output: Unit
                        functors: empty set
                        body: SpecDecl 3 [14-40]: Impl:
                            Block 4 [38-40]: <empty>
                        adj: <none>
                        ctl: <none>
                        ctl-adj: <none>"#]]
    .assert_eq(&package.to_string());
    expect![[r#"
            Package 0:
                Namespace 1 [0-40] (Ident 2 [0-40] "test"):
                    Item 3 [0-40]:
                        Attr 4 [0-13] (Ident 5 [1-11] "EntryPoint"):
                            Expr 6 [11-13]: Unit
                        Callable 7 [14-40] (Operation):
                            name: Ident 8 [24-28] "Main"
                            input: Pat 9 [28-30]: Unit
                            output: Type 10 [33-37]: Path: Path 11 [33-37] (Ident 12 [33-37] "Unit")
                            body: Block: Block 13 [38-40]: <empty>"#]]
    .assert_eq(&ast.package.to_string());
    expect![[r"
            []
        "]]
    .assert_debug_eq(&errors);
}

#[test]
fn dependency_error() {
    let mut program = mock_program();
    // Inject a syntax error into one of the dependencies
    program
        .package_graph_sources
        .packages
        .values_mut()
        .next()
        .expect("expected at least one dependency in the mock program")
        .sources[0]
        .1 = "broken_syntax".into();

    let buildable_program = super::prepare_package_store(
        TargetCapabilityFlags::default(),
        program.package_graph_sources,
    );

    expect![[r#"
            [
                WithSource {
                    sources: [
                        Source {
                            name: "librarymain",
                            contents: "broken_syntax",
                            offset: 0,
                        },
                    ],
                    error: Frontend(
                        Error(
                            Parse(
                                Error(
                                    ExpectedItem(
                                        Ident,
                                        Span {
                                            lo: 0,
                                            hi: 0,
                                        },
                                    ),
                                ),
                            ),
                        ),
                    ),
                },
            ]
        "#]]
    .assert_debug_eq(&buildable_program.dependency_errors);

    // compile the user code
    let compiled = compile::compile(
        &buildable_program.store,
        &buildable_program.user_code_dependencies[..],
        SourceMap::new(buildable_program.user_code.sources, None),
        PackageType::Exe,
        TargetCapabilityFlags::default(),
        LanguageFeatures::default(),
    );

    let CompileUnit {
        package,
        ast,
        errors,
        ..
    } = compiled.0;

    expect![[r#"
            Package:
                entry expression: Expr 8 [0-0] [Type Unit]: Call:
                    Expr 7 [24-28] [Type Unit]: Var: Item 1
                    Expr 6 [28-30] [Type Unit]: Unit
                Item 0 [0-40] (Public):
                    Namespace (Ident 5 [0-40] "test"): Item 1
                Item 1 [0-40] (Internal):
                    Parent: 0
                    EntryPoint
                    Callable 0 [14-40] (operation):
                        name: Ident 1 [24-28] "Main"
                        input: Pat 2 [28-30] [Type Unit]: Unit
                        output: Unit
                        functors: empty set
                        body: SpecDecl 3 [14-40]: Impl:
                            Block 4 [38-40]: <empty>
                        adj: <none>
                        ctl: <none>
                        ctl-adj: <none>"#]]
    .assert_eq(&package.to_string());
    expect![[r#"
            Package 0:
                Namespace 1 [0-40] (Ident 2 [0-40] "test"):
                    Item 3 [0-40]:
                        Attr 4 [0-13] (Ident 5 [1-11] "EntryPoint"):
                            Expr 6 [11-13]: Unit
                        Callable 7 [14-40] (Operation):
                            name: Ident 8 [24-28] "Main"
                            input: Pat 9 [28-30]: Unit
                            output: Type 10 [33-37]: Path: Path 11 [33-37] (Ident 12 [33-37] "Unit")
                            body: Block: Block 13 [38-40]: <empty>"#]]
    .assert_eq(&ast.package.to_string());
    expect![[r"
            []
        "]]
    .assert_debug_eq(&errors);
}
