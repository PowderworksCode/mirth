//! A derive with no dependencies: `#[derive(Named)]` gives a type a `NAME`
//! constant and a `name` method, through a trait in `wide-core`.

use proc_macro::{TokenStream, TokenTree};

#[proc_macro_derive(Named, attributes(named))]
pub fn derive_named(input: TokenStream) -> TokenStream {
    let mut tokens = input.into_iter();
    let mut name = None;
    while let Some(token) = tokens.next() {
        if let TokenTree::Ident(ident) = &token {
            let word = ident.to_string();
            if word == "struct" || word == "enum" {
                name = tokens.next().map(|token| token.to_string());
                break;
            }
        }
    }
    let name = name.expect("a struct or an enum");
    format!(
        "impl ::wide_core::Named for {name} {{ const NAME: &'static str = \"{name}\"; }}"
    )
    .parse()
    .unwrap()
}

/// An attribute macro that wraps a function's body in a call counter.
#[proc_macro_attribute]
pub fn counted(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let text = item.to_string();
    let brace = text.find('{').expect("a function body");
    let (head, body) = text.split_at(brace);
    format!("{head} {{ ::wide_core::COUNT.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed); {body} }}")
        .parse()
        .unwrap()
}
