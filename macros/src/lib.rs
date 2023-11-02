use heck::ToSnakeCase;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DataEnum, DeriveInput, Fields, LitStr, Variant};

macro_rules! error {
    ($span:expr, $msg:expr) => {
        ::syn::Error::new_spanned($span, $msg)
    };
}

#[proc_macro_derive(Enum)]
pub fn derive_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_enum_input(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_enum_input(input: &DeriveInput) -> Result<TokenStream2, syn::Error> {
    let Data::Enum(DataEnum { variants, .. }) = &input.data else {
        return Err(error!(&input, "derive(Enum) only supports enums"));
    };

    if let Some(variant) = invalid_variant(variants) {
        return Err(error!(&variant, "derive(Enum) only supports unit variants"));
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

            unsafe fn from_index_unchecked(index: usize) -> Self {
                match index {
                    #(#from_index_unchecked_arms),*,
                    _ => unsafe { ::std::hint::unreachable_unchecked() },
                }
            }

            fn to_index(self) -> usize {
                match self {
                    #(#to_index_arms),*
                }
            }
        }
    })
}

#[proc_macro_derive(Display, attributes(display))]
pub fn derive_display(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_display_input(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_display_input(input: &DeriveInput) -> Result<TokenStream2, syn::Error> {
    let Data::Enum(DataEnum { variants, .. }) = &input.data else {
        return Err(error!(&input, "derive(Display) only supports enums"));
    };

    if let Some(variant) = invalid_variant(variants) {
        return Err(error!(
            &variant,
            "derive(Display) only supports unit variants"
        ));
    }

    let format = parse_display_attrs(input)?;

    if format.value() != "snake_case" {
        return Err(error!(
            &format,
            "unknown display format, expected one of \"snake_case\""
        ));
    }

    let ident = &input.ident;

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let arms = variants.iter().map(|variant| {
        let ident = &variant.ident;
        let output = ident.to_string().to_snake_case();
        quote! { Self::#ident => #output.fmt(f) }
    });

    Ok(quote! {
        impl #impl_generics ::core::fmt::Display for #ident #ty_generics #where_clause {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match self {
                    #(#arms),*
                }
            }
        }
    })
}

fn parse_display_attrs(input: &DeriveInput) -> Result<LitStr, syn::Error> {
    let mut format = None;
    for attr in &input.attrs {
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
    format.ok_or_else(|| error!(&input, "expected #[display(format = \"\")"))
}

fn invalid_variant<'a, I>(variants: I) -> Option<&'a Variant>
where
    I: IntoIterator<Item = &'a Variant>,
{
    variants
        .into_iter()
        .find(|variant| !matches!(variant.fields, Fields::Unit))
}
