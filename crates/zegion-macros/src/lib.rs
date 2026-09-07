//! `#[zegion_tool]`: wraps `#[aisdk::macros::tool]` and additionally emits a
//! companion `*_output_schema()` function describing the tool's return type,
//! so tool outputs can be validated/typed like inputs.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ItemFn, Pat, ReturnType};

#[proc_macro_attribute]
pub fn zegion_tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;
    let attrs = &input.attrs;

    let fn_name = &sig.ident;
    let output_schema_fn = syn::Ident::new(&format!("{}_output_schema", fn_name), fn_name.span());

    let return_ty = match &sig.output {
        ReturnType::Type(_, ty) => quote!(#ty),
        ReturnType::Default => quote!(()),
    };

    // Extract parameter names/types for documentation of the generated schema fn.
    let _params: Vec<_> = sig
        .inputs
        .iter()
        .filter_map(|a| match a {
            FnArg::Typed(pt) => match &*pt.pat {
                Pat::Ident(id) => Some(id.ident.clone()),
                _ => None,
            },
            FnArg::Receiver(_) => None,
        })
        .collect();

    let expanded = quote! {
        #(#attrs)*
        #[::aisdk::macros::tool]
        #vis #sig #block

        /// Returns the JSON schema name of this tool's return type.
        #[allow(dead_code)]
        #vis fn #output_schema_fn() -> &'static str {
            ::core::stringify!(#return_ty)
        }
    };

    // Forward any aisdk attr tokens untouched (currently unused).
    let _ = attr;
    expanded.into()
}
