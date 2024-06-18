// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#![allow(clippy::needless_raw_string_hashes)]

use super::{Error, Locals, Names, Res};
use crate::{
    compile,
    resolve::{LocalKind, Resolver},
};
use expect_test::{expect, Expect};
use indoc::indoc;
use qsc_ast::ast::{Idents, Item, ItemKind};
use qsc_ast::{
    assigner::Assigner as AstAssigner,
    ast::{Ident, NodeId, Package, Path, TopLevelNode},
    mut_visit::MutVisitor,
    visit::{self, Visitor},
};

use qsc_data_structures::{
    language_features::LanguageFeatures,
    namespaces::{NamespaceId, NamespaceTreeRoot},
    span::Span,
    target::TargetCapabilityFlags,
};
use qsc_hir::assigner::Assigner as HirAssigner;
use rustc_hash::FxHashMap;
use std::fmt::Write;
use std::rc::Rc;

#[derive(Debug)]
enum Change {
    Res(Res),
    NamespaceId(NamespaceId),
}

impl From<Res> for Change {
    fn from(res: Res) -> Self {
        Self::Res(res)
    }
}

impl From<NamespaceId> for Change {
    fn from(ns_id: NamespaceId) -> Self {
        Self::NamespaceId(ns_id)
    }
}

struct Renamer<'a> {
    names: &'a Names,
    changes: Vec<(Span, Change)>,
    namespaces: NamespaceTreeRoot,
    aliases: FxHashMap<Vec<Rc<str>>, NamespaceId>,
}

impl<'a> Renamer<'a> {
    fn new(names: &'a Names, namespaces: NamespaceTreeRoot) -> Self {
        Self {
            names,
            changes: Vec::new(),
            namespaces,
            aliases: FxHashMap::default(),
        }
    }

    fn rename(&self, input: &mut String) {
        for (span, change) in self.changes.iter().rev() {
            let name = match change {
                Change::Res(res) => Self::format_res(res),
                Change::NamespaceId(ns_id) => format!("namespace{}", Into::<usize>::into(ns_id)),
            };
            input.replace_range((span.lo as usize)..(span.hi as usize), &name);
        }
    }

    fn format_res(res: &Res) -> String {
        match res {
            Res::Item(item, _) => match item.package {
                None => format!("item{}", item.item),
                Some(package) => format!("package{package}_item{}", item.item),
            },
            Res::Local(node) => format!("local{node}"),
            Res::PrimTy(prim) => format!("{prim:?}"),
            Res::UnitTy => "Unit".to_string(),
            Res::Param(id) => format!("param{id}"),
        }
    }
}

impl Visitor<'_> for Renamer<'_> {
    fn visit_path(&mut self, path: &Path) {
        if let Some(&id) = self.names.get(path.id) {
            self.changes.push((path.span, id.into()));
        } else {
            visit::walk_path(self, path);
        }
    }

    fn visit_ident(&mut self, ident: &Ident) {
        if let Some(&id) = self.names.get(ident.id) {
            self.changes.push((ident.span, id.into()));
        }
    }

    fn visit_item(&mut self, item: &'_ Item) {
        match &*item.kind {
            ItemKind::Open(namespace, Some(alias)) => {
                let Some(ns_id) = self.namespaces.get_namespace_id(namespace.str_iter()) else {
                    return;
                };
                self.aliases.insert(vec![alias.name.clone()], ns_id);
            }
            ItemKind::ImportOrExport(export) => {
                for item in export.items() {
                    if let Some(resolved_id) = self.names.get(item.path.id) {
                        self.changes.push((item.span(), (*resolved_id).into()));
                    } else if let Some(namespace_id) = self
                        .namespaces
                        .get_namespace_id(Into::<Idents>::into(item.clone().path).str_iter())
                    {
                        self.changes.push((item.span(), namespace_id.into()));
                    }
                }
                return;
            }
            _ => (),
        }
        visit::walk_item(self, item);
    }

    fn visit_idents(&mut self, vec_ident: &Idents) {
        let ns_id = match self.namespaces.get_namespace_id(vec_ident.str_iter()) {
            Some(x) => x,
            None => match self
                .aliases
                .get(&(Into::<Vec<Rc<str>>>::into(vec_ident)))
                .copied()
            {
                Some(x) => x,
                None => return,
            },
        };
        self.changes.push((vec_ident.span(), ns_id.into()));
    }
}

fn check(input: &str, expect: &Expect) {
    expect.assert_eq(&resolve_names(input, TargetCapabilityFlags::all()));
}

fn check_with_capabilities(input: &str, capabilities: TargetCapabilityFlags, expect: &Expect) {
    expect.assert_eq(&resolve_names(input, capabilities));
}

fn resolve_names(input: &str, capabilities: TargetCapabilityFlags) -> String {
    let (package, names, _, errors, namespaces) =
        compile(input, LanguageFeatures::default(), capabilities);
    let mut renamer = Renamer::new(&names, namespaces);
    renamer.visit_package(&package);
    let mut output = input.to_string();
    renamer.rename(&mut output);
    if !errors.is_empty() {
        output += "\n";
    }
    for error in &errors {
        writeln!(output, "// {error:?}").expect("string should be writable");
    }
    output
}

fn compile(
    input: &str,
    language_features: LanguageFeatures,
    capabilities: TargetCapabilityFlags,
) -> (Package, Names, Locals, Vec<Error>, NamespaceTreeRoot) {
    let (namespaces, parse_errors) = qsc_parse::namespaces(input, None, language_features);
    assert!(parse_errors.is_empty(), "parse failed: {parse_errors:#?}");
    let mut package = Package {
        id: NodeId::default(),
        nodes: namespaces
            .into_iter()
            .map(TopLevelNode::Namespace)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        entry: None,
    };

    AstAssigner::new().visit_package(&mut package);

    let mut cond_compile = compile::preprocess::Conditional::new(capabilities);
    cond_compile.visit_package(&mut package);
    let dropped_names = cond_compile.into_names();

    let mut assigner = HirAssigner::new();
    let mut globals = super::GlobalTable::new();
    let mut errors = globals.add_local_package(&mut assigner, &package);
    let mut resolver = Resolver::new(globals, dropped_names);
    resolver.bind_and_resolve_imports_and_exports(&package);
    resolver.with(&mut assigner).visit_package(&package);
    let (names, locals, mut resolve_errors, namespaces) = resolver.into_result();
    errors.append(&mut resolve_errors);
    (package, names, locals, errors, namespaces)
}

#[test]
fn global_callable() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {}

                function B() : Unit {
                    A();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}

                function item2() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn global_callable_recursive() {
    check(
        indoc! {
            "namespace Foo {
                function A() : Unit {
                    A();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn global_callable_internal() {
    check(
        indoc! {"
            namespace Foo {
                internal function A() : Unit {}

                function B() : Unit {
                    A();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                internal function item1() : Unit {}

                function item2() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn global_callable_duplicate_error() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {}
                operation A() : Unit {}
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}
                operation item2() : Unit {}
            }

            // Duplicate("A", "Foo", Span { lo: 57, hi: 58 })
        "#]],
    );
}

#[test]
fn global_path() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {}
            }

            namespace Bar {
                function B() : Unit {
                    Foo.A();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}
            }

            namespace namespace8 {
                function item3() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn open_namespace() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {}
            }

            namespace Bar {
                open Foo;

                function B() : Unit {
                    A();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}
            }

            namespace namespace8 {
                open namespace7;

                function item3() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn open_alias() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {}
            }

            namespace Bar {
                open Foo as F;

                function B() : Unit {
                    F.A();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}
            }

            namespace namespace8 {
                open namespace7 as F;

                function item3() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn prelude_callable() {
    check(
        indoc! {"
            namespace Microsoft.Quantum.Core {
                function A() : Unit {}
            }

            namespace Foo {
                function B() : Unit {
                    A();
                }
            }
        "},
        &expect![[r#"
            namespace namespace4 {
                function item1() : Unit {}
            }

            namespace namespace7 {
                function item3() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn parent_namespace_shadows_prelude() {
    check(
        indoc! {"
            namespace Microsoft.Quantum.Core {
                function A() : Unit {}
            }

            namespace Foo {
                function A() : Unit {}

                function B() : Unit {
                    A();
                }
            }
        "},
        &expect![[r#"
            namespace namespace4 {
                function item1() : Unit {}
            }

            namespace namespace7 {
                function item3() : Unit {}

                function item4() : Unit {
                    item3();
                }
            }
        "#]],
    );
}

#[test]
fn open_shadows_prelude() {
    check(
        indoc! {"
            namespace Microsoft.Quantum.Core {
                function A() : Unit {}
            }

            namespace Foo {
                function A() : Unit {}
            }

            namespace Bar {
                open Foo;

                function B() : Unit {
                    A();
                }
            }
        "},
        &expect![[r#"
            namespace namespace4 {
                function item1() : Unit {}
            }

            namespace namespace7 {
                function item3() : Unit {}
            }

            namespace namespace8 {
                open namespace7;

                function item5() : Unit {
                    item3();
                }
            }
        "#]],
    );
}

#[test]
fn ambiguous_prelude() {
    check(
        indoc! {"
        namespace Microsoft.Quantum.Canon {
            function A() : Unit {}
        }

        namespace Microsoft.Quantum.Core {
            function A() : Unit {}
        }

        namespace Foo {
            function B() : Unit {
                A();
            }
        }
        "},
        &expect![[r#"
            namespace namespace3 {
                function item1() : Unit {}
            }

            namespace namespace4 {
                function item3() : Unit {}
            }

            namespace namespace7 {
                function item5() : Unit {
                    A();
                }
            }

            // AmbiguousPrelude { name: "A", candidate_a: "Microsoft.Quantum.Canon", candidate_b: "Microsoft.Quantum.Core", span: Span { lo: 181, hi: 182 } }
        "#]],
    );
}

#[test]
fn local_var() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Int {
                    let x = 0;
                    x
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Int {
                    let local13 = 0;
                    local13
                }
            }
        "#]],
    );
}

#[test]
fn shadow_local() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Int {
                    let x = 0;
                    let y = {
                        let x = 1;
                        x
                    };
                    x + y
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Int {
                    let local13 = 0;
                    let local17 = {
                        let local22 = 1;
                        local22
                    };
                    local13 + local17
                }
            }
        "#]],
    );
}

#[test]
fn callable_param() {
    check(
        indoc! {"
            namespace Foo {
                function A(x : Int) : Int {
                    x
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1(local8 : Int) : Int {
                    local8
                }
            }
        "#]],
    );
}

#[test]
fn spec_param() {
    check(
        indoc! {"
            namespace Foo {
                operation A(q : Qubit) : (Qubit[], Qubit) {
                    controlled (cs, ...) {
                        (cs, q)
                    }
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1(local8 : Qubit) : (Qubit[], Qubit) {
                    controlled (local23, ...) {
                        (local23, local8)
                    }
                }
            }
        "#]],
    );
}

#[test]
fn spec_param_shadow_disallowed() {
    check(
        indoc! {"
            namespace Foo {
                operation A(qs : Qubit[]) : Qubit[] {
                    controlled (qs, ...) {
                        qs
                    }
                    body ... {
                        qs
                    }
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1(local8 : Qubit[]) : Qubit[] {
                    controlled (local20, ...) {
                        local20
                    }
                    body ... {
                        local8
                    }
                }
            }

            // DuplicateBinding("qs", Span { lo: 78, hi: 80 })
        "#]],
    );
}

#[test]
fn local_shadows_global() {
    check(
        indoc! {"
            namespace Foo {
                function x() : Unit {}

                function y() : Int {
                    x();
                    let x = 1;
                    x
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}

                function item2() : Int {
                    item1();
                    let local27 = 1;
                    local27
                }
            }
        "#]],
    );
}

#[test]
fn shadow_same_block() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Int {
                    let x = 0;
                    let x = x + 1;
                    x
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Int {
                    let local13 = 0;
                    let local17 = local13 + 1;
                    local17
                }
            }
        "#]],
    );
}

#[test]
fn parent_namespace_shadows_open() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {}
            }

            namespace Bar {
                open Foo;

                function A() : Unit {}

                function B() : Unit {
                    A();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}
            }

            namespace namespace8 {
                open namespace7;

                function item3() : Unit {}

                function item4() : Unit {
                    item3();
                }
            }
        "#]],
    );
}

#[test]
fn open_alias_shadows_global() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {}
            }

            namespace Bar {
                function A() : Unit {}
            }

            namespace Baz {
                open Foo as Bar;

                function B() : Unit {
                    Bar.A();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}
            }

            namespace namespace8 {
                function item3() : Unit {}
            }

            namespace namespace9 {
                open namespace7 as Bar;

                function item5() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn shadowing_disallowed_within_parameters() {
    check(
        indoc! {"
            namespace Test {
                operation Foo(x: Int, y: Double, x: Bool) : Unit {}
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1(local8: Int, local13: Double, local18: Bool) : Unit {}
            }

            // DuplicateBinding("x", Span { lo: 54, hi: 55 })
        "#]],
    );
}

#[test]
fn shadowing_disallowed_within_local_binding() {
    check(
        indoc! {"
            namespace Test {
                operation Foo() : Unit {
                    let (first, second, first) = (1, 2, 3);
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {
                    let (local14, local16, local18) = (1, 2, 3);
                }
            }

            // DuplicateBinding("first", Span { lo: 74, hi: 79 })
        "#]],
    );
}

#[test]
fn shadowing_disallowed_within_for_loop() {
    check(
        indoc! {"
            namespace Test {
                operation Foo() : Unit {
                    for (key, val, key) in [(1, 1, 1)] {}
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {
                    for (local15, local17, local19) in [(1, 1, 1)] {}
                }
            }

            // DuplicateBinding("key", Span { lo: 69, hi: 72 })
        "#]],
    );
}

#[test]
fn shadowing_disallowed_within_lambda_param() {
    check(
        indoc! {"
            namespace Test {
                operation Foo() : Unit {
                    let f = (x, y, x) -> x + y + 1;
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {
                    let local13 = (local17, local19, local21) -> local21 + local19 + 1;
                }
            }

            // DuplicateBinding("x", Span { lo: 69, hi: 70 })
        "#]],
    );
}

#[test]
fn merged_aliases() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {}
            }

            namespace Bar {
                function B() : Unit {}
            }

            namespace Baz {
                open Foo as Alias;
                open Bar as Alias;

                function C() : Unit {
                    Alias.A();
                    Alias.B();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}
            }

            namespace namespace8 {
                function item3() : Unit {}
            }

            namespace namespace9 {
                open namespace7 as Alias;
                open namespace8 as Alias;

                function item5() : Unit {
                    item1();
                    item3();
                }
            }
        "#]],
    );
}

#[test]
fn ty_decl() {
    check(
        indoc! {"
            namespace Foo {
                newtype A = Unit;
                function B(a : A) : Unit {}
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                newtype item1 = Unit;
                function item2(local14 : item1) : Unit {}
            }
        "#]],
    );
}

#[test]
fn struct_decl() {
    check(
        indoc! {"
            namespace Foo {
                struct A {}
                function B(a : A) : Unit {}
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                struct item1 {}
                function item2(local11 : item1) : Unit {}
            }
        "#]],
    );
}

#[test]
fn ty_decl_duplicate_error() {
    check(
        indoc! {"
            namespace Foo {
                newtype A = Unit;
                newtype A = Bool;
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                newtype item1 = Unit;
                newtype item2 = Bool;
            }

            // Duplicate("A", "Foo", Span { lo: 50, hi: 51 })
        "#]],
    );
}

#[test]
fn struct_decl_duplicate_error() {
    check(
        indoc! {"
            namespace Foo {
                struct A {}
                struct A { first : Bool }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                struct item1 {}
                struct item2 { first : Bool }
            }

            // Duplicate("A", "Foo", Span { lo: 43, hi: 44 })
        "#]],
    );
}

#[test]
fn ty_decl_duplicate_error_on_built_in_ty() {
    check(
        indoc! {"
            namespace Microsoft.Quantum.Core {
                newtype Pauli = Unit;
            }
        "},
        &expect![[r#"
            namespace namespace4 {
                newtype item1 = Unit;
            }

            // Duplicate("Pauli", "Microsoft.Quantum.Core", Span { lo: 47, hi: 52 })
        "#]],
    );
}

#[test]
fn struct_decl_duplicate_error_on_built_in_ty() {
    check(
        indoc! {"
            namespace Microsoft.Quantum.Core {
                struct Pauli {}
            }
        "},
        &expect![[r#"
            namespace namespace4 {
                struct item1 {}
            }

            // Duplicate("Pauli", "Microsoft.Quantum.Core", Span { lo: 46, hi: 51 })
        "#]],
    );
}

#[test]
fn ty_decl_in_ty_decl() {
    check(
        indoc! {"
            namespace Foo {
                newtype A = Unit;
                newtype B = A;
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                newtype item1 = Unit;
                newtype item2 = item1;
            }
        "#]],
    );
}

#[test]
fn struct_decl_in_struct_decl() {
    check(
        indoc! {"
            namespace Foo {
                struct A {}
                struct B { a : A }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                struct item1 {}
                struct item2 { a : item1 }
            }
        "#]],
    );
}

#[test]
fn ty_decl_recursive() {
    check(
        indoc! {"
            namespace Foo {
                newtype A = A;
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                newtype item1 = item1;
            }
        "#]],
    );
}

#[test]
fn struct_decl_recursive() {
    check(
        indoc! {"
            namespace Foo {
                struct A { a : A }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                struct item1 { a : item1 }
            }
        "#]],
    );
}

#[test]
fn ty_decl_cons() {
    check(
        indoc! {"
            namespace Foo {
                newtype A = Unit;

                function B() : A {
                    A()
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                newtype item1 = Unit;

                function item2() : item1 {
                    item1()
                }
            }
        "#]],
    );
}

#[test]
fn struct_decl_call_cons() {
    check(
        indoc! {"
            namespace Foo {
                struct A {}

                function B() : A {
                    A()
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                struct item1 {}

                function item2() : item1 {
                    item1()
                }
            }
        "#]],
    );
}

#[test]
fn struct_decl_cons() {
    check(
        indoc! {"
            namespace Foo {
                struct A {}

                function B() : A {
                    new A {}
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                struct item1 {}

                function item2() : item1 {
                    new item1 {}
                }
            }
        "#]],
    );
}

#[test]
fn struct_decl_cons_with_fields() {
    check(
        indoc! {"
            namespace Foo {
                struct A {}
                struct B {}
                struct C { a : A, b : B }

                function D() : C {
                    new C { a = new A {}, b = new B {} }
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                struct item1 {}
                struct item2 {}
                struct item3 { a : item1, b : item2 }

                function item4() : item3 {
                    new item3 { a = new item1 {}, b = new item2 {} }
                }
            }
        "#]],
    );
}

#[test]
fn unknown_term() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {
                    B();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {
                    B();
                }
            }

            // NotFound("B", Span { lo: 50, hi: 51 })
        "#]],
    );
}

#[test]
fn unknown_ty() {
    check(
        indoc! {"
            namespace Foo {
                function A(b : B) : Unit {}
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1(local8 : B) : Unit {}
            }

            // NotFound("B", Span { lo: 35, hi: 36 })
        "#]],
    );
}

#[test]
fn open_ambiguous_terms() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {}
            }

            namespace Bar {
                function A() : Unit {}
            }

            namespace Baz {
                open Foo;
                open Bar;

                function C() : Unit {
                    A();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}
            }

            namespace namespace8 {
                function item3() : Unit {}
            }

            namespace namespace9 {
                open namespace7;
                open namespace8;

                function item5() : Unit {
                    A();
                }
            }

            // Ambiguous { name: "A", first_open: "Foo", second_open: "Bar", name_span: Span { lo: 171, hi: 172 }, first_open_span: Span { lo: 117, hi: 120 }, second_open_span: Span { lo: 131, hi: 134 } }
        "#]],
    );
}

#[test]
fn open_ambiguous_tys() {
    check(
        indoc! {"
            namespace Foo {
                newtype A = Unit;
            }

            namespace Bar {
                newtype A = Unit;
            }

            namespace Baz {
                open Foo;
                open Bar;

                function C(a : A) : Unit {}
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                newtype item1 = Unit;
            }

            namespace namespace8 {
                newtype item3 = Unit;
            }

            namespace namespace9 {
                open namespace7;
                open namespace8;

                function item5(local28 : A) : Unit {}
            }

            // Ambiguous { name: "A", first_open: "Foo", second_open: "Bar", name_span: Span { lo: 146, hi: 147 }, first_open_span: Span { lo: 107, hi: 110 }, second_open_span: Span { lo: 121, hi: 124 } }
        "#]],
    );
}

#[test]
fn merged_aliases_ambiguous_terms() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {}
            }

            namespace Bar {
                function A() : Unit {}
            }

            namespace Baz {
                open Foo as Alias;
                open Bar as Alias;

                function C() : Unit {
                    Alias.A();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}
            }

            namespace namespace8 {
                function item3() : Unit {}
            }

            namespace namespace9 {
                open namespace7 as Alias;
                open namespace8 as Alias;

                function item5() : Unit {
                    namespace8.A();
                }
            }

            // Ambiguous { name: "A", first_open: "Foo", second_open: "Bar", name_span: Span { lo: 195, hi: 196 }, first_open_span: Span { lo: 117, hi: 120 }, second_open_span: Span { lo: 140, hi: 143 } }
        "#]],
    );
}

#[test]
fn merged_aliases_ambiguous_tys() {
    check(
        indoc! {"
            namespace Foo {
                newtype A = Unit;
            }

            namespace Bar {
                newtype A = Unit;
            }

            namespace Baz {
                open Foo as Alias;
                open Bar as Alias;

                function C(a : Alias.A) : Unit {}
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                newtype item1 = Unit;
            }

            namespace namespace8 {
                newtype item3 = Unit;
            }

            namespace namespace9 {
                open namespace7 as Alias;
                open namespace8 as Alias;

                function item5(local30 : namespace8.A) : Unit {}
            }

            // Ambiguous { name: "A", first_open: "Foo", second_open: "Bar", name_span: Span { lo: 170, hi: 171 }, first_open_span: Span { lo: 107, hi: 110 }, second_open_span: Span { lo: 130, hi: 133 } }
        "#]],
    );
}

#[test]
fn lambda_param() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {
                    let f = x -> x + 1;
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {
                    let local13 = local16 -> local16 + 1;
                }
            }
        "#]],
    );
}

#[test]
fn lambda_shadows_local() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Int {
                    let x = 1;
                    let f = x -> x + 1;
                    x
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Int {
                    let local13 = 1;
                    let local17 = local20 -> local20 + 1;
                    local13
                }
            }
        "#]],
    );
}

#[test]
fn for_loop_range() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {
                    for i in 0..9 {
                        let _ = i;
                    }
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {
                    for local14 in 0..9 {
                        let _ = local14;
                    }
                }
            }
        "#]],
    );
}

#[test]
fn for_loop_var() {
    check(
        indoc! {"
            namespace Foo {
                function A(xs : Int[]) : Unit {
                    for x in xs {
                        let _ = x;
                    }
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1(local8 : Int[]) : Unit {
                    for local20 in local8 {
                        let _ = local20;
                    }
                }
            }
        "#]],
    );
}

#[test]
fn repeat_until() {
    check(
        indoc! {"
            namespace Foo {
                operation A() : Unit {
                    mutable cond = false;
                    repeat {
                        set cond = true;
                    } until cond;
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {
                    mutable local13 = false;
                    repeat {
                        set local13 = true;
                    } until local13;
                }
            }
        "#]],
    );
}

#[test]
fn repeat_until_fixup() {
    check(
        indoc! {"
            namespace Foo {
                operation A() : Unit {
                    mutable cond = false;
                    repeat {
                        set cond = false;
                    } until cond
                    fixup {
                        set cond = true;
                    }
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {
                    mutable local13 = false;
                    repeat {
                        set local13 = false;
                    } until local13
                    fixup {
                        set local13 = true;
                    }
                }
            }
        "#]],
    );
}

#[test]
fn repeat_until_fixup_scoping() {
    check(
        indoc! {"
        namespace Foo {
            operation A() : Unit {
                repeat {
                    mutable cond = false;
                }
                until cond
                fixup {
                    set cond = true;
                }
            }
        }"},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {
                    repeat {
                        mutable local16 = false;
                    }
                    until cond
                    fixup {
                        set cond = true;
                    }
                }
            }
            // NotFound("cond", Span { lo: 118, hi: 122 })
            // NotFound("cond", Span { lo: 155, hi: 159 })
        "#]],
    );
}

#[test]
fn use_qubit() {
    check(
        indoc! {"
            namespace Foo {
                operation X(q : Qubit) : Unit {
                    body intrinsic;
                }
                operation A() : Unit {
                    use q = Qubit();
                    X(q);
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1(local8 : Qubit) : Unit {
                    body intrinsic;
                }
                operation item2() : Unit {
                    use local26 = Qubit();
                    item1(local26);
                }
            }
        "#]],
    );
}

#[test]
fn use_qubit_block() {
    check(
        indoc! {"
            namespace Foo {
                operation X(q : Qubit) : Unit {
                    body intrinsic;
                }
                operation A() : Unit {
                    use q = Qubit() {
                        X(q);
                    }
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1(local8 : Qubit) : Unit {
                    body intrinsic;
                }
                operation item2() : Unit {
                    use local26 = Qubit() {
                        item1(local26);
                    }
                }
            }
        "#]],
    );
}

#[test]
fn use_qubit_block_qubit_restricted_to_block_scope() {
    check(
        indoc! {"
            namespace Foo {
                operation X(q : Qubit) : Unit {
                    body intrinsic;
                }
                operation A() : Unit {
                    use q = Qubit() {
                        X(q);
                    }
                    X(q);
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1(local8 : Qubit) : Unit {
                    body intrinsic;
                }
                operation item2() : Unit {
                    use local26 = Qubit() {
                        item1(local26);
                    }
                    item1(q);
                }
            }

            // NotFound("q", Span { lo: 173, hi: 174 })
        "#]],
    );
}

#[test]
fn local_function() {
    check(
        indoc! {"
            namespace A {
                function Foo() : Int {
                    function Bar() : Int { 2 }
                    Bar() + 1
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Int {
                    function item2() : Int { 2 }
                    item2() + 1
                }
            }
        "#]],
    );
}

#[test]
fn local_function_use_before_declare() {
    check(
        indoc! {"
            namespace A {
                function Foo() : () {
                    Bar();
                    function Bar() : () {}
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : () {
                    item2();
                    function item2() : () {}
                }
            }
        "#]],
    );
}

#[test]
fn local_function_is_really_local() {
    check(
        indoc! {"
            namespace A {
                function Foo() : () {
                    function Bar() : () {}
                    Bar();
                }

                function Baz() : () { Bar(); }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : () {
                    function item3() : () {}
                    item3();
                }

                function item2() : () { Bar(); }
            }

            // NotFound("Bar", Span { lo: 119, hi: 122 })
        "#]],
    );
}

#[test]
fn local_function_is_not_closure() {
    check(
        indoc! {"
            namespace A {
                function Foo() : () {
                    let x = 2;
                    function Bar() : Int { x }
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : () {
                    let local11 = 2;
                    function item2() : Int { x }
                }
            }

            // NotFound("x", Span { lo: 90, hi: 91 })
        "#]],
    );
}

#[test]
fn local_type() {
    check(
        indoc! {"
            namespace A {
                function Foo() : () {
                    newtype Bar = Int;
                    let x = Bar(5);
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : () {
                    newtype item2 = Int;
                    let local18 = item2(5);
                }
            }
        "#]],
    );
}

#[test]
fn local_open() {
    check(
        indoc! {"
            namespace A { function Foo() : () { open B; Bar(); } }
            namespace B { function Bar() : () {} }
        "},
        &expect![[r#"
            namespace namespace7 { function item1() : () { open namespace8; item3(); } }
            namespace namespace8 { function item3() : () {} }
        "#]],
    );
}

#[test]
fn local_open_shadows_parent_item() {
    check(
        indoc! {"
            namespace A {
                function Bar() : () {}
                function Foo() : () { open B; Bar(); }
            }

            namespace B { function Bar() : () {} }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : () {}
                function item2() : () { open namespace8; item4(); }
            }

            namespace namespace8 { function item4() : () {} }
        "#]],
    );
}

#[test]
fn local_open_shadows_parent_open() {
    check(
        indoc! {"
            namespace A {
                open B;
                function Foo() : () { open C; Bar(); }
            }

            namespace B { function Bar() : () {} }
            namespace C { function Bar() : () {} }
        "},
        &expect![[r#"
            namespace namespace7 {
                open namespace8;
                function item1() : () { open namespace9; item5(); }
            }

            namespace namespace8 { function item3() : () {} }
            namespace namespace9 { function item5() : () {} }
        "#]],
    );
}

#[test]
fn update_array_index_var() {
    check(
        indoc! {"
            namespace A {
                function Foo() : () {
                    let xs = [2];
                    let i = 0;
                    let ys = xs w/ i <- 3;
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : () {
                    let local11 = [2];
                    let local16 = 0;
                    let local20 = local11 w/ local16 <- 3;
                }
            }
        "#]],
    );
}

#[test]
fn update_array_index_expr() {
    check(
        indoc! {"
            namespace A {
                function Foo() : () {
                    let xs = [2];
                    let i = 0;
                    let ys = xs w/ i + 1 <- 3;
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : () {
                    let local11 = [2];
                    let local16 = 0;
                    let local20 = local11 w/ local16 + 1 <- 3;
                }
            }
        "#]],
    );
}

#[test]
fn update_udt_known_field_name() {
    check(
        indoc! {"
            namespace A {
                newtype Pair = (First : Int, Second : Int);

                function Foo() : () {
                    let p = Pair(1, 2);
                    let q = p w/ First <- 3;
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                newtype item1 = (First : Int, Second : Int);

                function item2() : () {
                    let local24 = item1(1, 2);
                    let local34 = local24 w/ First <- 3;
                }
            }
        "#]],
    );
}

#[test]
fn update_udt_known_field_name_expr() {
    check(
        indoc! {"
            namespace A {
                newtype Pair = (First : Int, Second : Int);

                function Foo() : () {
                    let p = Pair(1, 2);
                    let q = p w/ First + 1 <- 3;
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                newtype item1 = (First : Int, Second : Int);

                function item2() : () {
                    let local24 = item1(1, 2);
                    let local34 = local24 w/ First + 1 <- 3;
                }
            }

            // NotFound("First", Span { lo: 138, hi: 143 })
        "#]],
    );
}

#[test]
fn update_udt_unknown_field_name() {
    check(
        indoc! {"
            namespace A {
                newtype Pair = (First : Int, Second : Int);

                function Foo() : () {
                    let p = Pair(1, 2);
                    let q = p w/ Third <- 3;
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                newtype item1 = (First : Int, Second : Int);

                function item2() : () {
                    let local24 = item1(1, 2);
                    let local34 = local24 w/ Third <- 3;
                }
            }
        "#]],
    );
}

#[test]
fn update_udt_unknown_field_name_known_global() {
    check(
        indoc! {"
            namespace A {
                newtype Pair = (First : Int, Second : Int);

                function Third() : () {}

                function Foo() : () {
                    let p = Pair(1, 2);
                    let q = p w/ Third <- 3;
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                newtype item1 = (First : Int, Second : Int);

                function item2() : () {}

                function item3() : () {
                    let local30 = item1(1, 2);
                    let local40 = local30 w/ Third <- 3;
                }
            }
        "#]],
    );
}

#[test]
fn unknown_namespace() {
    check(
        indoc! {"
            namespace A {
                open Microsoft.Quantum.Fake;
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                open Microsoft.Quantum.Fake;
            }

            // NotFound("Microsoft.Quantum.Fake", Span { lo: 23, hi: 45 })
        "#]],
    );
}

#[test]
fn empty_namespace_works() {
    check(
        indoc! {"
            namespace A {
                open B;
                function foo(): Unit{}
            }
            namespace B {}
        "},
        &expect![[r#"
            namespace namespace7 {
                open namespace8;
                function item1(): Unit{}
            }
            namespace namespace8 {}
        "#]],
    );
}

#[test]
fn cyclic_namespace_dependency_supported() {
    check(
        indoc! {"
            namespace A {
                open B;
                operation Foo() : Unit {
                    Bar();
                }
            }
            namespace B {
                open A;
                operation Bar() : Unit {
                    Foo();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                open namespace8;
                operation item1() : Unit {
                    item3();
                }
            }
            namespace namespace8 {
                open namespace7;
                operation item3() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn bind_items_in_repeat() {
    check(
        indoc! {"
            namespace A {
                operation B() : Unit {
                    repeat {
                        function C() : Unit {}
                    } until false
                    fixup {
                        function D() : Unit {}
                    }
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {
                    repeat {
                        function item2() : Unit {}
                    } until false
                    fixup {
                        function item3() : Unit {}
                    }
                }
            }
        "#]],
    );
}

#[test]
fn bind_items_in_qubit_use_block() {
    check(
        indoc! {"
            namespace A {
                operation B() : Unit {
                    use q = Qubit() {
                        function C() : Unit {}
                    }
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {
                    use local13 = Qubit() {
                        function item2() : Unit {}
                    }
                }
            }
        "#]],
    );
}

#[test]
fn use_bound_item_in_another_bound_item() {
    check(
        indoc! {"
            namespace A {
                function B() : Unit {
                    function C() : Unit {
                        D();
                    }
                    function D() : Unit {}
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {
                    function item2() : Unit {
                        item3();
                    }
                    function item3() : Unit {}
                }
            }
        "#]],
    );
}

#[test]
fn use_unbound_generic() {
    check(
        indoc! {"
            namespace A {
                function B<'T>(x: 'U) : 'U {
                    x
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1<param0>(local9: 'U) : 'U {
                    local9
                }
            }

            // NotFound("'U", Span { lo: 36, hi: 38 })
            // NotFound("'U", Span { lo: 42, hi: 44 })
        "#]],
    );
}

#[test]
fn resolve_local_generic() {
    check(
        indoc! {"
            namespace A {
                function B<'T>(x: 'T) : 'T {
                    x
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1<param0>(local9: param0) : param0 {
                    local9
                }
            }
        "#]],
    );
}

#[test]
fn dropped_base_callable_from_unrestricted() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(Base)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::all(),
        &expect![[r#"
            namespace namespace7 {
                @Config(Base)
                function Dropped() : Unit {}

                function item1() : Unit {
                    Dropped();
                }
            }

            // NotAvailable("Dropped", "A.Dropped", Span { lo: 100, hi: 107 })
        "#]],
    );
}

#[test]
fn dropped_base_callable_from_adaptive() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(Base)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::Adaptive,
        &expect![[r#"
            namespace namespace7 {
                @Config(Base)
                function Dropped() : Unit {}

                function item1() : Unit {
                    Dropped();
                }
            }

            // NotAvailable("Dropped", "A.Dropped", Span { lo: 100, hi: 107 })
        "#]],
    );
}

#[test]
fn dropped_not_base_callable_from_base() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(not Base)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::empty(),
        &expect![[r#"
            namespace namespace7 {
                @Config(not Base)
                function Dropped() : Unit {}

                function item1() : Unit {
                    Dropped();
                }
            }

            // NotAvailable("Dropped", "A.Dropped", Span { lo: 104, hi: 111 })
        "#]],
    );
}

#[test]
fn resolved_not_base_callable_from_adaptive() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(not Base)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::Adaptive,
        &expect![[r#"
            namespace namespace7 {
                @Config(not Base)
                function item1() : Unit {}

                function item2() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn dropped_base_and_not_base_callable_from_base() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(Base)
                @Config(not Base)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::empty(),
        &expect![[r#"
            namespace namespace7 {
                @Config(Base)
                @Config(not Base)
                function Dropped() : Unit {}

                function item1() : Unit {
                    Dropped();
                }
            }

            // NotAvailable("Dropped", "A.Dropped", Span { lo: 122, hi: 129 })
        "#]],
    );
}

#[test]
fn resolved_not_unrestricted_callable_from_base() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(not Unrestricted)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::empty(),
        &expect![[r#"
            namespace namespace7 {
                @Config(not Unrestricted)
                function item1() : Unit {}

                function item2() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn resolved_not_unrestricted_callable_from_adaptive() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(not Unrestricted)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::Adaptive,
        &expect![[r#"
            namespace namespace7 {
                @Config(not Unrestricted)
                function item1() : Unit {}

                function item2() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn dropped_not_unrestricted_callable_from_unrestricted() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(not Unrestricted)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::all(),
        &expect![[r#"
            namespace namespace7 {
                @Config(not Unrestricted)
                function Dropped() : Unit {}

                function item1() : Unit {
                    Dropped();
                }
            }

            // NotAvailable("Dropped", "A.Dropped", Span { lo: 112, hi: 119 })
        "#]],
    );
}

#[test]
fn resolved_adaptive_callable_from_adaptive() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(Adaptive)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::Adaptive,
        &expect![[r#"
            namespace namespace7 {
                @Config(Adaptive)
                function item1() : Unit {}

                function item2() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn resolved_adaptive_callable_from_unrestricted() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(Adaptive)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::all(),
        &expect![[r#"
            namespace namespace7 {
                @Config(Adaptive)
                function item1() : Unit {}

                function item2() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn dropped_not_higher_level_callable_from_unrestricted() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(not HigherLevelConstructs)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::all(),
        &expect![[r#"
            namespace namespace7 {
                @Config(not HigherLevelConstructs)
                function Dropped() : Unit {}

                function item1() : Unit {
                    Dropped();
                }
            }

            // NotAvailable("Dropped", "A.Dropped", Span { lo: 121, hi: 128 })
        "#]],
    );
}

#[test]
fn resolved_not_higher_level_callable_from_adaptive() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(not HigherLevelConstructs)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::Adaptive,
        &expect![[r#"
            namespace namespace7 {
                @Config(not HigherLevelConstructs)
                function item1() : Unit {}

                function item2() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn resolved_not_higher_level_callable_from_base() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(not HigherLevelConstructs)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::empty(),
        &expect![[r#"
            namespace namespace7 {
                @Config(not HigherLevelConstructs)
                function item1() : Unit {}

                function item2() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn dropped_not_higher_level_and_adaptive_callable_from_base() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(Adaptive)
                @Config(not HigherLevelConstructs)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::empty(),
        &expect![[r#"
            namespace namespace7 {
                @Config(Adaptive)
                @Config(not HigherLevelConstructs)
                function Dropped() : Unit {}

                function item1() : Unit {
                    Dropped();
                }
            }

            // NotAvailable("Dropped", "A.Dropped", Span { lo: 143, hi: 150 })
        "#]],
    );
}

#[test]
fn dropped_not_higher_level_and_adaptive_callable_from_unrestricted() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(Adaptive)
                @Config(not HigherLevelConstructs)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::all(),
        &expect![[r#"
            namespace namespace7 {
                @Config(Adaptive)
                @Config(not HigherLevelConstructs)
                function Dropped() : Unit {}

                function item1() : Unit {
                    Dropped();
                }
            }

            // NotAvailable("Dropped", "A.Dropped", Span { lo: 143, hi: 150 })
        "#]],
    );
}

#[test]
fn resolved_not_higher_level_and_adaptive_callable_from_adaptive() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(Adaptive)
                @Config(not HigherLevelConstructs)
                function Dropped() : Unit {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::Adaptive,
        &expect![[r#"
            namespace namespace7 {
                @Config(Adaptive)
                @Config(not HigherLevelConstructs)
                function item1() : Unit {}

                function item2() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn dropped_floating_point_from_adaptive() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(FloatingPointComputations)
                function Dropped() : Double {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::Adaptive,
        &expect![[r#"
            namespace namespace7 {
                @Config(FloatingPointComputations)
                function Dropped() : Double {}

                function item1() : Unit {
                    Dropped();
                }
            }

            // NotAvailable("Dropped", "A.Dropped", Span { lo: 123, hi: 130 })
        "#]],
    );
}

#[test]
fn resolved_adaptive_and_integer_from_adaptive_and_integer() {
    check_with_capabilities(
        indoc! {"
            namespace A {
                @Config(Adaptive)
                @Config(IntegerComputations)
                function Dropped() : Double {}

                function B() : Unit {
                    Dropped();
                }
            }
        "},
        TargetCapabilityFlags::Adaptive | TargetCapabilityFlags::IntegerComputations,
        &expect![[r#"
            namespace namespace7 {
                @Config(Adaptive)
                @Config(IntegerComputations)
                function item1() : Double {}

                function item2() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn multiple_definition_dropped_is_not_found() {
    check(
        indoc! {"
            namespace A {
                @Config(Adaptive)
                operation B() : Unit {}
                @Config(Base)
                operation B() : Unit {}
                @Config(Base)
                operation C() : Unit {}
                @Config(Adaptive)
                operation C() : Unit {}
            }
            namespace D {
                operation E() : Unit {
                    B();
                    C();
                }
                operation F() : Unit {
                    open A;
                    B();
                    C();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                @Config(Adaptive)
                operation item1() : Unit {}
                @Config(Base)
                operation B() : Unit {}
                @Config(Base)
                operation C() : Unit {}
                @Config(Adaptive)
                operation item2() : Unit {}
            }
            namespace namespace8 {
                operation item4() : Unit {
                    B();
                    C();
                }
                operation item5() : Unit {
                    open namespace7;
                    item1();
                    item2();
                }
            }

            // NotFound("B", Span { lo: 257, hi: 258 })
            // NotFound("C", Span { lo: 270, hi: 271 })
        "#]],
    );
}

#[test]
fn disallow_duplicate_intrinsic() {
    check(
        indoc! {"
            namespace A {
                operation B() : Unit {
                    body intrinsic;
                }
            }
            namespace B {
                operation B() : Unit {
                    body intrinsic;
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {
                    body intrinsic;
                }
            }
            namespace namespace8 {
                operation item3() : Unit {
                    body intrinsic;
                }
            }

            // DuplicateIntrinsic("B", Span { lo: 101, hi: 102 })
        "#]],
    );
}

#[test]
fn disallow_duplicate_intrinsic_and_non_intrinsic_collision() {
    check(
        indoc! {"
            namespace A {
                internal operation C() : Unit {
                    body intrinsic;
                }
            }
            namespace B {
                operation C() : Unit {}
            }
            namespace B {
                operation C() : Unit {
                    body intrinsic;
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                internal operation item1() : Unit {
                    body intrinsic;
                }
            }
            namespace namespace8 {
                operation item3() : Unit {}
            }
            namespace namespace8 {
                operation item5() : Unit {
                    body intrinsic;
                }
            }

            // Duplicate("C", "B", Span { lo: 154, hi: 155 })
            // DuplicateIntrinsic("C", Span { lo: 154, hi: 155 })
        "#]],
    );
}

#[allow(clippy::cast_possible_truncation)]
fn check_locals(input: &str, expect: &Expect) {
    let parts = input.split('↘').collect::<Vec<_>>();
    assert_eq!(
        parts.len(),
        2,
        "input must contain exactly one cursor marker"
    );
    let cursor_offset = parts[0].len() as u32;
    let source = parts.join("");

    let (_, _, locals, _, _) = compile(
        &source,
        LanguageFeatures::default(),
        TargetCapabilityFlags::all(),
    );

    let locals = locals.get_all_at_offset(cursor_offset);
    let actual = locals.iter().fold(String::new(), |mut output, l| {
        let _ = writeln!(
            output,
            "{} ({})",
            l.name,
            match l.kind {
                LocalKind::Item(item_id) => item_id.to_string(),
                LocalKind::TyParam(param_id) => format!("ty_param {param_id}"),
                LocalKind::Var(node_id) => format!("var {node_id}"),
            }
        );
        output
    });

    expect.assert_eq(&actual);
}

#[test]
fn get_locals_vars() {
    check_locals(
        indoc! {"
            namespace Foo {
                function A() : Int {
                    let x = 0;
                    ↘
                    let y = 0;
                }
            }
        "},
        &expect![[r#"
            x (var 13)
        "#]],
    );
}

#[test]
fn get_locals_vars_shadowing_same_scope() {
    check_locals(
        indoc! {r#"
            namespace Foo {
                function A() : Int {
                    let x = 0;
                    let x = "foo";
                    ↘
                }
            }
        "#},
        &expect![[r#"
            x (var 17)
        "#]],
    );
}

#[test]
fn get_locals_vars_parent_scope() {
    check_locals(
        indoc! {r#"
            namespace Foo {
                function A() : Int {
                    let x = 0;
                    {
                        let y = 0;
                        ↘
                    }
                }
            }
        "#},
        &expect![[r#"
            y (var 20)
            x (var 13)
        "#]],
    );
}

#[test]
fn get_locals_params() {
    check_locals(
        indoc! {r#"
            namespace Foo {
                function A(x : Int) : Int {
                    ↘
                }
            }
        "#},
        &expect![[r#"
            x (var 8)
        "#]],
    );
}

#[test]
fn get_locals_spec_params() {
    check_locals(
        indoc! {"
            namespace Foo {
                operation A(q : Qubit) : (Qubit[], Qubit) {
                    controlled (cs, ...) {
                        ↘
                    }
                }
            }
        "},
        &expect![[r#"
            cs (var 23)
            q (var 8)
        "#]],
    );
}

#[test]
fn get_locals_before_binding() {
    check_locals(
        indoc! {"
            namespace Foo {
                function A() : Unit {
                    let y = 0;
                    let x = { ↘ };
                }
            }
        "},
        &expect![[r#"
            y (var 13)
        "#]],
    );
}

#[test]
fn get_locals_lambda_params() {
    check_locals(
        indoc! {"
            namespace Foo {
                function A() : Unit {
                    let y = 0;
                    let f = x -> { ↘ };
                }
            }
        "},
        &expect![[r#"
            x (var 20)
            y (var 13)
        "#]],
    );
}

#[test]
fn get_locals_for_loop() {
    check_locals(
        indoc! {"
            namespace Foo {
                function A() : Unit {
                    for x in 0..1 {
                        ↘
                    }
                }
            }
        "},
        &expect![[r#"
            x (var 14)
        "#]],
    );
}

#[test]
fn get_locals_for_loop_before_binding() {
    check_locals(
        indoc! {"
            namespace Foo {
                function A() : Unit {
                    for x in 0..{ ↘ } {
                    }
                }
            }
        "},
        &expect![""],
    );
}

#[test]
fn get_locals_items() {
    check_locals(
        indoc! {"
            namespace Foo {
                function A() : Unit {
                    ↘
                    function B() : Unit {}
                    newtype Bar = String;
                }
            }
        "},
        &expect![[r#"
            Bar (Item 3)
            B (Item 2)
        "#]],
    );
}

#[test]
fn get_locals_local_item_hide_parent_scope_variables() {
    check_locals(
        indoc! {"
            namespace Foo {
                function A() : Unit {
                    let x = 3;
                    function B() : Unit {
                        let y = 3;
                        ↘
                    }
                }
            }
        "},
        &expect![[r#"
            y (var 26)
            B (Item 2)
        "#]],
    );
}

#[test]
fn get_locals_shadow_parent_scope() {
    check_locals(
        indoc! {r#"
            namespace Foo {
                function A() : Unit {
                    let x = "foo";
                    {
                        let x = 0;
                        ↘
                    }
                }
            }
        "#},
        &expect![[r#"
            x (var 20)
        "#]],
    );
}

#[test]
fn get_locals_type_params() {
    check_locals(
        indoc! {"
            namespace Foo {
                function A<'T>(t: 'T) : Unit {
                    {
                        ↘
                    }
                }
            }
        "},
        &expect![[r#"
            t (var 9)
            'T (ty_param 0)
        "#]],
    );
}

#[test]
fn get_locals_block_scope_boundary() {
    check_locals(
        indoc! {"
            namespace Foo {
                function A() : Int {
                    {
                        let x = 0;
                    }↘
                }
            }
        "},
        &expect![""],
    );
}

#[test]
fn get_locals_block_scope_boundary_begin() {
    check_locals(
        indoc! {"
            namespace Foo {
                function A() : Int {
                    ↘{
                        function Bar(): Unit {}
                    }
                }
            }
        "},
        &expect![""],
    );
}

#[test]
fn use_after_scope() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {
                    {
                        let x = 42;
                    }
                    x; // x should not be accessible here
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {
                    {
                        let local16 = 42;
                    }
                    x; // x should not be accessible here
                }
            }

            // NotFound("x", Span { lo: 94, hi: 95 })
        "#]],
    );
}

#[test]
fn nested_function_definition() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {
                    function B() : Unit {
                        function C() : Unit {}
                        C();
                    }
                    B();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {
                    function item2() : Unit {
                        function item3() : Unit {}
                        item3();
                    }
                    item2();
                }
            }
        "#]],
    );
}

#[test]
fn variable_in_nested_blocks() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {
                    {
                        let x = 10;
                        {
                            let y = x + 5;
                            y; // Should be accessible
                        }
                        y; // Should not be accessible
                    }
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {
                    {
                        let local16 = 10;
                        {
                            let local23 = local16 + 5;
                            local23; // Should be accessible
                        }
                        y; // Should not be accessible
                    }
                }
            }

            // NotFound("y", Span { lo: 190, hi: 191 })
        "#]],
    );
}

#[test]
fn function_call_with_namespace_alias() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {}
            }
            namespace Bar {
                open Foo as F;
                function B() : Unit {
                    F.A();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}
            }
            namespace namespace8 {
                open namespace7 as F;
                function item3() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn type_alias_in_function_scope() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {
                    newtype MyInt = Int;
                    let x : MyInt = MyInt(5);
                }
                function B() : Unit {
                    let z: MyInt = MyInt(5); // this should be a different type (and unresolved)
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {
                    newtype item3 = Int;
                    let local20 : item3 = item3(5);
                }
                function item2() : Unit {
                    let local40: MyInt = MyInt(5); // this should be a different type (and unresolved)
                }
            }

            // NotFound("MyInt", Span { lo: 152, hi: 157 })
            // NotFound("MyInt", Span { lo: 160, hi: 165 })
        "#]],
    );
}

#[test]
fn lambda_inside_lambda() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {
                    let f = () -> {
                        let g = (x) -> x + 1;
                        g(10);
                    };
                    f();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {
                    let local13 = () -> {
                        let local20 = (local24) -> local24 + 1;
                        local20(10);
                    };
                    local13();
                }
            }
        "#]],
    );
}

#[test]
fn nested_namespaces_with_same_function_name() {
    check(
        indoc! {"
            namespace Foo {
                function A() : Unit {}
            }
            namespace Bar {
                function A() : Unit {}
                function B() : Unit {
                    Foo.A();
                    A(); // Should call Bar.A without needing to qualify
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}
            }
            namespace namespace8 {
                function item3() : Unit {}
                function item4() : Unit {
                    item1();
                    item3(); // Should call Bar.A without needing to qualify
                }
            }
        "#]],
    );
}

#[test]
fn newtype_with_invalid_field_type() {
    check(
        indoc! {"
            namespace Foo {
                newtype Complex = (Re: Real, Im: Imaginary); // Imaginary is not a valid type
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                newtype item1 = (Re: Real, Im: Imaginary); // Imaginary is not a valid type
            }

            // NotFound("Real", Span { lo: 43, hi: 47 })
            // NotFound("Imaginary", Span { lo: 53, hi: 62 })
        "#]],
    );
}

#[test]
fn newtype_with_tuple_destructuring() {
    check(
        indoc! {"
            namespace Foo {
                newtype Pair = (First: Int, Second: Int);
                function Destructure(pair: Pair) : Int {
                    let (first, second) = pair;
                    first + second
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                newtype item1 = (First: Int, Second: Int);
                function item2(local21: item1) : Int {
                    let (local32, local34) = local21;
                    local32 + local34
                }
            }
        "#]],
    );
}

#[test]
fn items_resolve_according_to_implicit_hierarchy() {
    check(
        indoc! {"
namespace Foo {
  @EntryPoint()
  function Main(): Int {
    Foo()
  }

  function Foo() : Int {
    Bar.Baz.Quux()
  }
}

namespace Foo.Bar.Baz {
  function Quux() : Int { 6 }
}
"},
        &expect![[r#"
            namespace namespace7 {
              @EntryPoint()
              function item1(): Int {
                item2()
              }

              function item2() : Int {
                item4()
              }
            }

            namespace namespace9 {
              function item4() : Int { 6 }
            }
        "#]],
    );
}

#[test]
fn basic_hierarchical_namespace() {
    check(
        indoc! {"
    namespace Foo.Bar.Baz {
        operation Quux() : Unit {}
    }
    namespace A {
        open Foo;
        operation Main() : Unit {
            Bar.Baz.Quux();
        }
    }
    namespace B {
        open Foo.Bar;
        operation Main() : Unit {
            Baz.Quux();
        }
    }"},
        &expect![[r#"
            namespace namespace9 {
                operation item1() : Unit {}
            }
            namespace namespace10 {
                open namespace7;
                operation item3() : Unit {
                    item1();
                }
            }
            namespace namespace11 {
                open namespace8;
                operation item5() : Unit {
                    item1();
                }
            }"#]],
    );
}

#[test]
fn test_katas_shadowing_use_case() {
    check(
        indoc! {"namespace Kata {
    operation ApplyX() : Unit {
        // Do nothing.
    }
}

namespace Kata.Verification {
    operation CheckSolution() : Bool {
        let _ = Kata.ApplyX();
        let _ = ApplyX();
    }

    operation ApplyX() : Unit {}
}
" },
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {
                    // Do nothing.
                }
            }

            namespace namespace8 {
                operation item3() : Bool {
                    let _ = item1();
                    let _ = item4();
                }

                operation item4() : Unit {}
            }
        "#]],
    );
}

#[test]
fn open_can_access_parent_scope() {
    check(
        indoc! {r#"
namespace Foo.Bar {
    operation Hello() : Unit {

    }
}

namespace Foo {
    open Bar;
    @EntryPoint()
    operation Main() : Unit {
        Hello();
    }
}"#},
        &expect![[r#"
            namespace namespace8 {
                operation item1() : Unit {

                }
            }

            namespace namespace7 {
                open Bar;
                @EntryPoint()
                operation item3() : Unit {
                    item1();
                }
            }"#]],
    );
}

#[test]
fn test_export_statement() {
    check(
        indoc! {"namespace Foo {
    operation ApplyX() : Unit {
    }
    export ApplyX;
}
" },
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {
                }
                export item1;
            }
        "#]],
    );
}

#[test]
fn test_complicated_nested_export_statement() {
    check(
        indoc! {
"

namespace Foo {
    export Foo.Bar.Baz.Quux.HelloWorld;
}
namespace Foo.Bar.Baz.Quux {
    function HelloWorld() : Unit {}
}

namespace Foo.Bar {
   export Baz.Quux.HelloWorld;
}

namespace Foo.Bar.Baz {
    export Quux.HelloWorld;
}

namespace Foo.Bar.Graule {
    // HelloWorld should be available from all namespaces
    operation Main() : Unit {
        Foo.Bar.Baz.Quux.HelloWorld();
        Foo.Bar.Baz.HelloWorld();
        Foo.Bar.HelloWorld();
        Foo.HelloWorld();
        open Foo;
        HelloWorld();
    }
    // and we should be able to re-export it
    export Foo.HelloWorld;
}" },
        &expect![[r#"

            namespace namespace7 {
                export item2;
            }
            namespace namespace10 {
                function item2() : Unit {}
            }

            namespace namespace8 {
               export item2;
            }

            namespace namespace9 {
                export item2;
            }

            namespace namespace11 {
                // HelloWorld should be available from all namespaces
                operation item6() : Unit {
                    item2();
                    item2();
                    item2();
                    item2();
                    open namespace7;
                    item2();
                }
                // and we should be able to re-export it
                export item2;
            }"#]],
    );
}

#[test]
fn exports_aware_of_opens() {
    check(
        indoc! {r#"
            namespace Foo {
                operation F() : Unit {}
            }
            namespace Main {
                open Foo;
                export F;
            }
            "# },
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
            }
            namespace namespace8 {
                open namespace7;
                export item1;
            }
        "#]],
    );
}

#[test]
fn export_symbol_and_call_it() {
    check(
        indoc! {
"
namespace Foo {
    export Foo.Bar.Baz.Quux.Function;
}
namespace Foo.Bar.Baz.Quux {
    function Function() : Unit {}
}

namespace Main {
  open Foo;
  operation Main() : Unit {
    Foo.Function();
    Function();
  }
}" },
        &expect![[r#"
            namespace namespace7 {
                export item2;
            }
            namespace namespace10 {
                function item2() : Unit {}
            }

            namespace namespace11 {
              open namespace7;
              operation item4() : Unit {
                item2();
                item2();
              }
            }"#]],
    );
}

#[test]
fn export_non_existent_symbol() {
    check(
        indoc! {"
            namespace Foo {
                export NonExistent;
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                export NonExistent;
            }

            // NotFound("NonExistent", Span { lo: 27, hi: 38 })
        "#]],
    );
}

#[test]
fn export_symbol_from_nested_namespace() {
    check(
        indoc! {"
            namespace Foo.Bar  {
                operation ApplyX() : Unit {}
            }
            namespace Foo {
                export Bar.ApplyX;
            }
            namespace Main {
                open Foo;
                operation Main() : Unit {
                    Bar.ApplyX();
                }
            }
        "},
        &expect![[r#"
            namespace namespace8  {
                operation item1() : Unit {}
            }
            namespace namespace7 {
                export item1;
            }
            namespace namespace9 {
                open namespace7;
                operation item4() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn disallow_exporting_local_vars() {
    check(
        indoc! {"
            namespace Foo {
                operation Main() : Unit {
                    let x = 5;
                }
                export x;
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {
                    let local13 = 5;
                }
                export x;
            }

            // NotFound("x", Span { lo: 82, hi: 83 })
        "#]],
    );
}

#[test]
fn export_non_item() {
    check(
        indoc! {"
            namespace Bar {}
            namespace Foo {
                operation Main() : Unit {
                }
                export Unit;

            }
        "},
        &expect![[r#"
            namespace namespace7 {}
            namespace namespace8 {
                operation item2() : Unit {
                }
                export Unit;

            }

            // ExportedNonItem(Span { lo: 80, hi: 84 })
        "#]],
    );
}

#[test]
fn export_with_alias() {
    check(
        indoc! {"
            namespace Foo {
                operation ApplyX() : Unit {}
                export ApplyX as SomeAlias;
            }
            namespace Main {
                open Foo;
                operation Main() : Unit {
                    SomeAlias();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
                export item1;
            }
            namespace namespace8 {
                open namespace7;
                operation item3() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn multiple_exports_with_aliases() {
    check(
        indoc! {"
            namespace Foo {
                operation ApplyX() : Unit {}
                operation ApplyY() : Unit {}
                export ApplyX as SomeAlias, ApplyY as AnotherAlias;
            }
            namespace Main {
                open Foo;
                operation Main() : Unit {
                    SomeAlias();
                    AnotherAlias();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
                operation item2() : Unit {}
                export item1, item2;
            }
            namespace namespace8 {
                open namespace7;
                operation item4() : Unit {
                    item1();
                    item2();
                }
            }
        "#]],
    );
}

#[test]
fn aliased_exports_call_with_qualified_paths() {
    check(
        indoc! {"
            namespace Foo {
                operation ApplyX() : Unit {}
                operation ApplyY() : Unit {}
                export ApplyX as SomeAlias, ApplyY as AnotherAlias;
            }
            namespace Main {
                open Foo;
                operation Main() : Unit {
                    Foo.SomeAlias();
                    Foo.AnotherAlias();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
                operation item2() : Unit {}
                export item1, item2;
            }
            namespace namespace8 {
                open namespace7;
                operation item4() : Unit {
                    item1();
                    item2();
                }
            }
        "#]],
    );
}

#[test]
fn reexport_from_full_path_with_alias() {
    check(
        indoc! {"
            namespace Foo {
                operation ApplyX() : Unit {}
                export ApplyX as SomeAlias;
            }
            namespace Main {
                open Foo;
                export Foo.SomeAlias as AnotherAlias;
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
                export item1;
            }
            namespace namespace8 {
                open namespace7;
                export item1;
            }
        "#]],
    );
}

#[test]
fn disallow_repeated_exports() {
    check(
        indoc! {"
            namespace Foo {
                operation ApplyX() : Unit {}
                export ApplyX;
                export ApplyX;
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
                export item1;
                export item1;
            }

            // DuplicateExport("ApplyX", Span { lo: 79, hi: 85 })
        "#]],
    );
}

#[test]
fn disallow_repeated_exports_inline() {
    check(
        indoc! {"
            namespace Foo {
                operation ApplyX() : Unit {}
                export ApplyX, ApplyX;
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
                export item1, item1;
            }

            // DuplicateExport("ApplyX", Span { lo: 68, hi: 74 })
        "#]],
    );
}

#[test]
fn order_of_exports_does_not_matter() {
    check(
        indoc! {"
            namespace Bar {
                export Foo.ApplyX;
                export ApplyY;
                operation ApplyY() : Unit {}
            }
            namespace Foo {
                operation ApplyX() : Unit {}
            }

        "},
        &expect![[r#"
            namespace namespace7 {
                export item3;
                export item1;
                operation item1() : Unit {}
            }
            namespace namespace8 {
                operation item3() : Unit {}
            }

        "#]],
    );
}

#[test]
fn export_udt_and_construct_it() {
    check(
        indoc! {"
            namespace Foo {
                newtype Pair = (First: Int, Second: Int);
                export Pair;
            }
            namespace Main {
                open Foo;
                operation Main() : Unit {
                    let z: Pair = Pair(1, 2);
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                newtype item1 = (First: Int, Second: Int);
                export item1;
            }
            namespace namespace8 {
                open namespace7;
                operation item3() : Unit {
                    let local33: item1 = item1(1, 2);
                }
            }
        "#]],
    );
}
#[test]
fn import_single_item() {
    check(
        indoc! {"
            namespace Foo {
                function Bar() : Unit {}
            }
            namespace Main {
                import Foo.Bar;
                operation Main() : Unit {
                    Bar();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}
            }
            namespace namespace8 {
                import item1;
                operation item3() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn import_namespace() {
    check(
        indoc! {"
            namespace Foo.Bar {
                function Baz() : Unit {}
            }
            namespace Main {
                import Foo.Bar;
                operation Main() : Unit {
                    Bar.Baz();
                }
            }
        "},
        &expect![[r#"
            namespace namespace8 {
                function item1() : Unit {}
            }
            namespace namespace9 {
                import namespace8;
                operation item3() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn import_non_existent_item() {
    check(
        indoc! {"
            namespace Foo {
            }
            namespace Main {
                import Foo.Bar;
                operation Main() : Unit {
                    Bar();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
            }
            namespace namespace8 {
                import Foo.Bar;
                operation item2() : Unit {
                    Bar();
                }
            }

            // NotFound("Foo.Bar", Span { lo: 46, hi: 53 })
            // NotFound("Bar", Span { lo: 93, hi: 96 })
        "#]],
    );
}

#[test]
fn import_shadowing() {
    check(
        indoc! {"
            namespace Foo {
                function Bar() : Unit {}
            }
            namespace Main {
                function Bar() : Unit {}
                import Foo.Bar;
                operation Main() : Unit {
                    Bar();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}
            }
            namespace namespace8 {
                function item3() : Unit {}
                import item1;
                operation item4() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn import_with_alias() {
    check(
        indoc! {"
            namespace Foo {
                function Bar() : Unit {}
            }
            namespace Main {
                import Foo.Bar as Baz;
                operation Main() : Unit {
                    Baz();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                function item1() : Unit {}
            }
            namespace namespace8 {
                import item1;
                operation item3() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn import_non_item() {
    check(
        indoc! {"
            namespace Main {
                import Unit;
                operation Main() : Unit {
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                import Unit;
                operation item1() : Unit {
                }
            }

            // ImportedNonItem(Span { lo: 28, hi: 32 })
        "#]],
    );
}

#[test]
fn import_namespace_nested() {
    check(
        indoc! {"
            namespace Foo.Bar.Baz {
                operation Quux() : Unit {}
            }
            namespace Main {
                import Foo.Bar;
                operation Main() : Unit {
                    Bar.Baz.Quux();
                }
            }
        "},
        &expect![[r#"
            namespace namespace9 {
                operation item1() : Unit {}
            }
            namespace namespace10 {
                import namespace8;
                operation item3() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn import_single_namespace() {
    check(
        indoc! {"
            namespace Foo {
                operation Bar() : Unit {}
            }
            namespace Main {
                import Foo;

                operation Main() : Unit {
                    Foo.Bar();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
            }
            namespace namespace8 {
                import namespace7;

                operation item3() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn import_shadowing_function() {
    check(
        indoc! {"
            namespace Foo {
                operation Bar() : Unit {}
            }
            namespace Main {
                operation Bar() : Unit {}
                operation Main() : Unit {
                    import Foo.Bar;
                    Bar();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
            }
            namespace namespace8 {
                operation item3() : Unit {}
                operation item4() : Unit {
                    import item1;
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn import_non_existent_namespace() {
    check(
        indoc! {"
            namespace Main {
                operation Main() : Unit {
                    import NonExistent;
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {
                    import NonExistent;
                }
            }

            // NotFound("NonExistent", Span { lo: 62, hi: 73 })
        "#]],
    );
}

#[test]
fn import_self() {
    check(
        indoc! {"
            namespace Main {
                operation Foo() : Unit {
                    import Foo;
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {
                    import item1;
                }
            }
        "#]],
    );
}

#[test]
fn import_duplicate_symbol() {
    check(
        indoc! { r#"
        namespace Main {
            import Foo.Bar.Baz, Foo.Bar.Baz;
        }
        namespace Foo.Bar {
            operation Baz() : Unit {}
        }
"# },
        &expect![[r#"
            namespace namespace7 {
                import item2, item2;
            }
            namespace namespace9 {
                operation item2() : Unit {}
            }

            // ImportedDuplicate("Baz", Span { lo: 49, hi: 52 })
        "#]],
    );
}

#[test]
fn import_duplicate_symbol_different_name() {
    check(
        indoc! { r#"
        namespace Main {
            import Foo.Bar.Baz, Foo.Bar;
            import Bar.Baz;
        }
        namespace Foo.Bar {
            operation Baz() : Unit {}
        }
"# },
        &expect![[r#"
            namespace namespace7 {
                import item2, namespace9;
                import item2;
            }
            namespace namespace9 {
                operation item2() : Unit {}
            }

            // ImportedDuplicate("Baz", Span { lo: 65, hi: 68 })
        "#]],
    );
}
#[test]
fn import_takes_precedence_over_local_decl() {
    check(
        indoc! { r#"
        namespace Main {

            operation Baz() : Unit {
                import Foo.Bar.Baz;
                Baz();
            }

        }

        namespace Foo.Bar {
            operation Baz() : Unit {}
        }
"# },
        &expect![[r#"
            namespace namespace7 {

                operation item1() : Unit {
                    import item3;
                    item3();
                }

            }

            namespace namespace9 {
                operation item3() : Unit {}
            }
        "#]],
    );
}

#[test]
fn import_then_export() {
    check(
        indoc! {"
            namespace Foo {
                operation Bar() : Unit {}
            }
            namespace Main {
                import Foo.Bar;
                export Bar;
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
            }
            namespace namespace8 {
                import item1;
                export item1;
            }
        "#]],
    );
}

#[test]
fn import_namespace_advanced() {
    check(
        indoc! {"
            namespace A.B.C.D.E {
                operation DumpMachine() : Unit {}
            }
            namespace TestOne {
                import A;
                operation Main() : Unit {
                    A.B.C.D.E.DumpMachine();
                }
            }
            namespace TestTwo {
                import A.B;
                operation Main() : Unit {
                    B.C.D.E.DumpMachine();
                }
            }
            namespace TestThree {
                import A.B.C;
                operation Main() : Unit {
                    C.D.E.DumpMachine();
                }
            }
            namespace TestFour {
                import A.B.C.D;
                operation Main() : Unit {
                    D.E.DumpMachine();
                }
            }
            namespace TestFive {
                import A.B.C.D.E;
                operation Main() : Unit {
                    E.DumpMachine();
                }
            }
            namespace TestSix {
                import A.B.C.D.E.DumpMachine;
                operation Main() : Unit {
                    DumpMachine();
                }
            }
        "},
        &expect![[r#"
            namespace namespace11 {
                operation item1() : Unit {}
            }
            namespace namespace12 {
                import namespace7;
                operation item3() : Unit {
                    item1();
                }
            }
            namespace namespace13 {
                import namespace8;
                operation item5() : Unit {
                    item1();
                }
            }
            namespace namespace14 {
                import namespace9;
                operation item7() : Unit {
                    item1();
                }
            }
            namespace namespace15 {
                import namespace10;
                operation item9() : Unit {
                    item1();
                }
            }
            namespace namespace16 {
                import namespace11;
                operation item11() : Unit {
                    item1();
                }
            }
            namespace namespace17 {
                import item1;
                operation item13() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn import_namespace_does_not_open_it() {
    check(
        indoc! {"
            namespace Microsoft.Quantum.Diagnostics {
                operation DumpMachine() : Unit {}
            }
            namespace Main {
                import Microsoft.Quantum.Diagnostics;
                operation Main() : Unit {
                    Diagnostics.DumpMachine();
                    DumpMachine();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
            }
            namespace namespace8 {
                import namespace7;
                operation item3() : Unit {
                    item1();
                    DumpMachine();
                }
            }

            // NotFound("DumpMachine", Span { lo: 214, hi: 225 })
        "#]],
    );
}

#[test]
fn invalid_import() {
    check(
        indoc! {"
            namespace Main {
                import A.B.C;
                operation Main() : Unit {
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                import A.B.C;
                operation item1() : Unit {
                }
            }

            // NotFound("A.B.C", Span { lo: 28, hi: 33 })
        "#]],
    );
}

#[test]
fn export_namespace() {
    check(
        indoc! {"
            namespace Foo {
                operation ApplyX() : Unit {}
                operation ApplyY() : Unit {}
            }
            namespace Main {
                export Foo;
            }
            namespace Test {
                open Main.Foo;
                operation Main() : Unit {
                    ApplyX();
                    ApplyY();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
                operation item2() : Unit {}
            }
            namespace namespace8 {
                export namespace7;
            }
            namespace namespace9 {
                open namespace7;
                operation item5() : Unit {
                    item1();
                    item2();
                }
            }
        "#]],
    );
}

#[test]
fn export_namespace_contains_children() {
    check(
        indoc! {"
            namespace Foo.Bar {
                operation ApplyX() : Unit {}
            }
            namespace Main {
                export Foo;
            }
            namespace Test {
                open Main.Foo.Bar;
                operation Main() : Unit {
                    ApplyX();
                }
            }
        "},
        &expect![[r#"
            namespace namespace8 {
                operation item1() : Unit {}
            }
            namespace namespace9 {
                export namespace7;
            }
            namespace namespace10 {
                open namespace8;
                operation item4() : Unit {
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn export_namespace_cyclic() {
    check(
        indoc! {"
            namespace Foo {
                export Bar;
            }
            namespace Bar {
                export Foo;
                operation Hello() : Unit {}
            }
            namespace Main {
                open Foo.Bar.Foo.Bar.Foo.Bar;
                operation Main() : Unit { Hello(); }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                export namespace8;
            }
            namespace namespace8 {
                export namespace7;
                operation item2() : Unit {}
            }
            namespace namespace9 {
                open namespace8;
                operation item4() : Unit { item2(); }
            }
        "#]],
    );
}

#[test]
fn export_direct_cycle() {
    check(
        indoc! {"
            namespace Foo {
                export Foo;
            }

            namespace Main {
                open Foo.Foo.Foo.Foo.Foo;
                operation Main() : Unit { }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                export namespace7;
            }

            namespace namespace8 {
                open namespace7;
                operation item2() : Unit { }
            }
        "#]],
    );
}

#[test]
fn export_namespace_with_alias() {
    check(
        indoc! {"
            namespace Foo.Bar {
                operation ApplyX() : Unit {}
            }
            namespace Main {
                export Foo.Bar as Baz;
            }
            namespace Test {
                open Main.Baz;
                operation Main() : Unit {
                    ApplyX();
                    Main.Baz.ApplyX();
                }
            }
        "},
        &expect![[r#"
            namespace namespace8 {
                operation item1() : Unit {}
            }
            namespace namespace9 {
                export namespace8;
            }
            namespace namespace10 {
                open namespace8;
                operation item4() : Unit {
                    item1();
                    item1();
                }
            }
        "#]],
    );
}

#[test]
fn import_glob() {
    check(
        indoc! {"
            namespace Foo {
                operation ApplyX() : Unit {}
                operation ApplyY() : Unit {}
            }
            namespace Main {
                import Foo.*;
                operation Main() : Unit {
                    ApplyX();
                    ApplyY();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
                operation item2() : Unit {}
            }
            namespace namespace8 {
                import namespace7.*;
                operation item4() : Unit {
                    item1();
                    item2();
                }
            }
        "#]],
    );
}

#[test]
fn import_aliased_glob() {
    check(
        indoc! {"
            namespace Foo {
                operation ApplyX() : Unit {}
                operation ApplyY() : Unit {}
            }
            namespace Main {
                import Foo as Bar;
                operation Main() : Unit {
                    Bar.ApplyX();
                    Bar.ApplyY();
                }
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
                operation item2() : Unit {}
            }
            namespace namespace8 {
                import namespace7;
                operation item4() : Unit {
                    item1();
                    item2();
                }
            }
        "#]],
    );
}

#[test]
fn disallow_glob_export() {
    check(
        indoc! {"
            namespace Foo {
                operation ApplyX() : Unit {}
                operation ApplyY() : Unit {}
            }
            namespace Bar {
                export Foo.*;
            }
        "},
        &expect![[r#"
            namespace namespace7 {
                operation item1() : Unit {}
                operation item2() : Unit {}
            }
            namespace namespace8 {
                export namespace7.*;
            }

            // GlobExportNotSupported(Span { lo: 111, hi: 114 })
        "#]],
    );
}

#[test]
fn import_glob_in_list() {
    check(
        indoc! {"
            namespace Foo.Bar {
                operation ApplyX() : Unit {}
                operation ApplyY() : Unit {}
            }
            namespace Foo.Bar.Baz {
                operation ApplyZ() : Unit {}
            }
            namespace Main {
                import Foo.Bar.*, Foo.Bar.Baz.ApplyZ;
                operation Main() : Unit {
                    ApplyX();
                    ApplyY();
                    Baz.ApplyZ();
                    ApplyZ();
                }
            }
        "},
        &expect![[r#"
            namespace namespace8 {
                operation item1() : Unit {}
                operation item2() : Unit {}
            }
            namespace namespace9 {
                operation item4() : Unit {}
            }
            namespace namespace10 {
                import namespace8.*, item4;
                operation item6() : Unit {
                    item1();
                    item2();
                    item4();
                    item4();
                }
            }
        "#]],
    );
}

#[test]
fn import_glob_in_list_with_alias() {
    check(
        indoc! {"
            namespace Foo.Bar {
                operation ApplyX() : Unit {}
                operation ApplyY() : Unit {}
            }
            namespace Foo.Bar.Baz {
                operation ApplyZ() : Unit {}
            }
            namespace Main {
                import Foo.Bar as Alias, Foo.Bar.Baz.ApplyZ as Foo;
                operation Main() : Unit {
                    Alias.ApplyX();
                    Alias.ApplyY();
                    Alias.Baz.ApplyZ();
                    Foo();
                }
            }
        "},
        &expect![[r#"
            namespace namespace8 {
                operation item1() : Unit {}
                operation item2() : Unit {}
            }
            namespace namespace9 {
                operation item4() : Unit {}
            }
            namespace namespace10 {
                import namespace8, item4;
                operation item6() : Unit {
                    item1();
                    item2();
                    item4();
                    item4();
                }
            }
        "#]],
    );
}

#[test]
fn import_newtype() {
    check(
        indoc! {r#"
                namespace Foo {
                    import Bar.NewType; // no error

                    operation FooOperation() : Unit {
                        let x: NewType = NewType("a");
                    }
                }

                namespace Bar {
                    newtype NewType = String;
                    export NewType;

                }"#},
        &expect![[r#"
            namespace namespace7 {
                import item3; // no error

                operation item1() : Unit {
                    let local17: item3 = item3("a");
                }
            }

            namespace namespace8 {
                newtype item3 = String;
                export item3;

            }"#]],
    );
}
