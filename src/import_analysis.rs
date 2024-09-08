use swc_core::atoms::Atom;
use swc_core::ecma::ast::{Id, ImportDecl, ImportSpecifier};
use swc_core::ecma::visit::Visit;

#[allow(unused)]
pub struct ImportSpecification {
    pub symbol_id: Id,
    pub local_imported_symbol: Atom,
    pub dependency_imported_symbol: Atom,
    pub package_name: Atom,
    pub is_type_only: bool,
}

/// Traverse module to get all imported symbol `Id` values
pub struct ImportAnalysis {
    import_specifications: Vec<ImportSpecification>,
}

impl ImportAnalysis {
    pub fn new() -> Self {
        Self {
            import_specifications: vec![],
        }
    }

    pub fn into_import_specifications(self) -> Vec<ImportSpecification> {
        self.import_specifications
    }
}

impl Visit for ImportAnalysis {
    fn visit_import_decl(&mut self, node: &ImportDecl) {
        if node.type_only {
            return;
        }

        let package_name = &node.src.value;

        for specifier in &node.specifiers {
            match specifier {
                // import { x }
                ImportSpecifier::Named(named) => {
                    let symbol_id = named.local.to_id();
                    let local_imported_symbol = named.local.sym.clone();
                    let dependency_imported_symbol = named
                        .imported
                        .as_ref()
                        .map(|s| s.atom())
                        .cloned()
                        .unwrap_or(named.local.sym.clone());
                    self.import_specifications.push(ImportSpecification {
                        symbol_id,
                        local_imported_symbol,
                        dependency_imported_symbol,
                        package_name: package_name.clone(),
                        is_type_only: named.is_type_only,
                    });
                }
                // import defaultExport
                ImportSpecifier::Default(default_import) => {
                    let symbol_id = default_import.local.to_id();
                    let local_imported_symbol = default_import.local.sym.clone();
                    let dependency_imported_symbol = local_imported_symbol.clone();
                    self.import_specifications.push(ImportSpecification {
                        symbol_id,
                        local_imported_symbol,
                        dependency_imported_symbol,
                        package_name: package_name.clone(),
                        is_type_only: node.type_only,
                    });
                }
                // import *
                ImportSpecifier::Namespace(namespace_import) => {
                    let symbol_id = namespace_import.local.to_id();
                    let local_imported_symbol = namespace_import.local.sym.clone();
                    let dependency_imported_symbol = local_imported_symbol.clone();
                    self.import_specifications.push(ImportSpecification {
                        symbol_id,
                        local_imported_symbol,
                        dependency_imported_symbol,
                        package_name: package_name.clone(),
                        is_type_only: node.type_only,
                    });
                }
            }
        }
    }
}
