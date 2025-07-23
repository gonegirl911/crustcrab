use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Data, DataEnum, DeriveInput, Fields, Variant, parse_macro_input};

#[proc_macro_derive(Enum)]
pub fn derive_enum(input: TokenStream) -> TokenStream {
    derive_enum2(&parse_macro_input!(input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn derive_enum2(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let Data::Enum(DataEnum { variants, .. }) = &input.data else {
        return Err(syn::Error::new(
            Span::call_site(),
            "derive macro only supports enums",
        ));
    };

    if let Some(variant) = variants
        .into_iter()
        .find(|variant| variant.fields != Fields::Unit)
    {
        return Err(syn::Error::new_spanned(
            variant,
            "#[derive(Enum)] only supports unit enum variants",
        ));
    }

    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let len = variants.len();
    let from_index_unchecked_arms =
        variants
            .iter()
            .enumerate()
            .map(|(i, Variant { ident, .. })| {
                quote! { #i => Self::#ident }
            });
    let to_index_arms = variants
        .iter()
        .enumerate()
        .map(|(i, Variant { ident, .. })| {
            quote! { Self::#ident => #i }
        });

    Ok(quote! {
        unsafe impl #impl_generics crate::shared::enum_map::Enum for #ident #ty_generics #where_clause {
            type Length = ::generic_array::typenum::U<#len>;

            unsafe fn from_index_unchecked(index: ::core::primitive::usize) -> Self {
                match index {
                    #(#from_index_unchecked_arms,)*
                    _ => unsafe { ::core::hint::unreachable_unchecked() },
                }
            }

            fn to_index(self) -> ::core::primitive::usize {
                match self {
                    #(#to_index_arms,)*
                }
            }
        }
    })
}
