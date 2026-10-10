//! Meaning-preserving rewrites of a Rust file, for metamorphic testing of rustc: a rewritten
//! program must get the same verdict (and the same error codes) as the original.
//!
//!     mirth-rewrite <rewrite> <file.rs>
//!
//! prints the rewritten file to stdout; exits 2 when the file does not parse (as `syn` sees
//! Rust) and 3 when the rewrite does not apply to it. Comments are not kept (the tokens are
//! printed back), so line numbers change: compare verdicts and error codes, not spans.
//!
//! Rewrites:
//!
//! - `identity`: the file printed back unchanged: the baseline for the others, since printing
//!   tokens back drops comments and moves every line.
//! - `generic-wrap`: the body of each free function with plain parameters moves into a generic
//!   inner function, called with `()` for its unused type parameter. What the body does is the
//!   same; it is now checked in a generic context and instantiated.
//! - `alias`: each struct, enum and union gets a type alias with the same generic parameters,
//!   and every type mentioning it by its bare name mentions the alias instead.
//! - `reorder`: the top-level items in reverse order (item order does not matter in Rust;
//!   files with `macro_rules!` or item-position macro calls are left out, where it does).
//! - `unused`: an unused function, struct and trait added at the end.

use std::collections::{BTreeMap, BTreeSet};
use std::process::exit;

use proc_macro2::Span;
use quote::{ToTokens, format_ident, quote};
use syn::visit_mut::VisitMut;
use syn::{FnArg, GenericParam, Ident, Item, ItemFn, Pat, Type, TypePath};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: mirth-rewrite <identity|generic-wrap|alias|reorder|unused> <file.rs>");
        exit(64);
    }
    let text = std::fs::read_to_string(&args[2]).unwrap_or_else(|error| {
        eprintln!("{}: {error}", args[2]);
        exit(64)
    });
    let Ok(mut file) = syn::parse_file(&text) else { exit(2) };
    let applied = match args[1].as_str() {
        "generic-wrap" => generic_wrap(&mut file),
        "alias" => alias(&mut file),
        "reorder" => reorder(&mut file),
        "unused" => unused(&mut file),
        "identity" => true,
        other => {
            eprintln!("unknown rewrite {other}");
            exit(64)
        }
    };
    if !applied {
        exit(3);
    }
    println!("{}", file.into_token_stream());
}

/// Moves the body of `fn f(a: A, b: B) -> R { body }` into
/// `fn __mirth_inner<__MirthT>(a: A, b: B) -> R { body }`, called as `__mirth_inner::<()>(a, b)`.
fn generic_wrap(file: &mut syn::File) -> bool {
    let mut applied = false;
    for item in &mut file.items {
        if let Item::Fn(f) = item
            && wrappable(f)
        {
            wrap(f);
            applied = true;
        }
    }
    applied
}

fn wrappable(f: &ItemFn) -> bool {
    let sig = &f.sig;
    // Plain functions only: no generics (an inner fn cannot use the outer's parameters), no
    // `impl Trait`, no qualifiers that change how the body runs, no `self`, no attributes that
    // name the function (`#[test]`, `#[no_mangle]`, `#[track_caller]` would change meaning).
    sig.generics.params.is_empty()
        && sig.generics.where_clause.is_none()
        && sig.constness.is_none()
        && sig.asyncness.is_none()
        && sig.unsafety.is_none()
        && sig.abi.is_none()
        && sig.variadic.is_none()
        && f.attrs.iter().all(|a| a.path().is_ident("allow") || a.path().is_ident("inline"))
        && !sig.to_token_stream().to_string().contains("impl ")
        && !sig.to_token_stream().to_string().contains('\'')
        // Parameters without attributes: a `#[cfg]`-ed out one cannot be passed on by name.
        && sig.inputs.iter().all(|arg| matches!(arg, FnArg::Typed(t) if t.attrs.is_empty() && matches!(&*t.pat, Pat::Ident(p) if p.by_ref.is_none() && p.subpat.is_none())))
}

fn wrap(f: &mut ItemFn) {
    let sig = &f.sig;
    let inputs = &sig.inputs;
    let output = &sig.output;
    let names: Vec<&Ident> = sig
        .inputs
        .iter()
        .map(|arg| match arg {
            FnArg::Typed(t) => match &*t.pat {
                Pat::Ident(p) => &p.ident,
                _ => unreachable!("checked in wrappable"),
            },
            FnArg::Receiver(_) => unreachable!("checked in wrappable"),
        })
        .collect();
    let body = &f.block;
    // No attributes (a test may `forbid` the lint they would allow); names no lint objects to.
    let new: syn::Block = syn::parse_quote!({
        fn _mirth_inner<MirthT>(#inputs) #output #body
        _mirth_inner::<()>(#(#names),*)
    });
    // Parameters declared `mut` are mutated in the body, now the inner function's.
    for arg in f.sig.inputs.iter_mut() {
        if let FnArg::Typed(t) = arg
            && let Pat::Ident(p) = &mut *t.pat
        {
            p.mutability = None;
        }
    }
    *f.block = new;
}

/// `type __MirthAlias_S<params> = S<params>;` for each struct, enum and union `S`, and every
/// type that names `S` by its bare name names the alias instead.
fn alias(file: &mut syn::File) -> bool {
    // Names also used for generic parameters somewhere: a bare path may mean the parameter.
    struct Params(BTreeSet<String>);
    impl VisitMut for Params {
        fn visit_generic_param_mut(&mut self, p: &mut GenericParam) {
            match p {
                GenericParam::Type(t) => self.0.insert(t.ident.to_string()),
                GenericParam::Const(c) => self.0.insert(c.ident.to_string()),
                GenericParam::Lifetime(_) => false,
            };
            syn::visit_mut::visit_generic_param_mut(self, p);
        }
    }
    let mut params_seen = Params(BTreeSet::new());
    params_seen.visit_file_mut(&mut file.clone());
    // Types defined more than once (a nested item shadowing a top-level one): a bare name may
    // mean either.
    struct Defined(BTreeMap<String, usize>);
    impl VisitMut for Defined {
        fn visit_item_struct_mut(&mut self, i: &mut syn::ItemStruct) {
            *self.0.entry(i.ident.to_string()).or_default() += 1;
            syn::visit_mut::visit_item_struct_mut(self, i);
        }
        fn visit_item_enum_mut(&mut self, i: &mut syn::ItemEnum) {
            *self.0.entry(i.ident.to_string()).or_default() += 1;
            syn::visit_mut::visit_item_enum_mut(self, i);
        }
        fn visit_item_union_mut(&mut self, i: &mut syn::ItemUnion) {
            *self.0.entry(i.ident.to_string()).or_default() += 1;
            syn::visit_mut::visit_item_union_mut(self, i);
        }
        fn visit_item_type_mut(&mut self, i: &mut syn::ItemType) {
            *self.0.entry(i.ident.to_string()).or_default() += 1;
            syn::visit_mut::visit_item_type_mut(self, i);
        }
    }
    let mut defined = Defined(BTreeMap::new());
    defined.visit_file_mut(&mut file.clone());
    let mut aliases = BTreeMap::new();
    let mut new_items = Vec::new();
    for item in &file.items {
        let (ident, generics) = match item {
            Item::Struct(s) => (&s.ident, &s.generics),
            Item::Enum(e) => (&e.ident, &e.generics),
            Item::Union(u) => (&u.ident, &u.generics),
            _ => continue,
        };
        if params_seen.0.contains(&ident.to_string()) || defined.0.get(&ident.to_string()) != Some(&1) {
            continue;
        }
        // Lifetime parameters elide differently through an alias; defaults may name other
        // parameters: such types are left alone.
        if generics.params.iter().any(|p| match p {
            GenericParam::Lifetime(_) => true,
            GenericParam::Type(t) => t.default.is_some(),
            GenericParam::Const(c) => c.default.is_some(),
        }) {
            continue;
        }
        let alias = format_ident!("_MirthAlias{}", ident);
        // The type's own `cfg`s: an alias of a configured-out type would name nothing.
        let cfgs: Vec<&syn::Attribute> = match item {
            Item::Struct(x) => &x.attrs,
            Item::Enum(x) => &x.attrs,
            Item::Union(x) => &x.attrs,
            _ => unreachable!(),
        }
        .iter()
        // `cfg` only: a `cfg_attr` may expand to a `derive`, which an alias cannot have.
        .filter(|a| a.path().is_ident("cfg"))
        .collect();
        // The alias's parameters: the type's, without bounds (aliases ignore them), with defaults.
        let mut params = generics.clone();
        params.where_clause = None;
        for p in params.params.iter_mut() {
            match p {
                GenericParam::Type(t) => {
                    // `?Sized` must stay: without it the alias requires `Sized`.
                    let maybe: Vec<syn::TypeParamBound> = t
                        .bounds
                        .iter()
                        .filter(|b| matches!(b, syn::TypeParamBound::Trait(tb) if matches!(tb.modifier, syn::TraitBoundModifier::Maybe(_))))
                        .cloned()
                        .collect();
                    t.bounds = maybe.into_iter().collect();
                    if t.bounds.is_empty() {
                        t.colon_token = None;
                    }
                }
                GenericParam::Lifetime(l) => {
                    l.bounds.clear();
                    l.colon_token = None;
                }
                GenericParam::Const(_) => {}
            }
        }
        let args: Vec<proc_macro2::TokenStream> = generics
            .params
            .iter()
            .map(|p| match p {
                GenericParam::Type(t) => t.ident.to_token_stream(),
                GenericParam::Lifetime(l) => l.lifetime.to_token_stream(),
                GenericParam::Const(c) => c.ident.to_token_stream(),
            })
            .collect();
        let target = if args.is_empty() { quote!(#ident) } else { quote!(#ident<#(#args),*>) };
        new_items.push(syn::parse_quote!(
            #(#cfgs)*
            type #alias #params = #target;
        ));
        aliases.insert(ident.to_string(), alias);
    }
    if aliases.is_empty() {
        return false;
    }
    struct Rename<'a> {
        aliases: &'a BTreeMap<String, Ident>,
        count: usize,
    }
    impl VisitMut for Rename<'_> {
        fn visit_type_path_mut(&mut self, ty: &mut TypePath) {
            if ty.qself.is_none()
                && ty.path.leading_colon.is_none()
                && ty.path.segments.len() == 1
                && let Some(alias) = self.aliases.get(&ty.path.segments[0].ident.to_string())
            {
                ty.path.segments[0].ident = Ident::new(&alias.to_string(), Span::call_site());
                self.count += 1;
            }
            syn::visit_mut::visit_type_path_mut(self, ty);
        }
        // Inside a type's own definition, `Self`-like uses must stay (a recursive type through
        // its alias would be a cycle in the alias), so definitions are not visited.
        fn visit_item_struct_mut(&mut self, _: &mut syn::ItemStruct) {}
        fn visit_item_enum_mut(&mut self, _: &mut syn::ItemEnum) {}
        fn visit_item_union_mut(&mut self, _: &mut syn::ItemUnion) {}
        // Macros' tokens are not types to `syn`; derive input stays as it is.
        fn visit_macro_mut(&mut self, _: &mut syn::Macro) {}
        // In an inline module the bare name reaches the type through a `use`, the alias would
        // need one too: modules are left as they are.
        fn visit_item_mod_mut(&mut self, _: &mut syn::ItemMod) {}
        // A receiver typed with the impl's own name (`self: &mut Test`) elides lifetimes like
        // `&mut self`; through an alias it does not (resolution does not see through aliases).
        fn visit_receiver_mut(&mut self, _: &mut syn::Receiver) {}
        // The same rule compares a receiver with the impl's self type: that stays as written.
        fn visit_item_impl_mut(&mut self, imp: &mut syn::ItemImpl) {
            let self_ty = std::mem::replace(&mut *imp.self_ty, Type::Verbatim(Default::default()));
            syn::visit_mut::visit_item_impl_mut(self, imp);
            *imp.self_ty = self_ty;
        }
    }
    let mut rename = Rename { aliases: &aliases, count: 0 };
    for item in &mut file.items {
        rename.visit_item_mut(item);
    }
    if rename.count == 0 {
        return false;
    }
    file.items.extend(new_items);
    true
}

fn reorder(file: &mut syn::File) -> bool {
    let has_macros = file.items.iter().any(|item| matches!(item, Item::Macro(_)));
    if has_macros || file.items.len() < 2 {
        return false;
    }
    // `use` and `extern crate` first, as written, so that preludes and `#[macro_use]` stay put.
    let (mut head, mut rest): (Vec<Item>, Vec<Item>) =
        file.items.drain(..).partition(|item| matches!(item, Item::Use(_) | Item::ExternCrate(_)));
    rest.reverse();
    head.extend(rest);
    file.items = head;
    true
}

fn unused(file: &mut syn::File) -> bool {
    let names: BTreeSet<String> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(f) => Some(f.sig.ident.to_string()),
            _ => None,
        })
        .collect();
    if names.contains("_mirth_unused") {
        return false;
    }
    // Leading underscores keep `dead_code` quiet without an attribute a test could `forbid`.
    file.items.push(syn::parse_quote!(fn _mirth_unused() {}));
    file.items.push(syn::parse_quote!(struct _MirthUnused;));
    file.items.push(syn::parse_quote!(trait _MirthUnusedTrait {}));
    true
}
