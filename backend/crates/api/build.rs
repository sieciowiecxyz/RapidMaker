use std::{collections::BTreeSet, fs, path::Path};
use syn::{Expr, Item, visit::Visit};

const METHODS: &[&str] = &[
    "get", "post", "put", "delete", "patch", "head", "options", "trace",
];

#[derive(Default)]
struct Routes {
    entries: BTreeSet<(String, String)>,
    error: Option<String>,
}

fn methods(expr: &Expr, found: &mut Vec<String>) -> Result<(), String> {
    match expr {
        Expr::Paren(expr) => methods(&expr.expr, found),
        Expr::Group(expr) => methods(&expr.expr, found),
        Expr::Call(call) => {
            let Expr::Path(function) = &*call.func else {
                return Err("route method must be an Axum routing function".into());
            };
            let path: Vec<_> = function
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            let Some(name) = path.last() else {
                return Err("empty routing function path".into());
            };
            if call.args.len() != 1
                || !METHODS.contains(&name.as_str())
                || !(path.len() == 1 || (path.len() == 3 && path[..2] == ["axum", "routing"]))
            {
                return Err(format!("unsupported routing function {}", path.join("::")));
            }
            found.push(name.to_uppercase());
            Ok(())
        }
        Expr::MethodCall(call) => {
            let name = call.method.to_string();
            if call.args.len() != 1 {
                return Err(format!("unsupported routing call {name}"));
            }
            if METHODS.contains(&name.as_str()) {
                found.push(name.to_uppercase());
            } else if !["layer", "route_layer", "with_state"].contains(&name.as_str()) {
                return Err(format!("unsupported method-router modifier {name}"));
            }
            methods(&call.receiver, found)
        }
        _ => Err("route method must be a literal Axum method-router expression".into()),
    }
}

fn test_only(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && attribute
                .parse_args::<syn::Path>()
                .is_ok_and(|path| path.is_ident("test"))
    })
}

impl<'ast> Visit<'ast> for Routes {
    fn visit_file(&mut self, file: &'ast syn::File) {
        if !test_only(&file.attrs) {
            syn::visit::visit_file(self, file);
        }
    }

    fn visit_item(&mut self, item: &'ast Item) {
        let attributes = match item {
            Item::Const(i) => &i.attrs,
            Item::Enum(i) => &i.attrs,
            Item::ExternCrate(i) => &i.attrs,
            Item::Fn(i) => &i.attrs,
            Item::ForeignMod(i) => &i.attrs,
            Item::Impl(i) => &i.attrs,
            Item::Macro(i) => &i.attrs,
            Item::Mod(i) => &i.attrs,
            Item::Static(i) => &i.attrs,
            Item::Struct(i) => &i.attrs,
            Item::Trait(i) => &i.attrs,
            Item::TraitAlias(i) => &i.attrs,
            Item::Type(i) => &i.attrs,
            Item::Union(i) => &i.attrs,
            Item::Use(i) => &i.attrs,
            _ => &[][..],
        };
        if !test_only(attributes) {
            syn::visit::visit_item(self, item);
        }
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if self.error.is_some() {
            return;
        }
        let name = call.method.to_string();
        if ["nest", "nest_service", "route_service"].contains(&name.as_str()) {
            self.error = Some(format!(
                "{name} requires extending the API inventory generator"
            ));
            return;
        }
        if name == "route" {
            let result = (|| {
                if call.args.len() != 2 {
                    return Err("route requires a literal path and a method router".into());
                }
                let Expr::Lit(path) = &call.args[0] else {
                    return Err("route path must be a string literal".into());
                };
                let syn::Lit::Str(path) = &path.lit else {
                    return Err("route path must be a string literal".into());
                };
                let path = path.value();
                if path != "/api" && !path.starts_with("/api/") {
                    return Err(format!("unsupported route outside /api: {path}"));
                }
                let mut found = Vec::new();
                methods(&call.args[1], &mut found)?;
                for method in found {
                    self.entries.insert((path.clone(), method));
                }
                Ok(())
            })();
            if let Err(error) = result {
                self.error = Some(error);
                return;
            }
        }
        syn::visit::visit_expr_method_call(self, call);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn methods_are_taken_only_from_the_registration_chain() {
        let file = syn::parse_file(
            r#"
            fn router() {
                Router::new().route("/api/a", get(|| client.post("url")).put(save).layer(layer))
                    .route("/api/b", axum::routing::delete(remove));
            }
            #[cfg(test)] mod tests { fn ignored() { router.route(dynamic, unknown); } }
        "#,
        )
        .unwrap();
        let mut routes = Routes::default();
        routes.visit_file(&file);
        assert!(routes.error.is_none());
        assert_eq!(
            routes.entries.into_iter().collect::<Vec<_>>(),
            vec![
                ("/api/a".into(), "GET".into()),
                ("/api/a".into(), "PUT".into()),
                ("/api/b".into(), "DELETE".into()),
            ]
        );
    }

    #[test]
    fn unsupported_registrations_fail_instead_of_disappearing() {
        for expression in [
            "router.route(dynamic, get(handler))",
            "router.route(\"/api/a\", methods)",
            "router.route(\"/api/a\", any(handler))",
            "router.nest(\"/api\", router)",
        ] {
            let mut routes = Routes::default();
            routes.visit_expr(&syn::parse_str::<Expr>(expression).unwrap());
            assert!(routes.error.is_some(), "{expression}");
        }
    }
}

fn scan(directory: &Path, routes: &mut Routes) -> Result<(), String> {
    println!("cargo:rerun-if-changed={}", directory.display());
    let mut entries = fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            scan(&path, routes)?;
        } else if kind.is_file() && path.extension().is_some_and(|e| e == "rs") {
            println!("cargo:rerun-if-changed={}", path.display());
            let source =
                fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let parsed =
                syn::parse_file(&source).map_err(|e| format!("{}: {e}", path.display()))?;
            routes.visit_file(&parsed);
            if let Some(error) = routes.error.take() {
                return Err(format!("{}: {error}", path.display()));
            }
        }
    }
    Ok(())
}

fn main() {
    let mut routes = Routes::default();
    scan(Path::new("src"), &mut routes)
        .unwrap_or_else(|error| panic!("API route inventory: {error}"));
    let mut generated = String::from("pub(super) const ROUTES: &[(&str, &str)] = &[\n");
    for (path, method) in routes.entries {
        generated.push_str(&format!("    ({method:?}, {path:?}),\n"));
    }
    generated.push_str("];\n");
    let output = std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR");
    fs::write(Path::new(&output).join("api_routes.rs"), generated)
        .expect("write API route inventory");
}
