use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TS2};
use quote::quote;
use std::collections::{BTreeMap, HashSet};
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput, Fields, Ident, LitFloat, LitInt, Type};

// ─── BitPack entry point ──────────────────────────────────────────────────────
#[proc_macro_derive(BitPack, attributes(bitpack))]
pub fn derive_bitpack(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_bitpack(&ast)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

// ─── FromRepr entry point ─────────────────────────────────────────────────────
/// Derives `From<ReprType> for Enum` for `#[repr(uN/iN)]` enums.
///
/// The variant marked `#[default]` (or the first variant if none) is used as
/// the catch-all arm for unknown wire values.
#[proc_macro_derive(FromRepr)]
pub fn derive_from_repr(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_from_repr(&ast)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn repr_int_type(attrs: &[syn::Attribute]) -> syn::Result<TS2> {
    for attr in attrs {
        if !attr.path().is_ident("repr") {
            continue;
        }
        let mut found: Option<TS2> = None;
        attr.parse_nested_meta(|m| {
            if let Some(id) = m.path.get_ident() {
                found = match id.to_string().as_str() {
                    "u8"  => Some(quote!(u8)),
                    "u16" => Some(quote!(u16)),
                    "u32" => Some(quote!(u32)),
                    "u64" => Some(quote!(u64)),
                    "i8"  => Some(quote!(i8)),
                    "i16" => Some(quote!(i16)),
                    "i32" => Some(quote!(i32)),
                    "i64" => Some(quote!(i64)),
                    _     => None,
                };
            }
            Ok(())
        })?;
        if let Some(ts) = found {
            return Ok(ts);
        }
    }
    Err(syn::Error::new(Span::call_site(), "FromRepr requires #[repr(u8/u16/u32/u64/i8/i16/i32/i64)]"))
}

fn impl_from_repr(ast: &DeriveInput) -> syn::Result<TS2> {
    let name = &ast.ident;
    let repr_ty = repr_int_type(&ast.attrs)?;

    let variants = match &ast.data {
        Data::Enum(e) => &e.variants,
        _ => return Err(syn::Error::new(Span::call_site(), "FromRepr only works on enums")),
    };

    for v in variants {
        if !matches!(v.fields, Fields::Unit) {
            return Err(syn::Error::new(v.ident.span(), "FromRepr only supports unit variants"));
        }
    }

    let mut arms: Vec<TS2> = Vec::new();
    let mut default_ident: Option<Ident> = None;
    let mut disc: i64 = 0;

    for variant in variants {
        if let Some((_, expr)) = &variant.discriminant {
            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(int), .. }) = expr {
                disc = int.base10_parse()?;
            } else {
                return Err(syn::Error::new(variant.ident.span(), "FromRepr requires integer literal discriminants"));
            }
        }
        let vi = &variant.ident;
        let disc_lit = proc_macro2::Literal::i64_unsuffixed(disc);
        arms.push(quote!(v if v == #disc_lit as #repr_ty => Self::#vi,));

        if variant.attrs.iter().any(|a| a.path().is_ident("default")) {
            default_ident = Some(vi.clone());
        }
        disc += 1;
    }

    let fallback_ident = default_ident
        .as_ref()
        .unwrap_or(&variants[0].ident);

    Ok(quote! {
        impl From<#repr_ty> for #name {
            fn from(v: #repr_ty) -> Self {
                match v {
                    #(#arms)*
                    _ => Self::#fallback_ident,
                }
            }
        }
    })
}

// ─── Internal data model ──────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq)]
enum Endian {
    Le,
    Be,
}

#[derive(Debug, Clone, Copy)]
enum WordType {
    U8,
    U16,
    U32,
    U64,
}

impl WordType {
    fn from_max_bit(max_bit_exclusive: u32) -> Self {
        match max_bit_exclusive {
            0..=8 => Self::U8,
            9..=16 => Self::U16,
            17..=32 => Self::U32,
            _ => Self::U64,
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "u8" => Some(Self::U8),
            "u16" => Some(Self::U16),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            _ => None,
        }
    }

    fn byte_size(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::U32 => 4,
            Self::U64 => 8,
        }
    }

    fn ts(self) -> TS2 {
        match self {
            Self::U8 => quote!(u8),
            Self::U16 => quote!(u16),
            Self::U32 => quote!(u32),
            Self::U64 => quote!(u64),
        }
    }
}

#[derive(Debug, Clone)]
enum FieldKind {
    Bitfield {
        lo: u32,
        hi: u32, // lo > hi means reversed / MSB-first
        word_hint: Option<WordType>,
    },
    SingleBit {
        bit: u32,
        word_hint: Option<WordType>,
    },
    Scalar {
        endian_override: Option<Endian>,
    },
    Raw {
        len: usize,
    },
    Skip,
}

struct FieldInfo {
    ident: Ident,
    ty: Type,
    byte: usize,
    kind: FieldKind,
    offset: Option<i64>,
    scale: Option<f64>,
    twos_comp: bool,
    /// Intermediate primitive type for enum fields, e.g. `via = "u8"`.
    /// Generates `FieldType::from(_raw as ViaType)` on decode and
    /// `self.field as ViaType as i64` on encode.
    via: Option<TS2>,
}

// ─── Attribute parsing ────────────────────────────────────────────────────────
fn parse_struct_attrs(attrs: &[syn::Attribute]) -> syn::Result<(usize, Endian)> {
    let mut size: Option<usize> = None;
    let mut endian = Endian::Le;
    for attr in attrs {
        if !attr.path().is_ident("bitpack") {
            continue;
        }
        attr.parse_nested_meta(|m| {
            if m.path.is_ident("size") {
                let lit: LitInt = m.value()?.parse()?;
                size = Some(lit.base10_parse()?);
            } else if m.path.is_ident("endian") {
                let lit: syn::LitStr = m.value()?.parse()?;
                endian = match lit.value().as_str() {
                    "le" => Endian::Le,
                    "be" => Endian::Be,
                    s => return Err(m.error(format!("expected \"le\" or \"be\", got {s:?}"))),
                };
            } else {
                return Err(m.error("unknown struct-level bitpack key"));
            }
            Ok(())
        })?;
    }
    let size = size.ok_or_else(|| {
        syn::Error::new(Span::call_site(), "missing #[bitpack(size = N)] on struct")
    })?;
    Ok((size, endian))
}

fn parse_bits_str(s: &str, span: Span) -> syn::Result<(u32, u32)> {
    let parts: Vec<&str> = s.splitn(2, "..").collect();
    if parts.len() != 2 {
        return Err(syn::Error::new(
            span,
            format!("expected \"lo..hi\", got {s:?}"),
        ));
    }
    let lo: u32 = parts[0].trim().parse().map_err(|_| {
        syn::Error::new(span, format!("bad lo in bit range {s:?}"))
    })?;
    let hi: u32 = parts[1].trim().parse().map_err(|_| {
        syn::Error::new(span, format!("bad hi in bit range {s:?}"))
    })?;
    Ok((lo, hi))
}

fn parse_field_attrs(
    attrs: &[syn::Attribute],
    ident: Ident,
    ty: Type,
) -> syn::Result<FieldInfo> {
    let attr = match attrs.iter().find(|a| a.path().is_ident("bitpack")) {
        Some(a) => a,
        None => {
            return Ok(FieldInfo {
                ident,
                ty,
                byte: 0,
                kind: FieldKind::Skip,
                offset: None,
                scale: None,
                twos_comp: false,
                via: None,
            })
        }
    };

    let mut skip = false;
    let mut byte_opt: Option<usize> = None;
    let mut bits_opt: Option<(u32, u32)> = None;
    let mut bit_opt: Option<u32> = None;
    let mut raw_opt: Option<usize> = None;
    let mut word_opt: Option<WordType> = None;
    let mut endian_ov: Option<Endian> = None;
    let mut offset: Option<i64> = None;
    let mut scale: Option<f64> = None;
    let mut twos_comp = false;
    let mut via: Option<TS2> = None;

    attr.parse_nested_meta(|m| {
        if m.path.is_ident("skip") {
            skip = true;
        } else if m.path.is_ident("byte") {
            let lit: LitInt = m.value()?.parse()?;
            byte_opt = Some(lit.base10_parse()?);
        } else if m.path.is_ident("bits") {
            let lit: syn::LitStr = m.value()?.parse()?;
            bits_opt = Some(parse_bits_str(&lit.value(), lit.span())?);
        } else if m.path.is_ident("bit") {
            let lit: LitInt = m.value()?.parse()?;
            bit_opt = Some(lit.base10_parse()?);
        } else if m.path.is_ident("raw") {
            let lit: LitInt = m.value()?.parse()?;
            raw_opt = Some(lit.base10_parse()?);
        } else if m.path.is_ident("word") {
            let lit: syn::LitStr = m.value()?.parse()?;
            word_opt = Some(WordType::from_str(&lit.value()).ok_or_else(|| {
                m.error(format!(
                    "expected u8/u16/u32/u64, got {:?}",
                    lit.value()
                ))
            })?);
        } else if m.path.is_ident("endian") {
            let lit: syn::LitStr = m.value()?.parse()?;
            endian_ov = Some(match lit.value().as_str() {
                "le" => Endian::Le,
                "be" => Endian::Be,
                s => return Err(m.error(format!("expected \"le\" or \"be\", got {s:?}"))),
            });
        } else if m.path.is_ident("offset") {
            let lit: LitInt = m.value()?.parse()?;
            offset = Some(lit.base10_parse()?);
        } else if m.path.is_ident("scale") {
            let lit: LitFloat = m.value()?.parse()?;
            scale = Some(lit.base10_parse()?);
        } else if m.path.is_ident("twos_comp") {
            twos_comp = true;
        } else if m.path.is_ident("via") {
            let lit: syn::LitStr = m.value()?.parse()?;
            let ident = proc_macro2::Ident::new(&lit.value(), lit.span());
            via = Some(quote!(#ident));
        } else {
            return Err(m.error("unknown field-level bitpack key"));
        }
        Ok(())
    })?;

    if skip {
        return Ok(FieldInfo {
            ident,
            ty,
            byte: 0,
            kind: FieldKind::Skip,
            offset: None,
            scale: None,
            twos_comp: false,
            via: None,
        });
    }

    let byte = byte_opt.ok_or_else(|| {
        syn::Error::new(attr.pound_token.span, "missing byte = N in #[bitpack(...)]")
    })?;

    let kind = if let Some((lo, hi)) = bits_opt {
        if twos_comp && (lo >= hi) {
            return Err(syn::Error::new(
                attr.span(),
                "twos_comp not yet implemented for reversed bit order (lo > hi)",
            ));
        }
        FieldKind::Bitfield {
            lo,
            hi,
            word_hint: word_opt,
        }
    } else if let Some(bit) = bit_opt {
        FieldKind::SingleBit {
            bit,
            word_hint: word_opt,
        }
    } else if let Some(len) = raw_opt {
        FieldKind::Raw { len }
    } else {
        FieldKind::Scalar {
            endian_override: endian_ov,
        }
    };

    if twos_comp && !matches!(kind, FieldKind::Bitfield { .. }) {
        return Err(syn::Error::new(
            attr.span(),
            "twos_comp only allowed on bitfields (with bits = \"lo..hi\")",
        ));
    }

    if via.is_some() && !matches!(kind, FieldKind::Bitfield { .. }) {
        return Err(syn::Error::new(
            attr.span(),
            "via only allowed on bitfields (with bits = \"lo..hi\")",
        ));
    }

    Ok(FieldInfo {
        ident,
        ty,
        byte,
        kind,
        offset,
        scale,
        twos_comp,
        via,
    })
}

// ─── Helpers ──────────────────────────────────────────────────────────────────
fn is_float(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        tp.qself.is_none()
            && tp.path.leading_colon.is_none()
            && tp.path.segments.len() == 1
            && ["f32", "f64"].contains(&tp.path.segments[0].ident.to_string().as_str())
    } else {
        false
    }
}

fn is_bool(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        tp.qself.is_none()
            && tp.path.leading_colon.is_none()
            && tp.path.segments.len() == 1
            && tp.path.segments[0].ident == "bool"
    } else {
        false
    }
}

// ─── Code generation ──────────────────────────────────────────────────────────
fn impl_bitpack(ast: &DeriveInput) -> syn::Result<TS2> {
    let name = &ast.ident;
    let (size, default_endian) = parse_struct_attrs(&ast.attrs)?;

    let named_fields = match &ast.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => {
                return Err(syn::Error::new(
                    Span::call_site(),
                    "#[derive(BitPack)] requires a struct with named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new(
                Span::call_site(),
                "#[derive(BitPack)] only works on structs",
            ))
        }
    };

    let mut fields: Vec<FieldInfo> = Vec::new();
    for f in named_fields {
        let ident = f.ident.clone().unwrap();
        let ty = f.ty.clone();
        fields.push(parse_field_attrs(&f.attrs, ident, ty)?);
    }

    // Group word types for bitfield groups
    let mut group_max_bit: BTreeMap<usize, u32> = BTreeMap::new();
    let mut group_word_hint: BTreeMap<usize, WordType> = BTreeMap::new();

    for fi in &fields {
        let (byte, max_bit, hint) = match &fi.kind {
            FieldKind::Bitfield { lo, hi, word_hint } => {
                let max = (*lo).max(*hi);
                (fi.byte, max, *word_hint)
            }
            FieldKind::SingleBit { bit, word_hint } => (fi.byte, bit + 1, *word_hint),
            _ => continue,
        };
        let entry = group_max_bit.entry(byte).or_insert(0);
        *entry = (*entry).max(max_bit);
        if let Some(wt) = hint {
            group_word_hint.insert(byte, wt);
        }
    }

    let group_word: BTreeMap<usize, WordType> = group_max_bit
        .iter()
        .map(|(&byte, &max_bit)| {
            let wt = group_word_hint
                .get(&byte)
                .copied()
                .unwrap_or_else(|| WordType::from_max_bit(max_bit));
            (byte, wt)
        })
        .collect();

    let mut emitted_groups: HashSet<usize> = HashSet::new();
    let mut enc_stmts: Vec<TS2> = Vec::new();
    let mut dec_stmts: Vec<TS2> = Vec::new();

    for fi in &fields {
        match &fi.kind {
            FieldKind::Skip => continue,

            FieldKind::Scalar { endian_override } => {
                let ident = &fi.ident;
                let ty = &fi.ty;
                let off = fi.byte;
                let end = endian_override.unwrap_or(default_endian);
                let sz = quote!(::core::mem::size_of::<#ty>());
                let (enc_fn, dec_fn) = match end {
                    Endian::Le => (quote!(to_le_bytes), quote!(from_le_bytes)),
                    Endian::Be => (quote!(to_be_bytes), quote!(from_be_bytes)),
                };

                let offset_dec = fi.offset.map(|o| quote!(_val = (_val as i64 + #o) as #ty;)).unwrap_or_default();
                let scale_dec = fi.scale.map(|s| {
                    let cast_to_f = if is_float(ty) { quote!(as f64) } else { quote!(as f64) };
                    let cast_from_f = if is_float(ty) {
                        quote!(as #ty)
                    } else {
                        quote!(.round() as #ty)
                    };
                    quote! {
                        _val = (_val #cast_to_f * #s) #cast_from_f;
                    }
                }).unwrap_or_default();

                dec_stmts.push(quote! {
                    let mut _val = #ty::#dec_fn(_buf[#off..#off + #sz].try_into().unwrap());
                    #offset_dec
                    #scale_dec
                    _s.#ident = _val;
                });

                let scale_enc = fi.scale.map(|s| {
                    let div = 1.0 / s;
                    quote! {
                        let _tmp = self.#ident as f64 * #div;
                        let mut _val = _tmp.round() as #ty;
                    }
                }).unwrap_or_else(|| quote!(let mut _val = self.#ident; ));

                let offset_enc = fi.offset.map(|o| quote!(_val = (_val as i64 - #o) as #ty;)).unwrap_or_default();

                enc_stmts.push(quote! {
                    #scale_enc
                    #offset_enc
                    _buf[#off..#off + #sz].copy_from_slice(&_val.#enc_fn());
                });
            }

            FieldKind::Raw { len } => {
                let ident = &fi.ident;
                let off = fi.byte;
                let end = off + len;
                enc_stmts.push(quote! {
                    _buf[#off..#end].copy_from_slice(&self.#ident);
                });
                dec_stmts.push(quote! {
                    _s.#ident = _buf[#off..#end].try_into().unwrap();
                });
            }

            FieldKind::Bitfield { .. } | FieldKind::SingleBit { .. } => {
                let byte = fi.byte;
                if emitted_groups.contains(&byte) {
                    continue;
                }
                emitted_groups.insert(byte);

                let wt = group_word[&byte];
                let wt_ts = wt.ts();
                let wt_sz = wt.byte_size();
                let end_off = byte + wt_sz;
                let (enc_fn, dec_fn) = match default_endian {
                    Endian::Le => (quote!(to_le_bytes), quote!(from_le_bytes)),
                    Endian::Be => (quote!(to_be_bytes), quote!(from_be_bytes)),
                };

                let group_fields: Vec<&FieldInfo> = fields
                    .iter()
                    .filter(|f| f.byte == byte && matches!(f.kind, FieldKind::Bitfield { .. } | FieldKind::SingleBit { .. }))
                    .collect();

                let mut g_enc: Vec<TS2> = Vec::new();
                let mut g_dec: Vec<TS2> = Vec::new();

                for gf in group_fields {
                    let gident = &gf.ident;
                    let gty = &gf.ty;
                    let bool_field = is_bool(gty);

                    match &gf.kind {
                        FieldKind::SingleBit { bit, .. } => {
                            let b = *bit as usize;
                            let mask_val = 1u64;

                            // Encode
                            g_enc.push(quote! {
                                if self.#gident as u64 != 0 {
                                    _w |= (#mask_val as #wt_ts) << #b;
                                }
                            });

                            // Decode (no offset/scale/twos_comp on single bit for simplicity)
                            if bool_field {
                                g_dec.push(quote! {
                                    _s.#gident = (_w >> #b) & 1 != 0;
                                });
                            } else {
                                g_dec.push(quote! {
                                    _s.#gident = ((_w >> #b) & 1) as #gty;
                                });
                            }
                        }

                        FieldKind::Bitfield { lo, hi, .. } => {
                            let lo_val = *lo;
                            let hi_val = *hi;

                            if lo_val < hi_val {
                                // Normal LSB-first
                                let len = (hi_val - lo_val) as u32;
                                let mask = (1u64 << len) - 1;
                                let lo_u = lo_val as usize;

                                // ── Decode ───────────────────────────────────────
                                let sign_extend = if gf.twos_comp {
                                    quote! {
                                        let sign_bit = 1i64 << (#len - 1);
                                        if _raw & sign_bit != 0 {
                                            _raw |= -sign_bit << 1;  // sign extend
                                        }
                                    }
                                } else {
                                    quote!()
                                };

                                let offset_dec = gf.offset.map(|o| quote!(_raw += #o;)).unwrap_or_default();

                                let scale_dec = gf.scale.map(|s| {
                                    let cast_to_f = if is_float(gty) { quote!(as f64) } else { quote!(as f64) };
                                    let cast_from_f = if is_float(gty) {
                                        quote!(as #gty)
                                    } else {
                                        quote!(.round() as #gty)
                                    };
                                    quote! {
                                        _s.#gident = ((_raw #cast_to_f) * #s) #cast_from_f;
                                    }
                                }).unwrap_or_else(|| {
                                    if bool_field {
                                        quote! { _s.#gident = _raw != 0; }
                                    } else if let Some(via_ty) = &gf.via {
                                        quote! { _s.#gident = #gty::from(_raw as #via_ty); }
                                    } else {
                                        quote! { _s.#gident = _raw as #gty; }
                                    }
                                });

                                g_dec.push(quote! {
                                    let mut _raw: i64 = ((_w as u64 >> #lo_u) & #mask) as i64;
                                    #sign_extend
                                    #offset_dec
                                    #scale_dec
                                });

                                // ── Encode ───────────────────────────────────────
                                let scale_enc = gf.scale.map(|s| {
                                    let div = 1.0 / s;
                                    quote! {
                                        let _tmp = self.#gident as f64 * #div;
                                        let mut _v_int = _tmp.round() as i64;
                                    }
                                }).unwrap_or_else(|| {
                                    if let Some(via_ty) = &gf.via {
                                        quote!(let mut _v_int = self.#gident as #via_ty as i64;)
                                    } else {
                                        quote!(let mut _v_int = self.#gident as i64;)
                                    }
                                });

                                let offset_enc = gf.offset.map(|o| quote!(_v_int -= #o;)).unwrap_or_default();

                                let value_insert = quote!((_v_int as u64) & #mask);

                                g_enc.push(quote! {
                                    #scale_enc
                                    #offset_enc
                                    let _v: u64 = #value_insert;
                                    _w |= (_v << #lo_u) as #wt_ts;
                                });
                            } else {
                                // Reversed (MSB-first) ── simplified, no twos_comp/scale yet
                                let len = (lo_val - hi_val + 1) as usize;
                                let len32 = len as u32;
                                let mask = (1u64 << len) - 1;
                                let hi_u = hi_val as usize;
                                let shift = 64 - len32;

                                g_enc.push(quote! {
                                    let _v: u64 = (self.#gident as u64 & #mask).reverse_bits() >> #shift;
                                    _w |= (_v << #hi_u) as #wt_ts;
                                });

                                g_dec.push(quote! {
                                    let _raw: u64 = ((_w as u64 >> #hi_u) & #mask).reverse_bits() >> #shift;
                                    _s.#gident = _raw as #gty;
                                });
                            }
                        }
                        _ => unreachable!(),
                    }
                }

                enc_stmts.push(quote! {
                    {
                        let mut _w: #wt_ts = 0;
                        #(#g_enc)*
                        _buf[#byte..#end_off].copy_from_slice(&_w.#enc_fn());
                    }
                });

                dec_stmts.push(quote! {
                    {
                        let _w = #wt_ts::#dec_fn(_buf[#byte..#end_off].try_into().unwrap());
                        #(#g_dec)*
                    }
                });
            }
        }
    }

    Ok(quote! {
        impl #name {
            pub fn encode(&self) -> ::std::vec::Vec<u8> {
                let mut _buf = vec![0u8; #size];
                #(#enc_stmts)*
                _buf
            }

            pub fn decode(_buf: &[u8]) -> Self {
                assert!(_buf.len() >= #size,
                    "buffer too short: need {} bytes, got {}", #size, _buf.len());
                let mut _s = Self::default();
                #(#dec_stmts)*
                _s
            }
        }
    })
}