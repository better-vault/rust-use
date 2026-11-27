use heck::ToKebabCase;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{
    Data, DeriveInput, Error, Field, Fields, Ident, LitStr, Path, ext::IdentExt, spanned::Spanned,
};

#[derive(Default)]
struct Options {
    name: Option<LitStr>,
    root: Option<Path>,
    no_actions: bool,
}

impl Options {
    fn parse(input: &DeriveInput) -> syn::Result<Self> {
        let mut options = Self::default();
        for attr in input
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("primitive"))
        {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    options.name = Some(meta.value()?.parse()?);
                } else if meta.path.is_ident("crate") {
                    options.root = Some(meta.value()?.parse::<LitStr>()?.parse()?);
                } else if meta.path.is_ident("no_actions") {
                    options.no_actions = true;
                } else {
                    return Err(meta.error("unknown primitive option"));
                }
                Ok(())
            })?;
        }
        Ok(options)
    }
}

#[derive(Default)]
struct Description<'a> {
    fields: Vec<TokenStream>,
    behaviors: Vec<&'a Ident>,
    has_value: bool,
}

impl<'a> Description<'a> {
    fn field(&mut self, field: &'a Field) -> syn::Result<()> {
        let Some(id) = &field.ident else {
            return Err(Error::new_spanned(
                field,
                "tuple structs cannot derive Primitive",
            ));
        };
        let mut name = id.unraw().to_string();
        let mut mode = None;
        for attr in &field.attrs {
            if attr.path().is_ident("behavior") {
                attr.meta.require_path_only()?;
                if mode.replace("behavior").is_some() {
                    return Err(Error::new_spanned(attr, "conflicting field annotations"));
                }
            } else if attr.path().is_ident("agent") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename") {
                        name = meta.value()?.parse::<LitStr>()?.value();
                        return Ok(());
                    }
                    let key = ["value", "flag", "skip"]
                        .into_iter()
                        .find(|s| meta.path.is_ident(s))
                        .ok_or_else(|| meta.error("unknown agent field option"))?;
                    if mode.replace(key).is_some() {
                        return Err(meta.error("conflicting field annotations"));
                    }
                    Ok(())
                })?;
            }
        }
        let span = id.span();
        match mode {
            Some("value") => {
                if self.has_value {
                    return Err(Error::new(span, "only one field may have #[agent(value)]"));
                }
                self.has_value = true;
                self.fields
                    .push(quote_spanned!(span=> out.value(&self.#id);));
            }
            Some("behavior") => self.behaviors.push(id),
            Some("flag") => self
                .fields
                .push(quote_spanned!(span=> out.flag(#name,self.#id);)),
            Some("skip") => (),
            _ => self
                .fields
                .push(quote_spanned!(span=> out.attr(#name,&self.#id);)),
        }
        Ok(())
    }
}

pub(super) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let opts = Options::parse(input)?;
    let id = &input.ident;
    let Data::Struct(data) = &input.data else {
        return Err(Error::new(
            id.span(),
            "Primitive can only be derived for structs",
        ));
    };
    if let Fields::Unnamed(fields) = &data.fields {
        return Err(Error::new(
            fields.span(),
            "tuple structs cannot derive Primitive",
        ));
    }
    let mut description = Description::default();
    for field in &data.fields {
        description.field(field)?;
    }
    let Description {
        fields, behaviors, ..
    } = description;
    let name = opts
        .name
        .map_or_else(|| id.unraw().to_string().to_kebab_case(), |s| s.value());
    let c = if let Some(path) = opts.root {
        quote!(#path)
    } else {
        super::core_path()?
    };
    let own = quote!(#c::__private::OwnActions);
    let (imp, ty, bounds) = input.generics.split_for_impl();
    let empty = opts.no_actions.then(|| quote! {
        impl #imp #own for #id #ty #bounds {
            const ACTIONS: &'static [#c::ActionSpec] = &[];

            fn own_is_enabled(&self, _: &str) -> ::core::option::Option<bool> { None }

            fn own_invoke(&mut self, _: &str, _: &#c::Args) -> ::core::option::Option<::core::result::Result<#c::Value, #c::ActionError>> { None }
        }
    });
    Ok(quote! {
        #empty

        impl #imp #c::Primitive for #id #ty #bounds { const NAME: &'static str = #name; }

        impl #imp #c::Behavior for #id #ty #bounds {
            // Always borrow so describing a non-Copy field cannot move it.
            #[allow(clippy::needless_borrows_for_generic_args)]
            fn describe(&self, out: &mut #c::Describe) {
                #(#fields)* #(#c::Behavior::describe(&self.#behaviors,out);)*
            }

            fn actions(&self) -> ::std::vec::Vec<#c::ActionSpec> {
                let mut specs = <Self as #own>::ACTIONS.to_vec();
                #(specs.extend(#c::Behavior::actions(&self.#behaviors));)*
                specs
            }

            fn owns(&self, action: &str) -> bool {
                <Self as #own>::ACTIONS.iter().any(|s| s.name == action)
                    #(|| #c::Behavior::owns(&self.#behaviors, action))*
            }

            fn is_enabled(&self, action: &str) -> bool {
                if let Some(enabled) = #own::own_is_enabled(self, action) { return enabled; }
                #(if #c::Behavior::owns(&self.#behaviors, action) {
                    return #c::Behavior::is_enabled(&self.#behaviors,action);
                })*
                false
            }

            fn invoke(&mut self, action: &str, args:  &#c::Args) -> ::core::option::Option<::core::result::Result<#c::Value, #c::ActionError>> {
                #own::own_invoke(self,action,args) #(.or_else(|| #c::Behavior::invoke(&mut self.#behaviors,action,args)))*
            }

            fn queryable(&self) -> bool {
                false #(|| #c::Behavior::queryable(&self.#behaviors))*
            }

            fn query(&self, query: &#c::Query) -> ::core::option::Option<#c::Page> {
                None #(.or_else(|| #c::Behavior::query(&self.#behaviors,query)))*
            }
        }
    })
}
