//! Procedural macros for describing and driving UI primitives.

mod action;

mod derive;

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream as Tokens;
use quote::quote;
use syn::{Path, parse_macro_input};

fn core_path() -> syn::Result<Tokens> {
    for (package, canonical) in [("rust-use", "rust_use"), ("rust-use-core", "rust_use_core")] {
        if let Ok(found) = crate_name(package) {
            let name = match found {
                FoundCrate::Itself => canonical.to_owned(),
                FoundCrate::Name(name) => name,
            };
            let path: Path = syn::parse_str(&format!("::{name}"))?;
            return Ok(if package == "rust-use" {
                quote!(#path::__private::core)
            } else {
                quote!(#path)
            });
        }
    }
    Err(syn::Error::new(
        proc_macro2::Span::call_site(),
        "add a dependency on rust-use or rust-use-core",
    ))
}

/// Describe a named-field or unit struct as an agent-visible primitive.
///
/// ```ignore
/// use rust_use::prelude::*;
/// #[derive(Primitive)]
/// #[primitive(no_actions)]
/// struct Meter { #[agent(value)] level: u8 }
/// assert_eq!(Meter::NAME, "meter");
/// ```
#[proc_macro_derive(Primitive, attributes(primitive, agent, behavior))]
pub fn primitive(input: TokenStream) -> TokenStream {
    derive::expand(&parse_macro_input!(input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Expose documented mutable methods as positional actions.
///
/// ```ignore
/// use rust_use::prelude::*;
/// #[derive(Primitive)]
/// struct Counter { count: u32 }
/// #[actions]
/// impl Counter {
///     /// Advance the counter.
///     fn advance(&mut self, by: u32) -> u32 { self.count += by; self.count }
/// }
/// assert_eq!(Counter { count: 0 }.actions()[0].name, "advance");
/// ```
#[proc_macro_attribute]
pub fn actions(attr: TokenStream, input: TokenStream) -> TokenStream {
    parse_macro_input!(attr as syn::parse::Nothing);
    action::expand(parse_macro_input!(input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
