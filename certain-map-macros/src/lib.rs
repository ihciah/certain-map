use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, quote_spanned, ToTokens};
use syn::{
    parse, parse::Parse, punctuated::Punctuated, Attribute, Field, Ident, ItemStruct, Meta, Result,
    Token, Type, Visibility,
};

#[proc_macro]
pub fn certain_map(input: TokenStream) -> TokenStream {
    let cmap: CMap = match parse(input) {
        Ok(m) => m,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };

    let output = cmap.to_token_stream();
    TokenStream::from(output)
}

struct CMap {
    attrs: Vec<Attribute>,
    vis: Visibility,
    ident: Ident,
    fields: Vec<Field>,
    fields_meta: Vec<Option<Punctuated<Meta, Token![,]>>>,

    span: Span,
}

impl Parse for CMap {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let span = input.span();
        let definition = ItemStruct::parse(input)?;

        if definition.generics.where_clause.is_some() {
            return Err(syn::Error::new(
                span,
                "generic where clause is not supported",
            ));
        }
        if definition.generics.type_params().next().is_some() {
            return Err(syn::Error::new(span, "generic types are not supported"));
        }
        if definition.generics.lifetimes().next().is_some() {
            return Err(syn::Error::new(span, "generic lifetimes are not supported"));
        }

        let fields: Vec<Field> = definition.fields.into_iter().collect();
        if fields.iter().any(|f| f.ident.is_none()) {
            return Err(syn::Error::new(
                span,
                "fields without names are not supported",
            ));
        }

        let mut fields_meta = Vec::with_capacity(fields.len());
        for field in fields.iter() {
            let maybe_meta = if let Some(attr) = field.attrs.first() {
                if !attr.path().is_ident("ensure") {
                    return Err(syn::Error::new(
                        span,
                        "fields attr now only support #[ensure(Clone)]",
                    ));
                }
                let nested =
                    attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
                if nested
                    .iter()
                    .any(|meta| !matches!(meta, Meta::Path(path) if path.is_ident("Clone")))
                {
                    return Err(syn::Error::new(
                        span,
                        "fields attr now only support #[ensure(Clone)]",
                    ));
                }
                Some(nested)
            } else {
                None
            };
            fields_meta.push(maybe_meta);
        }

        Ok(CMap {
            attrs: definition.attrs,
            vis: definition.vis,
            ident: definition.ident,
            fields,
            fields_meta,
            span,
        })
    }
}

impl ToTokens for CMap {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let attrs = &self.attrs;
        let vis = &self.vis;
        let ident = &self.ident;
        let generic_types: Vec<_> = (0..self.fields.len())
            .map(generic_type)
            .map(IdentOrTokens::from)
            .collect();
        let names: Vec<_> = self
            .fields
            .iter()
            .map(|f| f.ident.as_ref().unwrap())
            .collect();

        // struct definition
        tokens.extend(quote_spanned! {
            self.span =>
                #(#attrs)*
                #vis struct #ident<#(#generic_types),*> {
                    #(#names: #generic_types, )*
                }
        });

        // impl new and Default
        let vacancy_types1 =
            std::iter::repeat(quote!(::certain_map::Vacancy)).take(self.fields.len());
        let vacancy_types2 =
            std::iter::repeat(quote!(::certain_map::Vacancy)).take(self.fields.len());
        let vacancy_values =
            std::iter::repeat(quote!(::certain_map::Vacancy)).take(self.fields.len());
        tokens.extend(quote_spanned! {
            self.span =>
                impl Default for #ident<#(#vacancy_types1),*> {
                    #[inline]
                    fn default() -> Self {
                        Self::new()
                    }
                }
                impl #ident<#(#vacancy_types2),*> {
                    pub const fn new() -> Self {
                        Self {
                            #(#names: #vacancy_values),*
                        }
                    }
                }
        });

        // impl ParamRef
        for (idx, field) in self.fields.iter().enumerate() {
            let ty = &field.ty;
            let name = field.ident.as_ref().unwrap();
            let generic_types_ignored = IgnoreIter::new(generic_types.iter(), idx);
            let occupied = IdentOrTokens::from(occupied_type(ty));
            let generic_types_replaced = ReplaceIter::new(generic_types.iter(), idx, &occupied);
            tokens.extend(quote_spanned! {
                self.span =>
                    impl<#(#generic_types_ignored),*> ::certain_map::ParamRef<#ty> for #ident<#(#generic_types_replaced),*> {
                        #[inline]
                        fn param_ref(&self) -> &#ty {
                            &self.#name.0
                        }
                    }
            });
        }

        // impl ParamMut
        for (idx, field) in self.fields.iter().enumerate() {
            let ty = &field.ty;
            let name = field.ident.as_ref().unwrap();
            let generic_types_ignored = IgnoreIter::new(generic_types.iter(), idx);
            let occupied = IdentOrTokens::from(occupied_type(ty));
            let generic_types_replaced = ReplaceIter::new(generic_types.iter(), idx, &occupied);
            tokens.extend(quote_spanned! {
                self.span =>
                    impl<#(#generic_types_ignored),*> ::certain_map::ParamMut<#ty> for #ident<#(#generic_types_replaced),*> {
                        #[inline]
                        fn param_mut(&mut self) -> &mut #ty {
                            &mut self.#name.0
                        }
                    }
            });
        }

        // impl Param if #[ensure(Clone)]
        for (idx, (field, maybe_meta)) in
            self.fields.iter().zip(self.fields_meta.iter()).enumerate()
        {
            if maybe_meta
                .iter()
                .flat_map(|x| x.iter())
                .any(|meta| matches!(meta, Meta::Path(path) if path.is_ident("Clone")))
            {
                let ty = &field.ty;
                let name = field.ident.as_ref().unwrap();

                let generic_types_ignored = IgnoreIter::new(generic_types.iter(), idx);
                let occupied = IdentOrTokens::from(occupied_type(ty));
                let generic_types_replaced = ReplaceIter::new(generic_types.iter(), idx, &occupied);
                tokens.extend(quote_spanned! {
                self.span =>
                    impl<#(#generic_types_ignored),*> ::certain_map::Param<#ty> for #ident<#(#generic_types_replaced),*> {
                        #[inline]
                        fn param(&self) -> #ty {
                            #[allow(clippy::clone_on_copy)]
                            self.#name.0.clone()
                        }
                    }
                });
            }
        }

        // impl ParamSet
        for (idx, field) in self.fields.iter().enumerate() {
            let ty = &field.ty;
            let name = field.ident.as_ref().unwrap();
            let occupied = IdentOrTokens::from(occupied_type(ty));
            let generic_types_replaced = ReplaceIter::new(generic_types.iter(), idx, &occupied);
            let direct_assign = quote!(#name: ::certain_map::Occupied(item));
            let assignations = ReplaceIter::new(
                names.iter().map(|&name| quote!(#name: self.#name)),
                idx,
                direct_assign,
            );
            tokens.extend(quote_spanned! {
                self.span =>
                impl<#(#generic_types),*> ::certain_map::ParamSet<#ty> for #ident<#(#generic_types),*> {
                    type Transformed = #ident<#(#generic_types_replaced),*>;

                    #[inline]
                    fn param_set(self, item: #ty) -> Self::Transformed {
                        #ident {
                            #(#assignations),*
                        }
                    }
                }
            });
        }

        // impl ParamRemove
        for (idx, field) in self.fields.iter().enumerate() {
            let ty = &field.ty;
            let name = field.ident.as_ref().unwrap();
            let vacancy = IdentOrTokens::from(vacancy_type());
            let generic_types_replaced = ReplaceIter::new(generic_types.iter(), idx, &vacancy);
            let direct_assign = quote!(#name: ::certain_map::Vacancy);
            let assignations = ReplaceIter::new(
                names.iter().map(|&name| quote!(#name: self.#name)),
                idx,
                direct_assign,
            );
            tokens.extend(quote_spanned! {
                self.span =>
                impl<#(#generic_types),*> ::certain_map::ParamRemove<#ty> for #ident<#(#generic_types),*> {
                    type Transformed = #ident<#(#generic_types_replaced),*>;

                    #[inline]
                    fn param_remove(self) -> Self::Transformed {
                        #ident {
                            #(#assignations),*
                        }
                    }
                }
            });
        }

        // impl ParamTake
        for (idx, field) in self.fields.iter().enumerate() {
            let ty = &field.ty;
            let name = field.ident.as_ref().unwrap();
            let generic_types_ignored = IgnoreIter::new(generic_types.iter(), idx);
            let occupied = IdentOrTokens::from(occupied_type(ty));
            let generic_types_replaced = ReplaceIter::new(generic_types.iter(), idx, &occupied);

            let vacancy = IdentOrTokens::from(vacancy_type());
            let generic_types_replaced_transformed =
                ReplaceIter::new(generic_types.iter(), idx, &vacancy);
            let direct_assign = quote!(#name: ::certain_map::Vacancy);
            let assignations = ReplaceIter::new(
                names.iter().map(|&name| quote!(#name: self.#name)),
                idx,
                direct_assign,
            );
            let removed_name = names[idx];
            let removed = quote!(self.#removed_name);
            tokens.extend(quote_spanned! {
                self.span =>
                impl<#(#generic_types_ignored),*> ::certain_map::ParamTake<#ty> for #ident<#(#generic_types_replaced),*> {
                    type Transformed = #ident<#(#generic_types_replaced_transformed),*>;

                    #[inline]
                    fn param_take(self) -> (Self::Transformed, #ty) {
                        let after_remove = #ident {
                            #(#assignations),*
                        };
                        (after_remove, #removed.0)
                    }
                }
            });
        }
    }
}

fn generic_type(num: usize) -> Ident {
    quote::format_ident!("_CMT_{num}")
}

fn occupied_type(ty: &Type) -> proc_macro2::TokenStream {
    quote! {::certain_map::Occupied<#ty>}
}

fn vacancy_type() -> proc_macro2::TokenStream {
    quote! {::certain_map::Vacancy}
}

enum IdentOrTokens {
    Ident(Ident),
    Tokens(proc_macro2::TokenStream),
}

impl From<Ident> for IdentOrTokens {
    fn from(value: Ident) -> Self {
        Self::Ident(value)
    }
}

impl From<proc_macro2::TokenStream> for IdentOrTokens {
    fn from(value: proc_macro2::TokenStream) -> Self {
        Self::Tokens(value)
    }
}

impl ToTokens for IdentOrTokens {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            IdentOrTokens::Ident(inner) => inner.to_tokens(tokens),
            IdentOrTokens::Tokens(inner) => inner.to_tokens(tokens),
        }
    }
}

struct IgnoreIter<I> {
    inner: I,
    ignore: Option<usize>,
}

impl<I> IgnoreIter<I> {
    fn new(iter: I, idx: usize) -> Self {
        Self {
            inner: iter,
            ignore: Some(idx),
        }
    }
}

impl<I, Item> Iterator for IgnoreIter<I>
where
    I: Iterator<Item = Item>,
{
    type Item = Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self.ignore.as_mut() {
            None => self.inner.next(),
            Some(i) if *i == 0 => {
                self.ignore = None;
                let _ = self.inner.next();
                self.inner.next()
            }
            Some(i) => {
                *i -= 1;
                self.inner.next()
            }
        }
    }
}

struct ReplaceIter<I, Item> {
    inner: I,
    replace: Option<(usize, Item)>,
}

impl<I, Item> ReplaceIter<I, Item> {
    fn new(iter: I, idx: usize, item: Item) -> Self {
        Self {
            inner: iter,
            replace: Some((idx, item)),
        }
    }
}

impl<I, Item> Iterator for ReplaceIter<I, Item>
where
    I: Iterator<Item = Item>,
{
    type Item = Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self.replace.as_mut() {
            None => self.inner.next(),
            Some((i, _)) if *i == 0 => {
                let item = unsafe { self.replace.take().unwrap_unchecked().1 };
                let _ = self.inner.next();
                Some(item)
            }
            Some((i, _)) => {
                *i -= 1;
                self.inner.next()
            }
        }
    }
}
