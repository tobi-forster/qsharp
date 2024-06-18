// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#[cfg(test)]
mod tests;

use std::rc::Rc;

use crate::compilation::Compilation;
use crate::name_locator::{Handler, Locator, LocatorContext};
use crate::qsc_utils::into_location;
use qsc::ast::visit::{walk_callable_decl, walk_expr, walk_ty, Visitor};
use qsc::display::Lookup;
use qsc::hir::ty::Ty;
use qsc::hir::{PackageId, Res};
use qsc::line_column::{Encoding, Position};
use qsc::location::Location;
use qsc::{ast, hir, resolve, Span};

pub(crate) fn get_references(
    compilation: &Compilation,
    source_name: &str,
    position: Position,
    position_encoding: Encoding,
    include_declaration: bool,
) -> Vec<Location> {
    let offset =
        compilation.source_position_to_package_offset(source_name, position, position_encoding);
    let user_ast_package = &compilation.user_unit().ast.package;

    let mut name_handler = NameHandler {
        reference_finder: ReferenceFinder::new(position_encoding, compilation, include_declaration),
        references: vec![],
    };

    let mut locator = Locator::new(&mut name_handler, offset, compilation);
    locator.visit_package(user_ast_package);

    name_handler.references
}

pub(crate) struct ReferenceFinder<'a> {
    position_encoding: Encoding,
    compilation: &'a Compilation,
    include_declaration: bool,
}

struct NameHandler<'a> {
    reference_finder: ReferenceFinder<'a>,
    references: Vec<Location>,
}

impl<'a> Handler<'a> for NameHandler<'a> {
    fn at_callable_def(
        &mut self,
        _: &LocatorContext<'a>,
        name: &'a ast::Ident,
        _: &'a ast::CallableDecl,
    ) {
        if let Some(resolve::Res::Item(item_id, _)) =
            self.reference_finder.compilation.get_res(name.id)
        {
            self.references = self.reference_finder.for_item(item_id);
        }
    }

    fn at_callable_ref(
        &mut self,
        _: &'a ast::Path,
        item_id: &'_ hir::ItemId,
        _: &'a hir::CallableDecl,
    ) {
        self.references = self.reference_finder.for_item(item_id);
    }

    fn at_type_param_def(
        &mut self,
        context: &LocatorContext<'a>,
        _: &'a ast::Ident,
        param_id: hir::ty::ParamId,
    ) {
        if let Some(curr) = context.current_callable {
            self.references = self.reference_finder.for_ty_param(param_id, curr);
        }
    }

    fn at_type_param_ref(
        &mut self,
        context: &LocatorContext<'a>,
        _: &'a ast::Ident,
        param_id: hir::ty::ParamId,
        _: &'a ast::Ident,
    ) {
        if let Some(curr) = context.current_callable {
            self.references = self.reference_finder.for_ty_param(param_id, curr);
        }
    }

    fn at_new_type_def(
        &mut self,
        _: &LocatorContext<'a>,
        type_name: &'a ast::Ident,
        _: &'a ast::TyDef,
    ) {
        if let Some(resolve::Res::Item(item_id, _)) =
            self.reference_finder.compilation.get_res(type_name.id)
        {
            self.references = self.reference_finder.for_item(item_id);
        }
    }

    fn at_struct_def(
        &mut self,
        _: &LocatorContext<'a>,
        type_name: &'a ast::Ident,
        _: &'a ast::StructDecl,
    ) {
        if let Some(resolve::Res::Item(item_id, _)) =
            self.reference_finder.compilation.get_res(type_name.id)
        {
            self.references = self.reference_finder.for_item(item_id);
        }
    }

    fn at_new_type_ref(
        &mut self,
        _: &'a ast::Path,
        item_id: &'_ hir::ItemId,
        _: &'a hir::Ident,
        _: &'a hir::ty::Udt,
    ) {
        self.references = self.reference_finder.for_item(item_id);
    }

    fn at_field_def(
        &mut self,
        context: &LocatorContext<'a>,
        field_name: &'a ast::Ident,
        _: &'a ast::Ty,
    ) {
        if let Some(ty_item_id) = context.current_udt_id {
            self.references = self
                .reference_finder
                .for_field(ty_item_id, field_name.name.clone());
        }
    }

    fn at_field_ref(
        &mut self,
        field_ref: &'a ast::Ident,
        item_id: &'_ hir::ItemId,
        _: &'a hir::ty::UdtField,
    ) {
        self.references = self
            .reference_finder
            .for_field(item_id, field_ref.name.clone());
    }

    fn at_local_def(
        &mut self,
        context: &LocatorContext<'a>,
        ident: &'a ast::Ident,
        _: &'a ast::Pat,
    ) {
        if let Some(curr) = context.current_callable {
            self.references = self.reference_finder.for_local(ident.id, curr);
        }
    }

    fn at_local_ref(
        &mut self,
        context: &LocatorContext<'a>,
        _: &'a ast::Path,
        _: &'a ast::NodeId,
        definition: &'a ast::Ident,
    ) {
        if let Some(curr) = context.current_callable {
            self.references = self.reference_finder.for_local(definition.id, curr);
        }
    }
}

impl<'a> ReferenceFinder<'a> {
    pub fn new(
        position_encoding: Encoding,
        compilation: &'a Compilation,
        include_declaration: bool,
    ) -> Self {
        Self {
            position_encoding,
            compilation,
            include_declaration,
        }
    }

    pub fn for_item(&self, item_id: &hir::ItemId) -> Vec<Location> {
        let mut locations = vec![];

        let (def, _, resolved_item_id) = self
            .compilation
            .resolve_item_relative_to_user_package(item_id);
        if self.include_declaration {
            let def_span = match &def.kind {
                hir::ItemKind::Callable(decl) => decl.name.span,
                hir::ItemKind::Namespace(name, _) => name.span(),
                hir::ItemKind::Ty(name, _) => name.span,
            };
            locations.push(
                self.location(
                    def_span,
                    resolved_item_id
                        .package
                        .expect("package id should have been resolved"),
                ),
            );
        }

        let mut find_refs = FindItemRefs {
            item_id: &resolved_item_id,
            compilation: self.compilation,
            locations: vec![],
        };

        find_refs.visit_package(&self.compilation.user_unit().ast.package);
        locations.extend(
            find_refs
                .locations
                .drain(..)
                .map(|l| self.location(l, self.compilation.user_package_id)),
        );

        locations
    }

    pub fn for_field(&self, ty_item_id: &hir::ItemId, field_name: Rc<str>) -> Vec<Location> {
        let mut locations = vec![];

        let (ty_def, _, resolved_ty_item_id) = self
            .compilation
            .resolve_item_relative_to_user_package(ty_item_id);
        if self.include_declaration {
            if let hir::ItemKind::Ty(_, udt) = &ty_def.kind {
                let ty_field = udt
                    .find_field_by_name(&field_name)
                    .expect("field name should exist");
                let def_span = ty_field
                    .name_span
                    .expect("field found via name should have a name");
                locations.push(
                    self.location(
                        def_span,
                        resolved_ty_item_id
                            .package
                            .expect("package id should have been resolved"),
                    ),
                );
            } else {
                panic!("item id resolved to non-type: {ty_item_id}");
            }
        }

        let mut find_refs = FindFieldRefs {
            ty_item_id: &resolved_ty_item_id,
            field_name,
            compilation: self.compilation,
            locations: vec![],
        };

        find_refs.visit_package(&self.compilation.user_unit().ast.package);
        locations.extend(
            find_refs
                .locations
                .drain(..)
                .map(|l| self.location(l, self.compilation.user_package_id)),
        );

        locations
    }

    pub fn for_local(&self, node_id: ast::NodeId, callable: &ast::CallableDecl) -> Vec<Location> {
        let mut find_refs = FindLocalLocations {
            node_id,
            compilation: self.compilation,
            include_declaration: self.include_declaration,
            locations: vec![],
        };
        find_refs.visit_callable_decl(callable);
        find_refs
            .locations
            .into_iter()
            .map(|l| self.location(l, self.compilation.user_package_id))
            .collect()
    }

    pub fn for_ty_param(
        &self,
        param_id: hir::ty::ParamId,
        callable: &ast::CallableDecl,
    ) -> Vec<Location> {
        let mut find_refs = FindTyParamLocations {
            param_id,
            compilation: self.compilation,
            include_declaration: self.include_declaration,
            locations: vec![],
        };
        find_refs.visit_callable_decl(callable);
        find_refs
            .locations
            .into_iter()
            .map(|l| self.location(l, self.compilation.user_package_id))
            .collect()
    }

    fn location(&self, location: Span, package_id: PackageId) -> Location {
        into_location(
            self.position_encoding,
            self.compilation,
            location,
            package_id,
        )
    }
}

struct FindItemRefs<'a> {
    item_id: &'a hir::ItemId,
    compilation: &'a Compilation,
    locations: Vec<Span>,
}

impl<'a> Visitor<'_> for FindItemRefs<'a> {
    fn visit_path(&mut self, path: &'_ ast::Path) {
        let res = self.compilation.get_res(path.id);
        if let Some(resolve::Res::Item(item_id, _)) = res {
            if self.eq(item_id) {
                self.locations.push(path.name.span);
            }
        }
    }

    fn visit_ty(&mut self, ty: &'_ ast::Ty) {
        if let ast::TyKind::Path(ty_path) = &*ty.kind {
            let res = self.compilation.get_res(ty_path.id);
            if let Some(resolve::Res::Item(item_id, _)) = res {
                if self.eq(item_id) {
                    self.locations.push(ty_path.name.span);
                }
            }
        } else {
            walk_ty(self, ty);
        }
    }
}

impl<'a> FindItemRefs<'a> {
    fn eq(&mut self, item_id: &hir::ItemId) -> bool {
        item_id.item == self.item_id.item
            && item_id.package.unwrap_or(self.compilation.user_package_id)
                == self.item_id.package.expect("package id should be resolved")
    }
}

struct FindFieldRefs<'a> {
    ty_item_id: &'a hir::ItemId,
    field_name: Rc<str>,
    compilation: &'a Compilation,
    locations: Vec<Span>,
}

impl<'a> Visitor<'_> for FindFieldRefs<'a> {
    fn visit_expr(&mut self, expr: &'_ ast::Expr) {
        match &*expr.kind {
            ast::ExprKind::Field(qualifier, field_name) => {
                self.visit_expr(qualifier);
                if field_name.name == self.field_name {
                    if let Some(Ty::Udt(_, Res::Item(id))) = self.compilation.get_ty(qualifier.id) {
                        if self.eq(id) {
                            self.locations.push(field_name.span);
                        }
                    }
                }
            }
            ast::ExprKind::Struct(struct_name, copy, fields) => {
                self.visit_path(struct_name);
                if let Some(copy) = copy {
                    self.visit_expr(copy);
                }
                for field in fields.iter() {
                    if field.field.name == self.field_name {
                        if let Some(Ty::Udt(_, Res::Item(id))) = self.compilation.get_ty(expr.id) {
                            if self.eq(id) {
                                self.locations.push(field.field.span);
                            }
                        }
                    }
                    self.visit_expr(&field.value);
                }
            }
            _ => walk_expr(self, expr),
        }
    }
}

impl<'a> FindFieldRefs<'a> {
    fn eq(&mut self, item_id: &hir::ItemId) -> bool {
        item_id.item == self.ty_item_id.item
            && item_id.package.unwrap_or(self.compilation.user_package_id)
                == self
                    .ty_item_id
                    .package
                    .expect("package id should be resolved")
    }
}

struct FindLocalLocations<'a> {
    node_id: ast::NodeId,
    compilation: &'a Compilation,
    include_declaration: bool,
    locations: Vec<Span>,
}

impl<'a> Visitor<'_> for FindLocalLocations<'a> {
    fn visit_pat(&mut self, pat: &'_ ast::Pat) {
        if self.include_declaration {
            match &*pat.kind {
                ast::PatKind::Bind(ident, _) => {
                    if ident.id == self.node_id {
                        self.locations.push(ident.span);
                    }
                }
                _ => ast::visit::walk_pat(self, pat),
            }
        }
    }

    fn visit_path(&mut self, path: &'_ ast::Path) {
        let res = self.compilation.get_res(path.id);
        if let Some(resolve::Res::Local(node_id)) = res {
            if *node_id == self.node_id {
                self.locations.push(path.name.span);
            }
        }
    }
}

struct FindTyParamLocations<'a> {
    param_id: hir::ty::ParamId,
    compilation: &'a Compilation,
    include_declaration: bool,
    locations: Vec<Span>,
}

impl<'a> Visitor<'_> for FindTyParamLocations<'a> {
    fn visit_callable_decl(&mut self, decl: &'_ ast::CallableDecl) {
        if self.include_declaration {
            decl.generics.iter().for_each(|p| {
                let res = self.compilation.get_res(p.id);
                if let Some(resolve::Res::Param(param_id)) = res {
                    if *param_id == self.param_id {
                        self.locations.push(p.span);
                    }
                }
            });
        }
        walk_callable_decl(self, decl);
    }

    fn visit_ty(&mut self, ty: &'_ ast::Ty) {
        if let ast::TyKind::Param(param) = &*ty.kind {
            let res = self.compilation.get_res(param.id);
            if let Some(resolve::Res::Param(param_id)) = res {
                if *param_id == self.param_id {
                    self.locations.push(param.span);
                }
            }
        } else {
            walk_ty(self, ty);
        }
    }
}
