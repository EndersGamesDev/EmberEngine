//! Derive the exact Serde subset used at Ember's script boundaries.

use proc_macro::TokenStream;
use quote::quote;
use serde_rename_rule::RenameRule;
use syn::{
    Attribute, Data, DataEnum, DeriveInput, Expr, Fields, GenericArgument, LitStr, PathArguments,
    Token, Type, parse_macro_input,
};

type Tokens = proc_macro2::TokenStream;
type Parts = (Tokens, Vec<LitStr>);
const UNREGISTERED: &str = "unregistered generic instantiation is unsupported";

#[proc_macro_derive(Boundary, attributes(boundary, serde))]
/// Derives a compiler-owned script-boundary description.
///
/// An intersected payload must not own the enclosing tag key.
///
/// ```compile_fail
/// use ember_boundary_derive::Boundary;
///
/// #[derive(Boundary)]
/// #[boundary(direction = "input")]
/// struct Payload { t: u8 }
///
/// #[derive(Boundary)]
/// #[boundary(direction = "input")]
/// #[serde(tag = "t")]
/// enum Collision {
///     #[boundary(intersection)]
///     Value(Payload),
/// }
/// ```
///
/// An intersected payload must itself describe an object or tagged enum.
///
/// ```compile_fail
/// use ember_boundary_derive::Boundary;
///
/// #[derive(Boundary)]
/// #[boundary(direction = "input")]
/// enum UnitPayload { Value }
///
/// #[derive(Boundary)]
/// #[boundary(direction = "input")]
/// #[serde(tag = "t")]
/// enum InvalidIntersection {
///     #[boundary(intersection)]
///     Value(UnitPayload),
/// }
/// ```
pub fn boundary(input: TokenStream) -> TokenStream {
    expand(parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

struct Container {
    direction: Tokens,
    tag: Option<LitStr>,
    rename: Option<RenameRule>,
    deny_unknown_fields: bool,
}

fn expand(input: DeriveInput) -> syn::Result<Tokens> {
    if !input.generics.params.is_empty() {
        return Err(error(
            input.generics,
            "unregistered generic instantiation: Boundary types cannot be generic",
        ));
    }
    let options = container(&input.attrs)?;
    let name = input.ident;
    let (shape, keys) = match input.data {
        Data::Struct(data) => {
            if options.tag.is_some() {
                return Err(error(name, "tagged struct shape is unsupported"));
            }
            let Fields::Named(fields) = data.fields else {
                return Err(error(
                    data.fields,
                    "tuple or unit struct shape is unsupported; use a named struct",
                ));
            };
            let (fields, keys) = fields_tokens(&fields.named, options.rename)?;
            (
                quote!(::ember_boundary::Shape::Object(&[#(#fields),*])),
                keys,
            )
        }
        Data::Enum(data) => enum_tokens(data, &options)?,
        Data::Union(data) => return Err(error(data.union_token, "union shape is unsupported")),
    };
    let direction = options.direction;
    let deny_unknown_fields = options.deny_unknown_fields;
    Ok(quote! {
        impl ::ember_boundary::Boundary for #name {
            const DESCRIPTION: ::ember_boundary::Description = ::ember_boundary::Description {
                name: stringify!(#name),
                direction: #direction,
                deny_unknown_fields: #deny_unknown_fields,
                shape: #shape,
            };
            const OBJECT_KEYS: &'static [&'static str] = &[#(#keys),*];
        }
    })
}

fn container(attrs: &[Attribute]) -> syn::Result<Container> {
    let mut direction = None;
    let mut tag = None;
    let mut rename = None;
    let mut deny_unknown_fields = false;
    for attr in attrs {
        if attr.path().is_ident("boundary") {
            attr.parse_nested_meta(|meta| {
                if !meta.path.is_ident("direction") {
                    return Err(meta.error("unsupported boundary attribute; expected direction"));
                }
                let value: LitStr = meta.value()?.parse()?;
                direction = Some(match value.value().as_str() {
                    "input" => quote!(::ember_boundary::Direction::Input),
                    "output" => quote!(::ember_boundary::Direction::Output),
                    "both" => quote!(::ember_boundary::Direction::Both),
                    _ => return Err(syn::Error::new(value.span(), "unknown boundary direction")),
                });
                Ok(())
            })?;
        } else if attr.path().is_ident("serde") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("tag") {
                    tag = Some(meta.value()?.parse()?);
                } else if meta.path.is_ident("rename_all") {
                    let value: LitStr = meta.value()?.parse()?;
                    rename = Some(
                        RenameRule::from_rename_all_str(&value.value())
                            .map_err(|error| syn::Error::new(value.span(), error.to_string()))?,
                    );
                } else if meta.path.is_ident("deny_unknown_fields") {
                    deny_unknown_fields = true;
                } else {
                    return Err(meta.error("unsupported serde container shape"));
                }
                Ok(())
            })?;
        }
    }
    Ok(Container {
        direction: direction.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), "missing boundary direction")
        })?,
        tag,
        rename,
        deny_unknown_fields,
    })
}

fn enum_tokens(data: DataEnum, options: &Container) -> syn::Result<Parts> {
    let mut variants = Vec::new();
    let mut keys = options.tag.iter().cloned().collect::<Vec<_>>();
    for variant in data.variants {
        let intersection = variant_options(&variant.attrs)?;
        let rust_name = variant.ident.to_string();
        let wire_name = options.rename.map_or_else(
            || rust_name.clone(),
            |rule| rule.apply_to_variant(&rust_name),
        );
        let wire_name = LitStr::new(&wire_name, variant.ident.span());
        let shape = match variant.fields {
            Fields::Unit if !intersection => quote!(::ember_boundary::VariantShape::Unit),
            Fields::Named(fields) if !intersection && options.tag.is_some() => {
                let (fields, field_keys) = fields_tokens(&fields.named, None)?;
                keys.extend(field_keys);
                quote!(::ember_boundary::VariantShape::Object(&[#(#fields),*]))
            }
            Fields::Unnamed(fields)
                if intersection && options.tag.is_some() && fields.unnamed.len() == 1 =>
            {
                let ty = &fields.unnamed[0].ty;
                let ty_ref = type_ref(ty, None)?;
                let tag = options.tag.as_ref().expect("checked above");
                quote!({
                    const _: () = ::ember_boundary::assert_intersection::<#ty>(#tag);
                    ::ember_boundary::VariantShape::Intersection(#ty_ref)
                })
            }
            Fields::Unnamed(fields) if fields.unnamed.len() > 1 => {
                return Err(error(
                    fields,
                    "multi-field tuple variant shape is unsupported",
                ));
            }
            Fields::Unnamed(fields) => {
                return Err(error(
                    fields,
                    "newtype variant shape is unsupported unless marked boundary(intersection)",
                ));
            }
            Fields::Named(fields) => {
                return Err(error(
                    fields,
                    "named enum variant shape requires an internal serde tag",
                ));
            }
            Fields::Unit => {
                return Err(error(
                    variant.ident,
                    "boundary(intersection) requires one named payload type",
                ));
            }
        };
        variants.push(quote! {
            ::ember_boundary::Variant { name: #wire_name, shape: #shape }
        });
    }
    let tag = options
        .tag
        .as_ref()
        .map_or_else(|| quote!(None), |tag| quote!(Some(#tag)));
    Ok((
        quote!(::ember_boundary::Shape::Enum { tag: #tag, variants: &[#(#variants),*] }),
        keys,
    ))
}

fn fields_tokens(
    fields: &syn::punctuated::Punctuated<syn::Field, Token![,]>,
    rename: Option<RenameRule>,
) -> syn::Result<(Vec<Tokens>, Vec<LitStr>)> {
    let mut output = Vec::new();
    let mut keys = Vec::new();
    for field in fields {
        let ident = field.ident.as_ref().expect("named field");
        let name = ident.to_string();
        let name = rename.map_or_else(|| name.clone(), |rule| rule.apply_to_field(&name));
        let name = LitStr::new(&name, ident.span());
        let options = field_options(&field.attrs)?;
        let nullable = is_option(&field.ty);
        if options.omit_none && !nullable {
            return Err(error(field, "boundary(omit_none) requires an Option field"));
        }
        let wide = wide_tokens(field, &options)?;
        let ty = type_ref(&field.ty, wide.as_ref())?;
        let input_optional = options.default || nullable;
        let output_optional = options.omit_none;
        output.push(quote! {
            ::ember_boundary::Field {
                name: #name,
                ty: #ty,
                input_optional: #input_optional,
                output_optional: #output_optional,
            }
        });
        keys.push(name);
    }
    Ok((output, keys))
}

#[derive(Default)]
struct FieldOptions {
    default: bool,
    omit_none: bool,
    wide: Option<LitStr>,
    bound: Option<LitStr>,
}

fn field_options(attrs: &[Attribute]) -> syn::Result<FieldOptions> {
    let mut default = false;
    let mut omit_none = false;
    let mut wide = None;
    let mut bound = None;
    for attr in attrs {
        if attr.path().is_ident("boundary") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("omit_none") {
                    omit_none = true;
                } else if meta.path.is_ident("wide") {
                    if wide.is_some() {
                        return Err(meta.error("duplicate boundary wide declaration"));
                    }
                    wide = Some(meta.value()?.parse()?);
                } else if meta.path.is_ident("bound") {
                    if bound.is_some() {
                        return Err(meta.error("duplicate boundary precision bound"));
                    }
                    bound = Some(meta.value()?.parse()?);
                } else {
                    return Err(meta.error("unsupported boundary field attribute"));
                }
                Ok(())
            })?;
        }
        if attr.path().is_ident("serde") {
            attr.parse_nested_meta(|meta| {
                if !meta.path.is_ident("default") {
                    return Err(meta.error("unsupported serde field shape"));
                }
                default = true;
                if meta.input.peek(Token![=]) {
                    let _expression: Expr = meta.value()?.parse()?;
                }
                Ok(())
            })?;
        }
    }
    Ok(FieldOptions {
        default,
        omit_none,
        wide,
        bound,
    })
}

fn wide_tokens(field: &syn::Field, options: &FieldOptions) -> syn::Result<Option<Tokens>> {
    let Some(wide) = &options.wide else {
        if contains_wide_integer(&field.ty) {
            let name = field.ident.as_ref().expect("named field");
            return Err(error(
                field,
                &format!(
                    "64-bit boundary field `{name}` must declare boundary(wide = \"exact\") or boundary(wide = \"precise\", bound = \"...\")"
                ),
            ));
        }
        if let Some(bound) = &options.bound {
            return Err(error(
                bound,
                "boundary precision bound requires boundary(wide = \"precise\")",
            ));
        }
        return Ok(None);
    };
    if !contains_wide_integer(&field.ty) {
        return Err(error(
            wide,
            "boundary wide declaration requires a 64-bit integer field",
        ));
    }
    match wide.value().as_str() {
        "exact" => {
            if let Some(bound) = &options.bound {
                return Err(error(
                    bound,
                    "boundary(wide = \"exact\") cannot declare a precision bound",
                ));
            }
            Ok(Some(quote!(::ember_boundary::Wide::Exact)))
        }
        "precise" => {
            let bound = options.bound.as_ref().ok_or_else(|| {
                error(
                    wide,
                    "boundary(wide = \"precise\") requires a non-empty bound",
                )
            })?;
            if bound.value().is_empty() {
                return Err(error(
                    bound,
                    "boundary(wide = \"precise\") requires a non-empty bound",
                ));
            }
            Ok(Some(
                quote!(::ember_boundary::Wide::Precise { bound: #bound }),
            ))
        }
        _ => Err(error(
            wide,
            "unknown boundary wide representation; expected exact or precise",
        )),
    }
}

fn contains_wide_integer(ty: &Type) -> bool {
    match ty {
        Type::Array(array) => contains_wide_integer(&array.elem),
        Type::Tuple(tuple) => tuple.elems.iter().any(contains_wide_integer),
        Type::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            let segment = path.path.segments.first().expect("one segment");
            if matches!(segment.ident.to_string().as_str(), "u64" | "i64") {
                return true;
            }
            let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                return false;
            };
            arguments.args.iter().any(|argument| {
                matches!(argument, GenericArgument::Type(inner) if contains_wide_integer(inner))
            })
        }
        _ => false,
    }
}

fn is_option(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    path.qself.is_none() && path.path.segments.len() == 1 && path.path.segments[0].ident == "Option"
}

fn variant_options(attrs: &[Attribute]) -> syn::Result<bool> {
    let mut intersection = false;
    for attr in attrs {
        if attr.path().is_ident("serde") {
            return Err(error(
                attr,
                "per-item serde rename, skip, or serializer shape is unsupported",
            ));
        }
        if attr.path().is_ident("boundary") {
            attr.parse_nested_meta(|meta| {
                if !meta.path.is_ident("intersection") {
                    return Err(meta.error("unsupported boundary variant attribute"));
                }
                intersection = true;
                Ok(())
            })?;
        }
    }
    Ok(intersection)
}

fn type_ref(ty: &Type, wide: Option<&Tokens>) -> syn::Result<Tokens> {
    if let Type::Array(array) = ty {
        let inner = type_ref(&array.elem, wide)?;
        let len = &array.len;
        return Ok(quote!(::ember_boundary::TypeRef::Array(&#inner, #len)));
    }
    if let Type::Tuple(tuple) = ty {
        let items = tuple
            .elems
            .iter()
            .map(|item| type_ref(item, wide))
            .collect::<syn::Result<Vec<_>>>()?;
        return Ok(quote!(::ember_boundary::TypeRef::Tuple(&[#(#items),*])));
    }
    let Type::Path(path) = ty else {
        return Err(error(ty, "unsupported boundary field type"));
    };
    if path.qself.is_some() || path.path.segments.len() != 1 {
        return named_path(ty, &path.path);
    }
    let segment = path.path.segments.first().expect("one segment");
    let ident = segment.ident.to_string();
    if matches!(ident.as_str(), "usize" | "isize") {
        return Err(error(
            ty,
            "ambiguous integer lowering: target-sized integers are unsupported",
        ));
    }
    if let PathArguments::AngleBracketed(args) = &segment.arguments {
        if !matches!(ident.as_str(), "Option" | "Vec") || args.args.len() != 1 {
            return Err(error(ty, UNREGISTERED));
        }
        let Some(GenericArgument::Type(inner)) = args.args.first() else {
            return Err(syn::Error::new_spanned(ty, "unsupported generic argument"));
        };
        let inner = type_ref(inner, wide)?;
        return Ok(if ident == "Option" {
            quote!(::ember_boundary::TypeRef::Nullable(&#inner))
        } else {
            quote!(::ember_boundary::TypeRef::Sequence(&#inner))
        });
    }
    let scalar = match ident.as_str() {
        "bool" => quote!(::ember_boundary::TypeRef::Bool),
        "String" => quote!(::ember_boundary::TypeRef::String),
        "f32" => quote!(::ember_boundary::TypeRef::Number { bits: 32 }),
        "f64" => quote!(::ember_boundary::TypeRef::Number { bits: 64 }),
        "u8" | "u16" | "u32" | "u64" => integer(ty, &ident, false, wide)?,
        "i8" | "i16" | "i32" | "i64" => integer(ty, &ident, true, wide)?,
        "u128" | "i128" => {
            return Err(error(
                ty,
                "ambiguous integer lowering: integers wider than JSON's portable range are unsupported",
            ));
        }
        _ => return named_path(ty, &path.path),
    };
    Ok(scalar)
}

fn integer(ty: &Type, ident: &str, signed: bool, wide: Option<&Tokens>) -> syn::Result<Tokens> {
    let bits = ident
        .trim_start_matches(['u', 'i'])
        .parse::<u8>()
        .expect("integer width");
    let wide = if bits == 64 {
        let wide = wide.ok_or_else(|| {
            error(
                ty,
                "64-bit boundary integer must declare exact or precise representation",
            )
        })?;
        quote!(Some(#wide))
    } else {
        quote!(None)
    };
    Ok(quote!(::ember_boundary::TypeRef::Integer { signed: #signed, bits: #bits, wide: #wide }))
}

fn named_path(ty: &Type, path: &syn::Path) -> syn::Result<proc_macro2::TokenStream> {
    if path
        .segments
        .iter()
        .any(|segment| !matches!(segment.arguments, PathArguments::None))
    {
        return Err(error(ty, UNREGISTERED));
    }
    Ok(quote! {
        ::ember_boundary::TypeRef::Named {
            name: stringify!(#ty),
            describe: ::ember_boundary::describe::<#ty>,
        }
    })
}

fn error(tokens: impl quote::ToTokens, message: &str) -> syn::Error {
    syn::Error::new_spanned(tokens, message)
}
