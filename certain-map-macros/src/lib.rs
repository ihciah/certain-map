// Copyright 2024 ihciah. All Rights Reserved.

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, quote_spanned, ToTokens};
use syn::{
    parse, parse::Parse, punctuated::Punctuated, Attribute, Expr, ExprLit, Field, Ident,
    ItemStruct, Lit, Meta, Result, Token, Type, Visibility,
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

#[derive(Copy, Clone, Default)]
enum GenStyle {
    // PreFilled generates a struct with all fields, allows to pass `&mut Handler`
    // to avoid stack copy when passing or setting fields.
    #[default]
    PreFilled,
    // Unfilled generates a struct with empty fields, when passing or setting fields,,
    // it has to copy all fields to a new typed struct.
    // It is easier to use, but may have performance overhead.
    // This is default style for certain-map 0.2.*.
    Unfilled,
}

struct CMap {
    attrs: Vec<Attribute>,
    vis: Visibility,
    ident: Ident,
    fields: Vec<Field>,
    fields_meta: Vec<Option<Punctuated<Meta, Token![,]>>>,

    span: Span,
    style: GenStyle,
}

impl Parse for CMap {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let span = input.span();
        let mut definition = ItemStruct::parse(input)?;

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

        // parse #[style = "unfilled"] and remove it.
        let mut style = GenStyle::default();
        let mut remove_idx = None;
        for (idx, attr) in definition.attrs.iter().enumerate() {
            if let Ok(name_val) = attr.meta.require_name_value() {
                if name_val.path.is_ident("style")
                    && matches!(&name_val.value, Expr::Lit(ExprLit{lit: Lit::Str(l), ..}) if l.value().eq_ignore_ascii_case("unfilled"))
                {
                    style = GenStyle::Unfilled;
                    remove_idx = Some(idx);
                    break;
                }
            }
        }
        if let Some(idx) = remove_idx {
            definition.attrs.remove(idx);
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
            style,
        })
    }
}

impl CMap {
    fn to_pre_filled_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut derive_clone = false;
        if let Some(derive) = Self::find_path_attr(&self.attrs, "derive") {
            if derive.1 == "Clone" {
                derive_clone = true;
            }
        }

        let vis = &self.vis;
        let ident = &self.ident;
        let state_ident = quote::format_ident!("{ident}State");
        let handler_ident = quote::format_ident!("{ident}Handler");
        let generic_types: Vec<_> = (0..self.fields.len())
            .map(generic_type)
            .map(IdentOrTokens::from)
            .collect();
        let names: Vec<_> = self
            .fields
            .iter()
            .map(|f| f.ident.as_ref().unwrap())
            .collect();
        let types: Vec<_> = self.fields.iter().map(|f| &f.ty).collect();

        // struct definition
        tokens.extend(quote_spanned! {
            self.span =>
                #vis struct #ident {
                    #(#names: ::std::mem::MaybeUninit<#types>,)*
                }
                #[allow(non_camel_case_types)]
                #vis struct #state_ident<#(#generic_types),*>
                where
                    #(#generic_types: ::certain_map::MaybeAvailable,)*
                {
                    #(#names: ::std::marker::PhantomData<#generic_types>,)*
                }
                #[allow(non_camel_case_types)]
                #[repr(transparent)]
                #vis struct #handler_ident<'a, #(#generic_types),*>
                where
                    #(#generic_types: ::certain_map::MaybeAvailable,)*
                {
                    inner: &'a mut #ident,
                    state: #state_ident<#(#generic_types),*>,
                }
        });

        // type alias
        if let Some((_, empty_ident)) = Self::find_path_attr(&self.attrs, "empty") {
            let vacancy_types =
                std::iter::repeat(quote!(::certain_map::Vacancy)).take(self.fields.len());
            tokens.extend(quote_spanned! {
                self.span =>
                    #vis type #empty_ident<'a> = #handler_ident<'a, #(#vacancy_types),*>;
            });
        }

        if let Some((_, full_ident)) = Self::find_path_attr(&self.attrs, "full") {
            let occupied_types =
                std::iter::repeat(quote!(::certain_map::OccupiedM)).take(self.fields.len());
            tokens.extend(quote_spanned! {
                self.span =>
                    #vis type #full_ident<'a> = #handler_ident<'a, #(#occupied_types),*>;
            });
        }

        let clone_with = if derive_clone {
            quote_spanned! {
                self.span =>
                    #[allow(non_camel_case_types)]
                    unsafe fn clone_with<#(#generic_types),*>(&self, _state: &#state_ident<#(#generic_types),*>) -> Self
                    where
                        #(#generic_types: ::certain_map::MaybeAvailable,)*
                    {
                        Self {
                            #(#names: #generic_types::do_clone(&self.#names),)*
                        }
                    }
            }
        } else {
            quote!()
        };

        // impl #ident
        let vacancy_types =
            std::iter::repeat(quote!(::certain_map::Vacancy)).take(self.fields.len());
        let vacancy_types2 =
            std::iter::repeat(quote!(::certain_map::Vacancy)).take(self.fields.len());
        tokens.extend(quote_spanned! {
            self.span =>
                impl #ident {
                    #[inline]
                    pub const fn new() -> Self {
                        Self {
                            #(#names: ::std::mem::MaybeUninit::uninit(),)*
                        }
                    }
                    #[inline]
                    pub fn handler(&mut self) -> #handler_ident<'_, #(#vacancy_types),*> {
                        #handler_ident {
                            inner: self,
                            state: #state_ident::new(),
                        }
                    }
                    #clone_with
                }
                impl ::certain_map::Handler for #ident {
                    type Hdr<'a> = #handler_ident<'a, #(#vacancy_types2),*>
                    where
                        Self: 'a;
                    #[inline]
                    fn handler(&mut self) -> Self::Hdr<'_> {
                        self.handler()
                    }
                }
                impl ::std::default::Default for #ident {
                    #[inline]
                    fn default() -> Self {
                        Self::new()
                    }
                }
        });

        // impl #state_ident
        tokens.extend(quote_spanned! {
            self.span =>
                #[allow(non_camel_case_types)]
                impl<#(#generic_types),*> #state_ident<#(#generic_types),*>
                where
                    #(#generic_types: ::certain_map::MaybeAvailable,)*
                {
                    const fn new() -> Self {
                        Self {
                            #(#names: ::std::marker::PhantomData,)*
                        }
                    }
                    /// # Safety
                    /// The caller must make sure the attached map has the data of current state.
                    #[inline]
                    pub unsafe fn attach(self, inner: &mut #ident) -> #handler_ident<'_, #(#generic_types),*> {
                        #handler_ident {
                            inner,
                            state: Self::new(),
                        }
                    }
                }
                #[allow(non_camel_case_types)]
                impl<#(#generic_types),*> ::certain_map::Attach<#ident> for #state_ident<#(#generic_types),*>
                where
                    #(#generic_types: ::certain_map::MaybeAvailable,)*
                {
                    type Hdr<'a> = #handler_ident<'a, #(#generic_types),*>;
                    #[inline]
                    unsafe fn attach(self, store: &mut #ident) -> Self::Hdr<'_> {
                        self.attach(store)
                    }
                }
        });

        if derive_clone {
            // impl #handler_ident
            tokens.extend(quote_spanned! {
                self.span =>
                    #[allow(non_camel_case_types)]
                    impl<#(#generic_types),*> #handler_ident<'_, #(#generic_types),*>
                    where
                        #(#generic_types: ::certain_map::MaybeAvailable,)*
                    {
                        #[inline]
                        pub fn fork(&self) -> (#ident, #state_ident<#(#generic_types),*>) {
                            // Safety: we are sure about the state of the map.
                            let inner = unsafe { self.inner.clone_with(&self.state) };
                            (inner, #state_ident::new())
                        }
                    }
                    #[allow(non_camel_case_types)]
                    impl<#(#generic_types),*> ::certain_map::Fork for #handler_ident<'_, #(#generic_types),*>
                    where
                        #(#generic_types: ::certain_map::MaybeAvailable,)*
                    {
                        type Store = #ident;
                        type State = #state_ident<#(#generic_types),*>;
                        #[inline]
                        fn fork(&self) -> (Self::Store, Self::State) {
                            self.fork()
                        }
                    }
            });
        }

        // impl Drop for #handler_ident
        tokens.extend(quote_spanned! {
            self.span =>
                #[allow(non_camel_case_types)]
                impl<#(#generic_types),*> Drop for #handler_ident<'_, #(#generic_types),*>
                where
                    #(#generic_types: ::certain_map::MaybeAvailable,)*
                {
                    fn drop(&mut self) {
                        unsafe {
                            #(#generic_types::do_drop(&mut self.inner.#names);)*
                        }
                    }
                }
        });

        // impl ParamRef<T>/ParamMut<T>/ParamTake<T> for #handler_ident
        for (idx, field) in self.fields.iter().enumerate() {
            let ty = &field.ty;
            let name = field.ident.as_ref().unwrap();
            let generic_type = generic_type(idx);
            let generic_types_rest1 = IgnoreIter::new(generic_types.iter(), idx);
            let generic_types_rest2 = IgnoreIter::new(generic_types.iter(), idx);
            let generic_types_rest3 = IgnoreIter::new(generic_types.iter(), idx);
            let vacancy = IdentOrTokens::from(vacancy_type());
            let generic_types_replaced_vacancy =
                ReplaceIter::new(generic_types.iter(), idx, &vacancy);
            tokens.extend(quote_spanned! {
                self.span =>
                    #[allow(non_camel_case_types)]
                    impl<#(#generic_types),*> ::certain_map::ParamRef<#ty> for #handler_ident<'_, #(#generic_types),*>
                    where
                        #generic_type: ::certain_map::Available,
                        #(#generic_types_rest1: ::certain_map::MaybeAvailable,)*
                    {
                        #[inline]
                        fn param_ref(&self) -> &#ty {
                            unsafe { #generic_type::do_ref(&self.inner.#name) }
                        }
                    }
                    #[allow(non_camel_case_types)]
                    impl<#(#generic_types),*> ::certain_map::ParamMut<#ty> for #handler_ident<'_, #(#generic_types),*>
                    where
                        #generic_type: ::certain_map::Available,
                        #(#generic_types_rest2: ::certain_map::MaybeAvailable,)*
                    {
                        #[inline]
                        fn param_mut(&mut self) -> &mut #ty {
                            unsafe { #generic_type::do_mut(&mut self.inner.#name) }
                        }
                    }
                    #[allow(non_camel_case_types)]
                    impl<'a, #(#generic_types),*> ::certain_map::ParamTake<#ty> for #handler_ident<'a, #(#generic_types),*>
                    where
                        #generic_type: ::certain_map::Available,
                        #(#generic_types_rest3: ::certain_map::MaybeAvailable,)*
                    {
                        type Transformed = #handler_ident<'a, #(#generic_types_replaced_vacancy),*>;
                        #[inline]
                        fn param_take(self) -> (Self::Transformed, #ty) {
                            let item = unsafe { #generic_type::do_take(&self.inner.#name) };
                            #[allow(clippy::missing_transmute_annotations)]
                            (unsafe { ::std::mem::transmute(self) }, item)
                        }
                    }
            });
        }

        // impl ParamMaybeRef<T>/ParamMaybeMut<T>/ParamSet<T>/ParamRemove<T> for #handler_ident
        for (idx, field) in self.fields.iter().enumerate() {
            let ty = &field.ty;
            let name = field.ident.as_ref().unwrap();
            let generic_type = generic_type(idx);

            let occupied = IdentOrTokens::from(occupied_m_type());
            let generic_types_replaced_occupied =
                ReplaceIter::new(generic_types.iter(), idx, &occupied);
            let vacancy = IdentOrTokens::from(vacancy_type());
            let generic_types_replaced_vacancy =
                ReplaceIter::new(generic_types.iter(), idx, &vacancy);
            tokens.extend(quote_spanned! {
                self.span =>
                    #[allow(non_camel_case_types)]
                    impl<#(#generic_types),*> ::certain_map::ParamMaybeRef<#ty> for #handler_ident<'_, #(#generic_types),*>
                    where
                        #(#generic_types: ::certain_map::MaybeAvailable,)*
                    {
                        #[inline]
                        fn param_maybe_ref(&self) -> Option<&#ty> {
                            unsafe { #generic_type::do_maybe_ref(&self.inner.#name) }
                        }
                    }
                    #[allow(non_camel_case_types)]
                    impl<#(#generic_types),*> ::certain_map::ParamMaybeMut<#ty> for #handler_ident<'_, #(#generic_types),*>
                    where
                        #(#generic_types: ::certain_map::MaybeAvailable,)*
                    {
                        #[inline]
                        fn param_maybe_mut(&mut self) -> Option<&mut #ty> {
                            unsafe { #generic_type::do_maybe_mut(&mut self.inner.#name) }
                        }
                    }
                    #[allow(non_camel_case_types)]
                    impl<'a, #(#generic_types),*> ::certain_map::ParamSet<#ty> for #handler_ident<'a, #(#generic_types),*>
                    where
                        #(#generic_types: ::certain_map::MaybeAvailable,)*
                    {
                        type Transformed = #handler_ident<'a, #(#generic_types_replaced_occupied),*>;
                        #[inline]
                        fn param_set(self, item: #ty) -> Self::Transformed {
                            unsafe {
                                #generic_type::do_set(&mut self.inner.#name, item);
                                #[allow(clippy::missing_transmute_annotations)]
                                ::std::mem::transmute(self)
                            }
                        }
                    }
                    #[allow(non_camel_case_types)]
                    impl<'a, #(#generic_types),*> ::certain_map::ParamRemove<#ty> for #handler_ident<'a, #(#generic_types),*>
                    where
                        #(#generic_types: ::certain_map::MaybeAvailable,)*
                    {
                        type Transformed = #handler_ident<'a, #(#generic_types_replaced_vacancy),*>;
                        #[inline]
                        fn param_remove(self) -> Self::Transformed {
                            unsafe {
                                #generic_type::do_drop(&mut self.inner.#name);
                                #[allow(clippy::missing_transmute_annotations)]
                                ::std::mem::transmute(self)
                            }
                        }
                    }
            });
        }

        // impl Param<T> and Param<Option<T>> if #[ensure(Clone)] or derive_clone
        for (idx, (field, maybe_meta)) in
            self.fields.iter().zip(self.fields_meta.iter()).enumerate()
        {
            if derive_clone
                || maybe_meta
                    .iter()
                    .flat_map(|x| x.iter())
                    .any(|meta| matches!(meta, Meta::Path(path) if path.is_ident("Clone")))
            {
                let ty = &field.ty;
                let name = field.ident.as_ref().unwrap();
                let generic_type = generic_type(idx);
                let generic_types_rest = IgnoreIter::new(generic_types.iter(), idx);
                tokens.extend(quote_spanned! {
                    self.span =>
                        #[allow(non_camel_case_types)]
                        impl<#(#generic_types),*> ::certain_map::Param<#ty> for #handler_ident<'_, #(#generic_types),*>
                        where
                            #generic_type: ::certain_map::Available,
                            #(#generic_types_rest: ::certain_map::MaybeAvailable,)*
                        {
                            #[inline]
                            fn param(&self) -> #ty {
                                unsafe { #generic_type::do_read(&self.inner.#name) }
                            }
                        }
                        #[allow(non_camel_case_types)]
                        impl<#(#generic_types),*> ::certain_map::Param<Option<#ty>> for #handler_ident<'_, #(#generic_types),*>
                        where
                            #(#generic_types: ::certain_map::MaybeAvailable,)*
                        {
                            #[inline]
                            fn param(&self) -> Option<#ty> {
                                #[allow(clippy::clone_on_copy)]
                                unsafe { #generic_type::do_maybe_ref(&self.inner.#name).cloned() }
                            }
                        }
                });
            }
        }
    }

    fn to_unfilled_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut attrs = self.attrs.clone();
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
        if let Some((empty_idx, empty_ident)) = Self::find_path_attr(&attrs, "empty") {
            attrs.remove(empty_idx);
            let vacancy_types =
                std::iter::repeat(quote!(::certain_map::Vacancy)).take(self.fields.len());
            tokens.extend(quote_spanned! {
                self.span =>
                    #vis type #empty_ident = #ident<#(#vacancy_types),*>;
            });
        }

        if let Some((full_idx, full_ident)) = Self::find_path_attr(&attrs, "full") {
            attrs.remove(full_idx);
            let occupied_types = self.fields.iter().map(|f| occupied_type(&f.ty));
            tokens.extend(quote_spanned! {
                self.span =>
                    #vis type #full_ident = #ident<#(#occupied_types),*>;
            });
        }

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
                impl ::std::default::Default for #ident<#(#vacancy_types1),*> {
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

        // impl ParamRef<T>
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

        // impl ParamMaybeRef<T> for occupied
        for (idx, field) in self.fields.iter().enumerate() {
            let ty = &field.ty;
            let name = field.ident.as_ref().unwrap();
            let generic_types_ignored = IgnoreIter::new(generic_types.iter(), idx);
            let occupied = IdentOrTokens::from(occupied_type(ty));
            let generic_types_replaced = ReplaceIter::new(generic_types.iter(), idx, &occupied);
            tokens.extend(quote_spanned! {
                self.span =>
                    impl<#(#generic_types_ignored),*> ::certain_map::ParamMaybeRef<#ty> for #ident<#(#generic_types_replaced),*> {
                        #[inline]
                        fn param_maybe_ref(&self) -> Option<&#ty> {
                            Some(&self.#name.0)
                        }
                    }
            });
        }

        // impl ParamMaybeRef<T> for vacancy
        for (idx, field) in self.fields.iter().enumerate() {
            let ty = &field.ty;
            let generic_types_ignored = IgnoreIter::new(generic_types.iter(), idx);
            let vacancy = IdentOrTokens::from(vacancy_type());
            let generic_types_replaced = ReplaceIter::new(generic_types.iter(), idx, &vacancy);
            tokens.extend(quote_spanned! {
                self.span =>
                    impl<#(#generic_types_ignored),*> ::certain_map::ParamMaybeRef<#ty> for #ident<#(#generic_types_replaced),*> {
                        #[inline]
                        fn param_maybe_ref(&self) -> Option<&#ty> {
                            None
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

        // impl ParamMaybeMut<T> for occupied
        for (idx, field) in self.fields.iter().enumerate() {
            let ty = &field.ty;
            let name = field.ident.as_ref().unwrap();
            let generic_types_ignored = IgnoreIter::new(generic_types.iter(), idx);
            let occupied = IdentOrTokens::from(occupied_type(ty));
            let generic_types_replaced = ReplaceIter::new(generic_types.iter(), idx, &occupied);
            tokens.extend(quote_spanned! {
                self.span =>
                    impl<#(#generic_types_ignored),*> ::certain_map::ParamMaybeMut<#ty> for #ident<#(#generic_types_replaced),*> {
                        #[inline]
                        fn param_maybe_mut(&mut self) -> Option<&mut #ty> {
                            Some(&mut self.#name.0)
                        }
                    }
            });
        }

        // impl ParamMaybeMut<T> for vacancy
        for (idx, field) in self.fields.iter().enumerate() {
            let ty = &field.ty;
            let generic_types_ignored = IgnoreIter::new(generic_types.iter(), idx);
            let vacancy = IdentOrTokens::from(vacancy_type());
            let generic_types_replaced = ReplaceIter::new(generic_types.iter(), idx, &vacancy);
            tokens.extend(quote_spanned! {
                self.span =>
                    impl<#(#generic_types_ignored),*> ::certain_map::ParamMaybeMut<#ty> for #ident<#(#generic_types_replaced),*> {
                        #[inline]
                        fn param_maybe_mut(&mut self) -> Option<&mut #ty> {
                            None
                        }
                    }
            });
        }

        // impl Param<T> and Param<Option<T>> if #[ensure(Clone)]
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
                let occupied = IdentOrTokens::from(occupied_type(ty));
                let vacancy = IdentOrTokens::from(vacancy_type());

                let generic_types_ignored = IgnoreIter::new(generic_types.iter(), idx);
                let generic_types_occupied = ReplaceIter::new(generic_types.iter(), idx, &occupied);

                let generic_types_ignored2 = IgnoreIter::new(generic_types.iter(), idx);
                let generic_types_occupied2 =
                    ReplaceIter::new(generic_types.iter(), idx, &occupied);

                let generic_types_ignored3 = IgnoreIter::new(generic_types.iter(), idx);
                let generic_types_vacancy = ReplaceIter::new(generic_types.iter(), idx, &vacancy);
                tokens.extend(quote_spanned! {
                self.span =>
                    impl<#(#generic_types_ignored),*> ::certain_map::Param<#ty> for #ident<#(#generic_types_occupied),*> {
                        #[inline]
                        fn param(&self) -> #ty {
                            #[allow(clippy::clone_on_copy)]
                            self.#name.0.clone()
                        }
                    }
                    impl<#(#generic_types_ignored2),*> ::certain_map::Param<Option<#ty>> for #ident<#(#generic_types_occupied2),*> {
                        #[inline]
                        fn param(&self) -> Option<#ty> {
                            #[allow(clippy::clone_on_copy)]
                            Some(self.#name.0.clone())
                        }
                    }
                    impl<#(#generic_types_ignored3),*> ::certain_map::Param<Option<#ty>> for #ident<#(#generic_types_vacancy),*> {
                        #[inline]
                        fn param(&self) -> Option<#ty> {
                            None
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

    fn find_path_attr(attrs: &[Attribute], ident: &str) -> Option<(usize, Ident)> {
        let mut default = None;
        for (idx, attr) in attrs.iter().enumerate() {
            if !attr.path().is_ident(ident) {
                continue;
            }
            if let Ok(Meta::Path(path)) = attr.parse_args::<Meta>() {
                if let Some(path) = path.get_ident() {
                    default = Some((idx, path.clone()));
                    break;
                }
            }
        }
        default
    }
}

impl ToTokens for CMap {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self.style {
            GenStyle::PreFilled => self.to_pre_filled_tokens(tokens),
            GenStyle::Unfilled => self.to_unfilled_tokens(tokens),
        }
    }
}

fn generic_type(num: usize) -> Ident {
    quote::format_ident!("_CMT_{num}")
}

fn occupied_type(ty: &Type) -> proc_macro2::TokenStream {
    quote! {::certain_map::Occupied<#ty>}
}

fn occupied_m_type() -> proc_macro2::TokenStream {
    quote! {::certain_map::OccupiedM}
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
