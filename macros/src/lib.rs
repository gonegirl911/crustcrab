use heck::ToSnekCase;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DataEnum, DeriveInput, Fields, LitStr, Variant};

#[proc_macro_derive(Enum)]
pub fn derive_enum(input: TokenStream) -> TokenStream {
    derive_enum2(&parse_macro_input!(input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn derive_enum2(input: &DeriveInput) -> Result<TokenStream2, syn::Error> {
    let Data::Enum(DataEnum { variants, .. }) = &input.data else {
        return Err(syn::Error::new(
            Span::call_site(),
            "derive macro only supports enums",
        ));
    };

    if let Some(variant) = invalid_variant(variants) {
        return Err(syn::Error::new_spanned(
            variant,
            "#[derive(Enum)] only supports unit enum variants",
        ));
    }

    let ident = &input.ident;

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let len = variants.len();

    let from_index_unchecked_arms = variants.iter().enumerate().map(|(i, variant)| {
        let ident = &variant.ident;
        quote! { #i => Self::#ident }
    });

    let to_index_arms = variants.iter().enumerate().map(|(i, variant)| {
        let ident = &variant.ident;
        quote! { Self::#ident => #i }
    });

    Ok(quote! {
        unsafe impl #impl_generics crate::shared::enum_map::Enum for #ident #ty_generics #where_clause {
            type Length = ::generic_array::typenum::U<#len>;

            unsafe fn from_index_unchecked(index: ::core::primitive::usize) -> Self {
                match index {
                    #(#from_index_unchecked_arms),*,
                    _ => unsafe { ::core::hint::unreachable_unchecked() },
                }
            }

            fn to_index(self) -> ::core::primitive::usize {
                match self {
                    #(#to_index_arms),*,
                }
            }
        }
    })
}

#[proc_macro_derive(Display, attributes(display))]
pub fn derive_display(input: TokenStream) -> TokenStream {
    derive_display2(&parse_macro_input!(input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn derive_display2(input: &DeriveInput) -> Result<TokenStream2, syn::Error> {
    let Data::Enum(DataEnum { variants, .. }) = &input.data else {
        return Err(syn::Error::new(
            Span::call_site(),
            "derive macro only supports enums",
        ));
    };

    if let Some(variant) = invalid_variant(variants) {
        return Err(syn::Error::new_spanned(
            variant,
            "#[derive(Display)] only supports unit enum variants",
        ));
    }

    let format = parse_display_attrs(&input.attrs)?;

    if format.value() != "snek_case" {
        return Err(syn::Error::new_spanned(
            &format,
            "unknown display format, expected one of \"snek_case\"",
        ));
    }

    let ident = &input.ident;

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let arms = variants.iter().map(|variant| {
        let ident = &variant.ident;
        let output = ident.to_string().to_snek_case();
        quote! { Self::#ident => #output }
    });

    Ok(quote! {
        impl #impl_generics ::core::fmt::Display for #ident #ty_generics #where_clause {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match self {
                    #(#arms),*,
                }
                .fmt(f)
            }
        }
    })
}

fn parse_display_attrs(attrs: &[Attribute]) -> Result<LitStr, syn::Error> {
    let mut format = None;
    for attr in attrs {
        if attr.path().is_ident("display") {
            attr.parse_nested_meta(|meta| {
                if meta.path.require_ident()? == "format" {
                    if format.is_none() {
                        format = Some(meta.value()?.parse::<LitStr>()?);
                        Ok(())
                    } else {
                        Err(meta.error("duplicate display property"))
                    }
                } else {
                    Err(meta.error("unrecognized display property"))
                }
            })?;
        }
    }
    format.ok_or_else(|| syn::Error::new(Span::call_site(), "expected #[display(format = \"...\")"))
}

fn invalid_variant<'a, V>(variants: V) -> Option<&'a Variant>
where
    V: IntoIterator<Item = &'a Variant>,
{
    variants
        .into_iter()
        .find(|variant| !matches!(variant.fields, Fields::Unit))
}
