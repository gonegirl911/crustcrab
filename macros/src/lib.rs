use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{parse_macro_input, Data, DataEnum, DeriveInput, Fields, Variant};

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

fn invalid_variant<'a, V>(variants: V) -> Option<&'a Variant>
where
    V: IntoIterator<Item = &'a Variant>,
{
    variants
        .into_iter()
        .find(|variant| !matches!(variant.fields, Fields::Unit))
}
