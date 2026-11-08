//! Derive macros used by Mata's binary-format decoders.
//!
//! `FromPrimitive` selects a canonical integer representation from its values,
//! then implements checked conversions from every standard integer width.
//! An inherent `Enum::try_from(value)` method avoids the ambiguous integer
//! inference normally caused by multiple `TryFrom<integer>` implementations.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Data, DeriveInput, Expr, Fields, Lit, LitInt, RangeLimits, UnOp,
};

/// Derives checked conversions from standard integer representations of an enum.
///
/// The canonical integer type is determined in this order:
/// 1. `#[from_primitive(u16)]` on the enum (useful for fixed-width wire fields).
/// 2. `#[repr(u16)]` on the enum, when present.
/// 3. The smallest signed/unsigned integer that holds every `#[value(...)]`.
///
/// `#[value(N)]` and `#[value(A..=B)]` are supported. A single-field tuple
/// variant without `#[value]` is an optional catch-all; it produces an
/// infallible `From<canonical_type>`. Other integer types use checked `TryFrom`.
///
/// The derive also exposes an inherent `Enum::try_from(value)` method accepting
/// any integer that implements `TryInto<canonical_type>`, including typed `u16`
/// TIFF fields. This keeps unsuffixed literals unambiguous, while allowing
/// `<Enum as TryFrom<u16>>::try_from(value)` in generic code.
///
/// The canonical type still controls the representable range. If every possible
/// value of a binary field must be preserved (including unknown codes in a
/// catch-all variant), annotate its wire type via `#[from_primitive(u16)]`.
#[proc_macro_derive(FromPrimitive, attributes(value, from_primitive))]
pub fn derive_from_primitive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_from_primitive(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[derive(Clone, Copy)]
struct Number {
    negative: bool,
    magnitude: u128,
}

impl Number {
    fn from_expr(expr: &Expr) -> syn::Result<Self> {
        match expr {
            Expr::Lit(literal) => match &literal.lit {
                Lit::Int(n) => Ok(Self { negative: false, magnitude: parse_int(n)? }),
                _ => Err(syn::Error::new_spanned(expr, "expected integer value")),
            },
            Expr::Unary(unary) if matches!(unary.op, UnOp::Neg(_)) => {
                let mut number = Self::from_expr(&unary.expr)?;
                number.negative = number.magnitude != 0;
                Ok(number)
            }
            Expr::Paren(paren) => Self::from_expr(&paren.expr),
            Expr::Group(group) => Self::from_expr(&group.expr),
            _ => Err(syn::Error::new_spanned(expr, "expected a numeric literal")),
        }
    }
}

fn parse_int(lit: &LitInt) -> syn::Result<u128> {
    lit.base10_parse::<u128>()
}

struct Variant {
    ident: syn::Ident,
    expr: Expr,
    high: Option<Expr>,
    inclusive: bool,
    wrapped: bool,
}

fn primitive_name(ident: &syn::Ident) -> syn::Result<&'static str> {
    match ident.to_string().as_str() {
        "u8" => Ok("u8"), "u16" => Ok("u16"), "u32" => Ok("u32"),
        "u64" => Ok("u64"), "u128" => Ok("u128"), "usize" => Ok("usize"),
        "i8" => Ok("i8"), "i16" => Ok("i16"), "i32" => Ok("i32"),
        "i64" => Ok("i64"), "i128" => Ok("i128"), "isize" => Ok("isize"),
        _ => Err(syn::Error::new_spanned(ident, "expected an integer type (e.g. u8 or u16)")),
    }
}

fn inferred_type(numbers: &[Number]) -> &'static str {
    let negative = numbers.iter().any(|n| n.negative);
    let pos_max = numbers.iter().filter(|n| !n.negative)
        .map(|n| n.magnitude).max().unwrap_or(0);
    let neg_max = numbers.iter().filter(|n| n.negative)
        .map(|n| n.magnitude).max().unwrap_or(0);
    if negative {
        if neg_max <= 128 && pos_max <= i8::MAX as u128 { "i8" }
        else if neg_max <= 32768 && pos_max <= i16::MAX as u128 { "i16" }
        else if neg_max <= 2147483648 && pos_max <= i32::MAX as u128 { "i32" }
        else if neg_max <= (1u128 << 63) && pos_max <= i64::MAX as u128 { "i64" }
        else { "i128" }
    } else if pos_max <= u8::MAX as u128 { "u8" }
    else if pos_max <= u16::MAX as u128 { "u16" }
    else if pos_max <= u32::MAX as u128 { "u32" }
    else if pos_max <= u64::MAX as u128 { "u64" }
    else { "u128" }
}

fn fits_integer(number: Number, ty: &str) -> bool {
    let (signed, bits) = match ty {
        "u8" => (false, 8), "u16" => (false, 16), "u32" => (false, 32),
        "u64" => (false, 64), "u128" => (false, 128),
        "usize" => (false, usize::BITS),
        "i8" => (true, 8), "i16" => (true, 16), "i32" => (true, 32),
        "i64" => (true, 64), "i128" => (true, 128),
        "isize" => (true, isize::BITS),
        _ => return false,
    };
    if !signed {
        !number.negative && (bits == 128 || number.magnitude <= (1u128 << bits) - 1)
    } else if number.negative {
        number.magnitude <= (1u128 << (bits - 1))
    } else {
        number.magnitude <= (1u128 << (bits - 1)) - 1
    }
}

fn expand_from_primitive(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let mut explicitly_selected = None::<&'static str>;
    let mut repr_selected = None::<&'static str>;
    for attr in &input.attrs {
        if attr.path().is_ident("from_primitive") {
            if explicitly_selected.is_some() {
                return Err(syn::Error::new_spanned(attr, "duplicate #[from_primitive(...)]"));
            }
            let integer = attr.parse_args::<syn::Ident>()?;
            explicitly_selected = Some(primitive_name(&integer)?);
        } else if attr.path().is_ident("repr") {
            // `repr(C, u16)` is not a single ident, so use punctuated parsing.
            let names = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated
            );
            if let Ok(names) = names {
                for path in names {
                    if let Some(ident) = path.get_ident() {
                        if let Ok(ty) = primitive_name(ident) {
                            repr_selected = Some(ty);
                        }
                    }
                }
            }
        }
    }

    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(name, "FromPrimitive only supports enums"));
    };

    let mut variants = Vec::new();
    let mut numbers = Vec::new();
    let mut fallback = None::<syn::Ident>;
    for variant in &data.variants {
        let wrapped = match &variant.fields {
            Fields::Unit => false,
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => true,
            _ => return Err(syn::Error::new_spanned(&variant.fields,
                "FromPrimitive requires unit variants or one-field tuple variants")),
        };
        let mut value = None::<Expr>;
        for attr in &variant.attrs {
            if attr.path().is_ident("value") {
                if value.is_some() {
                    return Err(syn::Error::new_spanned(attr, "duplicate #[value(...)]"));
                }
                value = Some(attr.parse_args::<Expr>()?);
            }
        }
        if let Some(value) = value {
            match value {
                Expr::Range(range) => {
                    let Some(start) = range.start else {
                        return Err(syn::Error::new_spanned(&variant.ident, "range start required"));
                    };
                    let Some(end) = range.end else {
                        return Err(syn::Error::new_spanned(&variant.ident, "range end required"));
                    };
                    numbers.push(Number::from_expr(&start)?);
                    numbers.push(Number::from_expr(&end)?);
                    let inclusive = matches!(range.limits, RangeLimits::Closed(_));
                    variants.push(Variant { ident: variant.ident.clone(), expr: *start,
                        high: Some(*end), inclusive, wrapped });
                }
                expr => {
                    numbers.push(Number::from_expr(&expr)?);
                    variants.push(Variant { ident: variant.ident.clone(), expr,
                        high: None, inclusive: true, wrapped });
                }
            }
        } else if wrapped {
            if fallback.is_some() {
                return Err(syn::Error::new_spanned(&variant.ident,
                    "only one catch-all tuple variant is allowed"));
            }
            fallback = Some(variant.ident.clone());
        } else {
            return Err(syn::Error::new_spanned(&variant.ident,
                "add #[value(...)] to this variant"));
        }
    }
    if variants.is_empty() && fallback.is_none() {
        return Err(syn::Error::new_spanned(name, "enum must have at least one variant"));
    }
    let selected = explicitly_selected.or(repr_selected)
        .unwrap_or_else(|| inferred_type(&numbers));
    for number in numbers {
        if !fits_integer(number, selected) {
            return Err(syn::Error::new_spanned(name,
                format!("#[value] is not representable as {selected}; use a wider #[from_primitive(...)]")));
        }
    }
    let ty: syn::Type = syn::parse_str(selected)?;
    let arms = variants.iter().map(|variant| {
        let ident = &variant.ident;
        let low = &variant.expr;
        let condition = if let Some(end) = &variant.high {
            if variant.inclusive { quote! { value >= (#low) && value <= (#end) } }
            else { quote! { value >= (#low) && value < (#end) } }
        } else { quote! { value == (#low) } };
        let output = if variant.wrapped { quote! { Self::#ident(value as _) } }
            else { quote! { Self::#ident } };
        quote! { _ if #condition => #output, }
    }).collect::<Vec<_>>();
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Generate checked conversions for the other integer types. Narrowing
    // happens before matching codes so a source value like 256u16 cannot
    // accidentally match a u8 variant after wrapping to zero.
    let mut other_impls = Vec::new();
    for integer in [
        "u8", "u16", "u32", "u64", "u128", "usize",
        "i8", "i16", "i32", "i64", "i128", "isize",
    ] {
        if integer == selected {
            continue;
        }
        let source_ty: syn::Type = syn::parse_str(integer)?;
        other_impls.push(quote! {
            impl #impl_generics ::core::convert::TryFrom<#source_ty>
                for #name #ty_generics #where_clause
            {
                type Error = ();

                fn try_from(value: #source_ty) -> ::core::result::Result<Self, Self::Error> {
                    let narrowed: #ty =
                        <#ty as ::core::convert::TryFrom<#source_ty>>::try_from(value)
                            .map_err(|_| ())?;
                    <Self as ::core::convert::TryFrom<#ty>>::try_from(narrowed)
                        .map_err(|_| ())
                }
            }
        });
    }

    // This inherent method prevents `Enum::try_from(2)` from becoming
    // ambiguous now that the enum also implements TryFrom<u8>, TryFrom<u16>,
    // etc. Unsuffixed literals default to i32; typed values retain their width
    // and undergo a checked conversion to the canonical representation.
    let inherent_try_from = quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            #[inline]
            pub fn try_from(
                value: impl ::core::convert::TryInto<#ty>,
            ) -> ::core::result::Result<Self, ()> {
                let narrowed: #ty = ::core::convert::TryInto::try_into(value)
                    .map_err(|_| ())?;
                <Self as ::core::convert::TryFrom<#ty>>::try_from(narrowed)
                    .map_err(|_| ())
            }
        }
    };

    let primary_impl = if let Some(fallback) = fallback {
        quote! {
            impl #impl_generics ::core::convert::From<#ty> for #name #ty_generics #where_clause {
                fn from(value: #ty) -> Self {
                    match value { #(#arms)* _ => Self::#fallback(value as _), }
                }
            }
        }
    } else {
        let try_arms = variants.iter().map(|variant| {
            let ident = &variant.ident;
            let low = &variant.expr;
            let condition = if let Some(end) = &variant.high {
                if variant.inclusive { quote! { value >= (#low) && value <= (#end) } }
                else { quote! { value >= (#low) && value < (#end) } }
            } else { quote! { value == (#low) } };
            let output = if variant.wrapped { quote! { Self::#ident(value as _) } }
                else { quote! { Self::#ident } };
            quote! { _ if #condition => ::core::result::Result::Ok(#output), }
        }).collect::<Vec<_>>();
        quote! {
            impl #impl_generics ::core::convert::TryFrom<#ty>
                for #name #ty_generics #where_clause
            {
                type Error = ();
                fn try_from(value: #ty) -> ::core::result::Result<Self, Self::Error> {
                    match value {
                        #(#try_arms)*
                        _ => ::core::result::Result::Err(()),
                    }
                }
            }
        }
    };

    Ok(quote! {
        #primary_impl
        #(#other_impls)*
        #inherent_try_from
    })
}
