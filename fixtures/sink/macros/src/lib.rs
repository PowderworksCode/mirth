//! Procedural macros without dependencies: a derive with a helper attribute,
//! an attribute macro and a function-like macro.

use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};

fn item_name(input: TokenStream) -> (String, usize) {
    let mut tokens = input.into_iter().peekable();
    let mut fields = 0;
    let mut name = None;
    while let Some(token) = tokens.next() {
        match &token {
            TokenTree::Ident(ident) if name.is_none() => {
                let word = ident.to_string();
                if word == "struct" || word == "enum" {
                    name = tokens.next().map(|t| t.to_string());
                }
            }
            TokenTree::Group(group) if name.is_some() && group.delimiter() == Delimiter::Brace => {
                fields = group.stream().into_iter().filter(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == ':')).count();
            }
            _ => {}
        }
    }
    (name.expect("a struct or an enum"), fields)
}

/// `#[derive(Describe)]` implements `sink_core::Describe`, with the name, the
/// number of named fields, and an optional `#[describe(tag = "...")]`.
#[proc_macro_derive(Describe, attributes(describe))]
pub fn derive_describe(input: TokenStream) -> TokenStream {
    let text = input.to_string();
    let tag = text
        .split("tag = \"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or("untagged")
        .to_string();
    let (name, fields) = item_name(input);
    format!(
        "impl ::sink_core::Describe for {name} {{
            const NAME: &'static str = \"{name}\";
            const FIELDS: usize = {fields};
            fn tag(&self) -> &'static str {{ \"{tag}\" }}
        }}"
    )
    .parse()
    .unwrap()
}

/// `#[traced]` makes a function count its calls in `sink_core::TRACE`.
#[proc_macro_attribute]
pub fn traced(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut out: Vec<TokenTree> = item.into_iter().collect();
    if let Some(TokenTree::Group(body)) = out.pop() {
        let prefix: TokenStream =
            "::sink_core::TRACE.fetch_add(1, ::core::sync::atomic::Ordering::Relaxed);".parse().unwrap();
        let mut stream = prefix;
        stream.extend(body.stream());
        let mut group = Group::new(Delimiter::Brace, stream);
        group.set_span(body.span());
        out.push(TokenTree::Group(group));
    }
    out.into_iter().collect()
}

/// `squares!(a, b, c)` expands to an array of the squares, built token by token.
#[proc_macro]
pub fn squares(input: TokenStream) -> TokenStream {
    let mut items = Vec::new();
    for token in input {
        if let TokenTree::Literal(lit) = token {
            let n: i64 = lit.to_string().trim_end_matches("i64").parse().unwrap();
            items.push(TokenTree::Literal(Literal::i64_suffixed(n * n)));
            items.push(TokenTree::Punct(Punct::new(',', Spacing::Alone)));
        }
    }
    let group = Group::new(Delimiter::Bracket, items.into_iter().collect());
    let _ = Ident::new("unused", Span::call_site());
    TokenStream::from(TokenTree::Group(group))
}
