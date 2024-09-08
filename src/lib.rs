mod import_analysis;

use crate::import_analysis::{ImportAnalysis, ImportSpecification};
use std::rc::Rc;
use swc_core::atoms::{atom, Atom};
use swc_core::common::util::take::Take;
use swc_core::ecma::ast::{
    ArrayLit, ArrayPat, BindingIdent, CallExpr, Callee, ClassDecl, Decl, Expr, ExprOrSpread,
    Function, Ident, Pat, Stmt, VarDecl, VarDeclKind, VarDeclarator,
};
use swc_core::ecma::visit::{VisitMutWith, VisitWith};
use swc_core::ecma::{
    ast::Program,
    visit::{as_folder, FoldWith, VisitMut},
};
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};

struct ActiveReplacement {
    import: Rc<ImportSpecification>,
    symbol: Atom,
}

#[derive(Default)]
pub struct TransformVisitor {
    imports: Vec<Rc<ImportSpecification>>,
    active_replacements: Vec<ActiveReplacement>,
    is_in_replaceable_scope: bool,
    current_scope_symbol: Option<Atom>,
}

impl VisitMut for TransformVisitor {
    fn visit_mut_program(&mut self, node: &mut Program) {
        let mut import_analysis = ImportAnalysis::new();
        node.visit_with(&mut import_analysis);
        let imports = import_analysis.into_import_specifications();
        self.imports = imports.into_iter().map(Rc::new).collect();
        node.visit_mut_children_with(self);
    }

    fn visit_mut_class_decl(&mut self, node: &mut ClassDecl) {
        self.current_scope_symbol = Some(node.ident.sym.clone());
        node.visit_mut_children_with(self);
        self.current_scope_symbol = None;
    }

    fn visit_mut_function(&mut self, node: &mut Function) {
        let Some(body) = &mut node.body else { return };
        let Some(current_scope_symbol) = self.current_scope_symbol.clone() else {
            return;
        };

        self.is_in_replaceable_scope = true;
        body.visit_mut_children_with(self);
        self.is_in_replaceable_scope = false;
        let active_replacements = self.active_replacements.take();
        let mut new_statements = vec![];
        for replacement in active_replacements {
            new_statements.push(Stmt::Decl(Decl::Var(Box::new(VarDecl {
                span: Default::default(),
                ctxt: Default::default(),
                kind: VarDeclKind::Const,
                declare: false,
                decls: vec![VarDeclarator {
                    span: Default::default(),
                    name: Pat::Array(ArrayPat {
                        span: Default::default(),
                        elems: vec![Some(Pat::Ident(BindingIdent::from(Ident {
                            span: Default::default(),
                            ctxt: Default::default(),
                            sym: replacement.symbol,
                            optional: false,
                        })))],
                        optional: false,
                        type_ann: None,
                    }),
                    init: Some(Box::new(Expr::Call(CallExpr {
                        span: Default::default(),
                        ctxt: Default::default(),
                        callee: Callee::Expr(Box::new(Expr::Ident(Ident {
                            span: Default::default(),
                            ctxt: Default::default(),
                            sym: atom!("_di"),
                            optional: false,
                        }))),
                        args: vec![
                            ExprOrSpread::from(Expr::Array(ArrayLit {
                                span: Default::default(),
                                elems: vec![Some(ExprOrSpread::from(Expr::Ident(Ident {
                                    span: Default::default(),
                                    ctxt: Default::default(),
                                    sym: replacement.import.local_imported_symbol.clone(),
                                    optional: false,
                                })))],
                            })),
                            ExprOrSpread::from(Expr::Ident(Ident {
                                span: Default::default(),
                                ctxt: Default::default(),
                                sym: current_scope_symbol.clone(),
                                optional: false,
                            })),
                        ],
                        type_args: None,
                    }))),
                    definite: false,
                }],
            }))));
        }

        body.stmts = new_statements
            .into_iter()
            .chain(body.stmts.iter().cloned())
            .collect();
    }

    fn visit_mut_ident(&mut self, node: &mut Ident) {
        if !self.is_in_replaceable_scope {
            return;
        }

        let node_id = node.to_id();
        let Some(import) = self.imports.iter().find(|spec| spec.symbol_id == node_id) else {
            return;
        };

        let new_symbol = format!("_{}", import.local_imported_symbol.to_string());
        let new_symbol = Atom::new(new_symbol);
        node.sym = new_symbol.clone();
        self.active_replacements.push(ActiveReplacement {
            symbol: new_symbol,
            import: import.clone(),
        });
    }
}

/// An example plugin function with macro support.
/// `plugin_transform` macro interop pointers into deserialized structs, as well
/// as returning ptr back to host.
///
/// It is possible to opt out from macro by writing transform fn manually
/// if plugin need to handle low-level ptr directly via
/// `__transform_plugin_process_impl(
///     ast_ptr: *const u8, ast_ptr_len: i32,
///     unresolved_mark: u32, should_enable_comments_proxy: i32) ->
///     i32 /*  0 for success, fail otherwise.
///             Note this is only for internal pointer interop result,
///             not actual transform result */`
///
/// This requires manual handling of serialization / deserialization from ptrs.
/// Refer swc_plugin_macro to see how does it work internally.
#[plugin_transform]
pub fn process_transform(program: Program, _metadata: TransformPluginProgramMetadata) -> Program {
    program.fold_with(&mut as_folder(TransformVisitor::default()))
}

// An example to test plugin transform.
// Recommended strategy to test plugin's transform is verify
// the Visitor's behavior, instead of trying to run `process_transform` with mocks
// unless explicitly required to do so.
#[cfg(test)]
mod test {
    use super::*;
    use swc_core::ecma::transforms::testing::test_inline_input_output;
    use swc_core::ecma::visit::as_folder;
    use swc_ecma_parser::{EsSyntax, Syntax};

    #[test]
    fn test_should_work_in_class_components() {
        test_inline_input_output(
            Syntax::Es(EsSyntax {
                jsx: true,
                ..Default::default()
            }),
            |_| as_folder(TransformVisitor::default()),
            // Input codes
            r#"
import React, { Component } from 'react';
import Modal from 'modal';

class MyComponent extends Component {
    render() {
        return <Modal />;
    }
}"#,
            // Output codes after transformed with plugin
            r#"
import React, { Component } from 'react';
import Modal from 'modal';

class MyComponent extends Component {
    render() {
        const [_Modal] = _di([Modal], MyComponent);
        return <_Modal />;
    }
}"#,
        );
    }
}
