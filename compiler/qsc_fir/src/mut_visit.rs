// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use crate::fir::{
    Block, BlockId, CallableDecl, CallableImpl, Expr, ExprId, ExprKind, FieldAssign, Ident, Item,
    ItemKind, Package, Pat, PatId, PatKind, SpecDecl, SpecImpl, Stmt, StmtId, StmtKind,
    StringComponent,
};

pub trait MutVisitor<'a>: Sized {
    fn visit_package(&mut self, package: &'a mut Package) {
        walk_package(self, package);
    }

    fn visit_item(&mut self, item: &'a mut Item) {
        walk_item(self, item);
    }

    fn visit_callable_decl(&mut self, decl: &'a mut CallableDecl) {
        walk_callable_decl(self, decl);
    }

    fn visit_callable_impl(&mut self, callable_impl: &'a mut CallableImpl) {
        walk_callable_impl(self, callable_impl);
    }

    fn visit_spec_impl(&mut self, spec_impl: &'a mut SpecImpl) {
        walk_spec_impl(self, spec_impl);
    }

    fn visit_spec_decl(&mut self, decl: &'a mut SpecDecl) {
        walk_spec_decl(self, decl);
    }

    fn visit_block(&mut self, block: BlockId) {
        walk_block(self, block);
    }

    fn visit_stmt(&mut self, stmt: StmtId) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: ExprId) {
        walk_expr(self, expr);
    }

    fn visit_pat(&mut self, pat: PatId) {
        walk_pat(self, pat);
    }

    fn visit_ident(&mut self, _: &'a mut Ident) {}

    fn get_block(&mut self, id: BlockId) -> &'a mut Block;
    fn get_expr(&mut self, id: ExprId) -> &'a mut Expr;
    fn get_pat(&mut self, id: PatId) -> &'a mut Pat;
    fn get_stmt(&mut self, id: StmtId) -> &'a mut Stmt;
}

pub fn walk_package<'a>(vis: &mut impl MutVisitor<'a>, package: &'a mut Package) {
    package.items.values_mut().for_each(|i| vis.visit_item(i));
    package.entry.iter_mut().for_each(|e| vis.visit_expr(*e));
}

pub fn walk_item<'a>(vis: &mut impl MutVisitor<'a>, item: &'a mut Item) {
    match &mut item.kind {
        ItemKind::Callable(decl) => vis.visit_callable_decl(decl),
        ItemKind::Namespace(name, _) | ItemKind::Ty(name, _) => vis.visit_ident(name),
    };
}

pub fn walk_callable_decl<'a>(vis: &mut impl MutVisitor<'a>, decl: &'a mut CallableDecl) {
    vis.visit_ident(&mut decl.name);
    vis.visit_pat(decl.input);
    vis.visit_callable_impl(&mut decl.implementation);
}

pub fn walk_callable_impl<'a>(vis: &mut impl MutVisitor<'a>, callable_impl: &'a mut CallableImpl) {
    match callable_impl {
        CallableImpl::Intrinsic => {}
        CallableImpl::Spec(spec_impl) => {
            vis.visit_spec_impl(spec_impl);
        }
        CallableImpl::SimulatableIntrinsic(spec_decl) => {
            vis.visit_spec_decl(spec_decl);
        }
    }
}

pub fn walk_spec_impl<'a>(vis: &mut impl MutVisitor<'a>, spec_impl: &'a mut SpecImpl) {
    vis.visit_spec_decl(&mut spec_impl.body);
    spec_impl
        .adj
        .iter_mut()
        .for_each(|spec| vis.visit_spec_decl(spec));
    spec_impl
        .ctl
        .iter_mut()
        .for_each(|spec| vis.visit_spec_decl(spec));
    spec_impl
        .ctl_adj
        .iter_mut()
        .for_each(|spec| vis.visit_spec_decl(spec));
}

pub fn walk_spec_decl<'a>(vis: &mut impl MutVisitor<'a>, decl: &'a mut SpecDecl) {
    decl.input.iter().for_each(|pat| vis.visit_pat(*pat));
    vis.visit_block(decl.block);
}

pub fn walk_block<'a>(vis: &mut impl MutVisitor<'a>, block: BlockId) {
    let block = vis.get_block(block);
    block.stmts.iter().for_each(|s| vis.visit_stmt(*s));
}

pub fn walk_stmt<'a>(vis: &mut impl MutVisitor<'a>, id: StmtId) {
    let stmt = vis.get_stmt(id);
    match &mut stmt.kind {
        StmtKind::Item(_) => {}
        StmtKind::Expr(expr) | StmtKind::Semi(expr) => vis.visit_expr(*expr),
        StmtKind::Local(_, pat, value) => {
            vis.visit_pat(*pat);
            vis.visit_expr(*value);
        }
    }
}

pub fn walk_expr<'a>(vis: &mut impl MutVisitor<'a>, expr: ExprId) {
    let expr = vis.get_expr(expr);
    match &expr.kind {
        ExprKind::Array(exprs) | ExprKind::ArrayLit(exprs) => {
            exprs.iter().for_each(|e| vis.visit_expr(*e));
        }
        ExprKind::ArrayRepeat(item, size) => {
            vis.visit_expr(*item);
            vis.visit_expr(*size);
        }
        ExprKind::Assign(lhs, rhs)
        | ExprKind::AssignOp(_, lhs, rhs)
        | ExprKind::BinOp(_, lhs, rhs) => {
            vis.visit_expr(*lhs);
            vis.visit_expr(*rhs);
        }
        ExprKind::AssignField(record, _, replace) | ExprKind::UpdateField(record, _, replace) => {
            vis.visit_expr(*record);
            vis.visit_expr(*replace);
        }
        ExprKind::AssignIndex(array, index, replace) => {
            vis.visit_expr(*array);
            vis.visit_expr(*index);
            vis.visit_expr(*replace);
        }
        ExprKind::Block(block) => vis.visit_block(*block),
        ExprKind::Call(callee, arg) => {
            vis.visit_expr(*callee);
            vis.visit_expr(*arg);
        }
        ExprKind::Fail(msg) => vis.visit_expr(*msg),
        ExprKind::Field(record, _) => vis.visit_expr(*record),
        ExprKind::If(cond, body, otherwise) => {
            vis.visit_expr(*cond);
            vis.visit_expr(*body);
            otherwise.iter().for_each(|e| vis.visit_expr(*e));
        }
        ExprKind::Index(array, index) => {
            vis.visit_expr(*array);
            vis.visit_expr(*index);
        }
        ExprKind::Return(expr) | ExprKind::UnOp(_, expr) => {
            vis.visit_expr(*expr);
        }
        ExprKind::Range(start, step, end) => {
            start.iter().for_each(|s| vis.visit_expr(*s));
            step.iter().for_each(|s| vis.visit_expr(*s));
            end.iter().for_each(|e| vis.visit_expr(*e));
        }
        ExprKind::Struct(_, copy, fields) => {
            copy.iter().for_each(|c| vis.visit_expr(*c));
            fields
                .iter()
                .for_each(|FieldAssign { value, .. }| vis.visit_expr(*value));
        }
        ExprKind::String(components) => {
            for component in components {
                match component {
                    StringComponent::Expr(expr) => vis.visit_expr(*expr),
                    StringComponent::Lit(_) => {}
                }
            }
        }
        ExprKind::UpdateIndex(e1, e2, e3) => {
            vis.visit_expr(*e1);
            vis.visit_expr(*e2);
            vis.visit_expr(*e3);
        }
        ExprKind::Tuple(exprs) => exprs.iter().for_each(|e| vis.visit_expr(*e)),
        ExprKind::While(cond, block) => {
            vis.visit_expr(*cond);
            vis.visit_block(*block);
        }
        ExprKind::Closure(_, _) | ExprKind::Hole | ExprKind::Lit(_) | ExprKind::Var(_, _) => {}
    }
}

pub fn walk_pat<'a>(vis: &mut impl MutVisitor<'a>, pat: PatId) {
    let pat = vis.get_pat(pat);
    match &mut pat.kind {
        PatKind::Bind(name) => vis.visit_ident(name),
        PatKind::Discard => {}
        PatKind::Tuple(pats) => pats.iter().for_each(|p| vis.visit_pat(*p)),
    }
}
