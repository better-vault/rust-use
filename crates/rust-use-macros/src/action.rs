use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Attribute, Error, Expr, FnArg, GenericArgument, ImplItem, ImplItemFn, ItemImpl, Lit, LitStr,
    Meta, Pat, Path, PathArguments, PathSegment, ReturnType, Type, ext::IdentExt,
};

#[derive(Default)]
struct Options {
    skip: bool,
    enabled: Option<Path>,
    name: Option<LitStr>,
}

fn options(attrs: &mut Vec<Attribute>) -> syn::Result<Options> {
    let mut opts = Options::default();
    for attr in attrs.iter().filter(|a| a.path().is_ident("action")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                opts.skip = true;
            } else if meta.path.is_ident("enabled") {
                opts.enabled = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("name") {
                opts.name = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error("unknown action option"));
            }
            Ok(())
        })?;
    }
    attrs.retain(|a| !a.path().is_ident("action"));
    Ok(opts)
}

fn doc(method: &ImplItemFn) -> syn::Result<String> {
    let mut words = Vec::new();
    for attr in method.attrs.iter().filter(|a| a.path().is_ident("doc")) {
        if let Meta::NameValue(meta) = &attr.meta
            && let Expr::Lit(expr) = &meta.value
            && let Lit::Str(lit) = &expr.lit
        {
            for line in lit.value().split('\n') {
                if line.trim().is_empty() && !words.is_empty() {
                    return Ok(words.join(" "));
                }
                words.extend(line.split_whitespace().map(str::to_owned));
            }
        }
    }
    if words.is_empty() {
        return Err(Error::new_spanned(
            &method.sig.ident,
            "document this action: its doc comment is what the agent reads",
        ));
    }
    Ok(words.join(" "))
}

fn segment(ty: &Type) -> Option<&PathSegment> {
    if let Type::Path(path) = ty {
        path.path.segments.last()
    } else {
        None
    }
}

fn option_inner(ty: &Type) -> Option<&Type> {
    let seg = segment(ty)?;
    if seg.ident != "Option" {
        return None;
    }
    if let PathArguments::AngleBracketed(args) = &seg.arguments
        && let Some(GenericArgument::Type(inner)) = args.args.first()
    {
        return Some(inner);
    }
    None
}

fn type_name(ty: &Type) -> syn::Result<&'static str> {
    if let Type::Reference(r) = ty
        && segment(&r.elem).is_some_and(|s| s.ident == "str")
    {
        return Err(Error::new_spanned(
            ty,
            "use String instead of &str for action parameters",
        ));
    }
    Ok(match segment(ty).map(|s| s.ident.to_string()).as_deref() {
        Some("bool") => "bool",
        Some(
            "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
            | "usize",
        ) => "int",
        Some("f32" | "f64") => "float",
        Some("String") => "str",
        Some("Vec") => "list",
        _ => "any",
    })
}

struct Generated {
    spec: TokenStream,
    enabled: TokenStream,
    invoke: TokenStream,
}

fn parameters(
    method: &ImplItemFn,
    core: &TokenStream,
) -> syn::Result<(Vec<TokenStream>, Vec<TokenStream>)> {
    let mut params = Vec::new();
    let mut reads = Vec::new();
    for (idx, arg) in method.sig.inputs.iter().skip(1).enumerate() {
        let FnArg::Typed(arg) = arg else {
            return Err(Error::new_spanned(arg, "expected a named parameter"));
        };
        let Pat::Ident(pat) = &*arg.pat else {
            return Err(Error::new_spanned(
                &arg.pat,
                "action parameters need a name",
            ));
        };
        let label = pat.ident.unraw().to_string();
        let inner = option_inner(&arg.ty);
        let optional = inner.is_some();
        let decoder = if optional {
            quote!(get_opt)
        } else {
            quote!(get)
        };
        let ty = inner.unwrap_or(&arg.ty);
        let short = type_name(ty)?;
        params.push(quote!(#core::ParamSpec { name: #label, ty: #short, optional: #optional }));
        reads.push(quote!(args.#decoder::<#ty>(#idx, #label)?));
    }
    Ok((params, reads))
}

fn generate(method: &ImplItemFn, opts: Options, c: &TokenStream) -> syn::Result<Generated> {
    let sig = &method.sig;
    if let Some(token) = sig.asyncness {
        return Err(Error::new_spanned(token, "async actions are not supported"));
    }
    if !sig.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &sig.generics,
            "generic actions are not supported",
        ));
    }
    let id = &sig.ident;
    let name = opts
        .name
        .map_or_else(|| id.unraw().to_string(), |s| s.value());
    let doc = doc(method)?;
    let (params, reads) = parameters(method, c)?;
    let count = params.len();
    let cfg: Vec<_> = method
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("cfg"))
        .collect();
    let call = quote!(self.#id(#(#reads),*));
    let value = match &sig.output {
        ReturnType::Type(_, ty) if segment(ty).is_some_and(|s| s.ident == "Result") => {
            quote! {
                let value = #call.map_err(::core::convert::Into::<#c::ActionError>::into)?;
                Ok(#c::__private::serde_json::to_value(value).unwrap_or(#c::Value::Null))
            }
        }
        ReturnType::Default => quote! { #call; Ok(#c::Value::Null) },
        ReturnType::Type(_, ty) if matches!(ty.as_ref(), Type::Tuple(t) if t.elems.is_empty()) => {
            quote! { #call; Ok(#c::Value::Null) }
        }
        ReturnType::Type(..) => {
            quote! { Ok(#c::__private::serde_json::to_value(#call).unwrap_or(#c::Value::Null)) }
        }
    };
    let enabled = opts
        .enabled
        .map_or_else(|| quote!(true), |path| quote!(#path(self)));
    Ok(Generated {
        spec: quote!(#(#cfg)* #c::ActionSpec {name: #name, doc: #doc, params: &[#(#params),*]}),
        enabled: quote!(#(#cfg)* #name => Some(#enabled),),
        invoke: quote!(#(#cfg)* #name => Some((|| {
            if args.len() > #count {
                return Err(#c::ActionError::InvalidArg {name: "args", reason: ::std::format!("expected {}, got {}", #count, args.len())});
            }
            #value
        })()),),
    })
}

pub(super) fn expand(mut block: ItemImpl) -> syn::Result<TokenStream> {
    if block.trait_.is_some() {
        return Err(Error::new_spanned(
            &block.self_ty,
            "#[actions] needs an inherent impl",
        ));
    }
    let c = super::core_path()?;
    let mut specs = Vec::new();
    let mut enabled = Vec::new();
    let mut invokes = Vec::new();
    for item in &mut block.items {
        if let ImplItem::Fn(method) = item {
            let opts = options(&mut method.attrs)?;
            if !opts.skip
                && method
                    .sig
                    .receiver()
                    .is_some_and(|r| r.reference.is_some() && r.mutability.is_some())
            {
                let action = generate(method, opts, &c)?;
                specs.push(action.spec);
                enabled.push(action.enabled);
                invokes.push(action.invoke);
            }
        }
    }
    let ty = &block.self_ty;
    let (imp, _, bounds) = block.generics.split_for_impl();
    Ok(quote! {
        #block
        impl #imp #c::__private::OwnActions for #ty #bounds {
            const ACTIONS: &'static [#c::ActionSpec] = &[#(#specs),*];
            fn own_is_enabled(&self, action: &str) -> ::core::option::Option<bool> { match action {#(#enabled)* _=>None} }
            fn own_invoke(&mut self, action: &str, args: &#c::Args) -> ::core::option::Option<::core::result::Result<#c::Value, #c::ActionError>> {
                match action { #(#invokes)* _=>None }
            }
        }
    })
}
