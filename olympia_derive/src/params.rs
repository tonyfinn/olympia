use crate::errors;
use crate::errors::DeriveError;

use olympia_core::derive::{AppendedParam, InnerParam, OpcodePosition, ParamPosition};

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_quote;
use syn::spanned::Spanned;

#[derive(Debug, Clone)]
pub(crate) struct ParamBuilder {
    name: Option<syn::Ident>,
    declared_type: Option<syn::Type>,
    span: Option<proc_macro2::Span>,
    pos: Option<ParamPosition>,
    mask: Option<DeclaredMask>,
    constant: Option<ConstantValue>,
}

pub(crate) struct ParsedParam {
    name: syn::Ident,
    declared_type: syn::Type,
    pos: ParamPosition,
    param_type: ParsedParamType,
}

#[derive(Debug, Clone)]
pub(crate) enum ParsedConstantType {
    ByteRegister,
    WordRegister,
    ByteRegisterOffset,
    LiteralAddress,
}

pub(crate) enum ParsedParamType {
    Appended(AppendedParam),
    Inner {
        pos: OpcodePosition,
        ty: InnerParam,
    },
    Constant {
        value: ConstantValue,
        ty: ParsedConstantType,
    },
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub(crate) struct ConstantValue(syn::Path);
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub(crate) struct DeclaredMask(u8);
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub(crate) struct RuntimeMask(u8);

impl ParamBuilder {
    pub(crate) fn build(&self, opcode_mask: u32) -> errors::ParamResult<ParsedParam> {
        Ok(ParsedParam {
            name: self.name.clone().unwrap(),
            declared_type: self.declared_type.clone().unwrap(),
            param_type: self.determine_param_type(opcode_mask)?,
            pos: self
                .pos
                .ok_or_else(|| errors::ParamError::MissingPosition(self.span.unwrap()))?,
        })
    }

    fn determine_param_type(&self, opcode_mask: u32) -> errors::ParamResult<ParsedParamType> {
        let ty = &self.declared_type.as_ref().unwrap();
        if self.mask.is_some() && self.constant.is_some() {
            Err(errors::ParamError::MaskAndConstant(self.span.unwrap()))
        } else if let Some(declared_mask) = self.mask {
            let runtime_mask = find_runtime_mask(opcode_mask, declared_mask);
            let shift = find_shift(opcode_mask, declared_mask);
            determine_inner_param_type(ty).map(|ip| ParsedParamType::Inner {
                ty: ip,
                pos: OpcodePosition {
                    mask: runtime_mask.0,
                    shift,
                },
            })
        } else if let Some(value) = &self.constant {
            determine_constant_param_type(ty).map(|ty| ParsedParamType::Constant {
                value: value.clone(),
                ty,
            })
        } else {
            determine_appended_param_type(ty).map(ParsedParamType::Appended)
        }
    }
}

impl Default for ParamBuilder {
    fn default() -> Self {
        ParamBuilder {
            name: None,
            pos: None,
            declared_type: None,
            span: None,
            mask: None,
            constant: None,
        }
    }
}

fn determine_constant_param_type(ty: &syn::Type) -> errors::ParamResult<ParsedConstantType> {
    if let syn::Type::Path(ty_path) = ty {
        let type_name = &ty_path.path.segments.last().unwrap().ident;
        Ok(match type_name.to_string().as_str() {
            "ByteRegister" => ParsedConstantType::ByteRegister,
            "ByteRegisterOffset" => ParsedConstantType::ByteRegisterOffset,
            "WordRegister" => ParsedConstantType::WordRegister,
            "LiteralAddress" => ParsedConstantType::LiteralAddress,
            _ => {
                return Err(errors::ParamError::UnsupportedConstantType(Box::new(
                    ty.clone(),
                )))
            }
        })
    } else {
        Err(errors::ParamError::UnsupportedEmbeddedType(Box::new(
            ty.clone(),
        )))
    }
}

fn determine_inner_param_type(ty: &syn::Type) -> errors::ParamResult<InnerParam> {
    if let syn::Type::Path(ty_path) = ty {
        let type_name = &ty_path.path.segments.last().unwrap().ident;
        Ok(match type_name.to_string().as_str() {
            "ALOp" => InnerParam::ALOp,
            "Increment" => InnerParam::Increment,
            "RotateDirection" => InnerParam::RotateDirection,
            "Carry" => InnerParam::Carry,
            "Condition" => InnerParam::Condition,
            "ByteRegisterTarget" => InnerParam::ByteRegisterTarget,
            "AccRegister" => InnerParam::AccRegister,
            "StackRegister" => InnerParam::StackRegister,
            _ => {
                return Err(errors::ParamError::UnsupportedEmbeddedType(Box::new(
                    ty.clone(),
                )))
            }
        })
    } else {
        Err(errors::ParamError::UnsupportedEmbeddedType(Box::new(
            ty.clone(),
        )))
    }
}

fn determine_appended_param_type(ty: &syn::Type) -> errors::ParamResult<AppendedParam> {
    if let syn::Type::Path(ty_path) = &ty {
        let type_name = &ty_path.path.segments.last().unwrap().ident;
        match type_name.to_string().as_str() {
            "u8" => Ok(AppendedParam::Literal8),
            "u16" => Ok(AppendedParam::Literal16),
            "i8" => Ok(AppendedParam::LiteralSigned8),
            "LiteralAddress" => Ok(AppendedParam::LiteralAddress),
            "HighAddress" => Ok(AppendedParam::HighAddress),
            "AddressOffset" => Ok(AppendedParam::AddressOffset),
            _ => Err(errors::ParamError::UnsupportedAppendedType(Box::new(
                ty.clone(),
            ))),
        }
    } else {
        Err(errors::ParamError::UnsupportedAppendedType(Box::new(
            ty.clone(),
        )))
    }
}

fn find_shift(opcode_mask: u32, param_mask: DeclaredMask) -> u8 {
    for i in 0..8 {
        let shift = i * 4;
        let hex_digit = (opcode_mask >> shift) & 0xF;
        if hex_digit as u8 == param_mask.0 {
            return i;
        }
    }
    0
}

fn find_runtime_mask(opcode_mask: u32, param_mask: DeclaredMask) -> RuntimeMask {
    let mut runtime_mask = 0;
    for i in 0..8 {
        let shift = i * 4;
        let hex_digit = (opcode_mask >> shift) & 0xF;
        if hex_digit as u8 == param_mask.0 {
            runtime_mask |= 1 << i;
        }
    }
    RuntimeMask(runtime_mask)
}

fn quote_opcode_position(position: OpcodePosition) -> TokenStream {
    let mask = position.mask;
    let shift = position.shift;
    quote! {
        ::olympia_core::derive::OpcodePosition {
            mask: #mask,
            shift: #shift,
        }
    }
}

fn parse_param_path(pb: &mut ParamBuilder, path: &syn::Path) {
    if path.is_ident("dest") {
        pb.pos = Some(ParamPosition::Dest);
    } else if path.is_ident("src") {
        pb.pos = Some(ParamPosition::Src);
    } else if path.is_ident("single") {
        pb.pos = Some(ParamPosition::Single);
    }else if path.is_ident("addsrc") {
        pb.pos = Some(ParamPosition::AddSrc);
    }
}

fn parse_param_name_value(
    pb: &mut ParamBuilder,
    nv: &syn::MetaNameValue,
) -> errors::ParamResult<()> {
    if nv.path.is_ident("mask") {
        match &nv.lit {
            syn::Lit::Int(val) => {
                pb.mask = val.base10_parse().map(DeclaredMask).ok();
                Ok(())
            }
            _ => Err(errors::ParamError::InvalidParamMask(nv.span())),
        }
    } else {
        Err(errors::ParamError::UnknownField(nv.path.clone()))
    }
}

fn parse_nested_meta_list(pb: &mut ParamBuilder, ml: &syn::MetaList) -> errors::ParamResult<()> {
    if ml.path.is_ident("constant") {
        if ml.nested.len() != 1 {
            Err(errors::ParamError::MultipleConstantValues(ml.clone()))
        } else if let Some(syn::NestedMeta::Meta(syn::Meta::Path(val))) = ml.nested.first() {
            pb.constant = Some(ConstantValue(val.clone()));
            Ok(())
        } else {
            Err(errors::ParamError::ConstantNotPath(ml.clone()))
        }
    } else {
        Err(errors::ParamError::UnknownField(ml.path.clone()))
    }
}

fn parse_nested_meta(pb: &mut ParamBuilder, item: &syn::NestedMeta) -> errors::ParamResult<()> {
    match item {
        syn::NestedMeta::Meta(syn::Meta::Path(path)) => {
            parse_param_path(pb, path);
            Ok(())
        }
        syn::NestedMeta::Meta(syn::Meta::NameValue(nv)) => {
            parse_param_name_value(pb, nv)?;
            Ok(())
        }
        syn::NestedMeta::Meta(syn::Meta::List(ml)) => {
            parse_nested_meta_list(pb, ml)?;
            Ok(())
        }
        syn::NestedMeta::Lit(lit) => Err(errors::ParamError::UnexpectedLiteral(lit.clone())),
    }
}

fn parse_meta_list(pb: &mut ParamBuilder, items: syn::MetaList) -> errors::ParamResult<()> {
    let possible_errs = items
        .nested
        .iter()
        .map(|item| parse_nested_meta(pb, item))
        .filter_map(|item| item.err())
        .collect();
    errors::ParamError::ok_or_group_errors(possible_errs)
}

pub(crate) fn parse_param(field: &syn::Field) -> errors::ParamResult<ParamBuilder> {
    let mut pb = ParamBuilder::default();
    pb.name = field.ident.clone();
    pb.declared_type = Some(field.ty.clone());
    pb.span = Some(field.span());
    for attr in &field.attrs {
        let meta = attr.parse_meta()?;
        pb.declared_type = Some(field.ty.clone());
        if !meta.path().is_ident("olympia") {
            continue;
        }
        match meta {
            syn::Meta::Path(path) => parse_param_path(&mut pb, &path),
            syn::Meta::List(metalist) => parse_meta_list(&mut pb, metalist)?,
            syn::Meta::NameValue(nv) => return Err(errors::ParamError::UnknownField(nv.path)),
        }
    }
    Ok(pb)
}

pub(crate) fn build_opcode_constructor_statements(params: &[ParsedParam]) -> Vec<TokenStream> {
    let mut output = Vec::new();

    for param in params {
        if let ParsedParamType::Inner { pos, .. } = param.param_type {
            let ident = &param.name;
            let ty = &param.declared_type;
            let position = quote_opcode_position(pos);
            output.push(quote! {
                let #ident = <#ty as ::olympia_core::derive::EmbeddableParam>::extract_from_opcode(
                    opcode, #position
                ).unwrap();
            })
        }
    }

    output
}

pub(crate) fn build_inner_param_extractor(params: &[ParsedParam]) -> TokenStream {
    let inner_param_names: Vec<&syn::Ident> = params
        .iter()
        .filter_map(|param| match param.param_type {
            ParsedParamType::Inner { .. } => Some(&param.name),
            _ => None,
        })
        .collect();

    quote! {
        let Self { #(#inner_param_names),* } = self;
    }
}

pub(crate) fn build_into_instruction_constant_params(params: &[ParsedParam]) -> Vec<TokenStream> {
    params
        .iter()
        .filter_map(|param| {
            let name = &param.name;
            let declared_type = &param.declared_type;
            match &param.param_type {
                ParsedParamType::Constant { value, .. } => {
                    let value_path = &value.0;
                    Some(quote! {
                        let #name: #declared_type = #value_path.into();
                    })
                }
                _ => None,
            }
        })
        .collect()
}

pub(crate) fn build_into_instruction_appended_params(params: &[ParsedParam]) -> Vec<TokenStream> {
    params
        .iter()
        .filter_map(|param| {
            let name = &param.name;
            let ty = &param.declared_type;
            match param.param_type {
                ParsedParamType::Appended(AppendedParam::LiteralAddress)
                | ParsedParamType::Appended(AppendedParam::Literal16) => Some(quote! {
                    let #name: #ty = u16::from_le_bytes([
                        iter.next().unwrap_or(0), iter.next().unwrap_or(0)
                    ]).into();
                }),
                ParsedParamType::Appended(AppendedParam::Literal8) => Some(quote! {
                    let #name: #ty = iter.next().unwrap_or(0);
                }),
                ParsedParamType::Appended(AppendedParam::LiteralSigned8) => Some(quote! {
                    let #name: #ty = i8::from_le_bytes([
                        iter.next().unwrap_or(0)
                    ]);
                }),
                ParsedParamType::Appended(AppendedParam::HighAddress)
                | ParsedParamType::Appended(AppendedParam::AddressOffset) => Some(quote! {
                    let #name: #ty = iter.next().unwrap_or(0).into();
                }),
                _ => None,
            }
        })
        .collect()
}

pub(crate) fn get_param_names(params: &[ParsedParam]) -> Vec<&syn::Ident> {
    params.iter().map(|param| &param.name).collect()
}

pub(crate) fn get_inner_fields(params: &[ParsedParam]) -> Vec<(syn::Ident, syn::Type)> {
    let mut output = Vec::new();

    for param in params {
        if let ParsedParamType::Inner { .. } = param.param_type {
            output.push((param.name.clone(), param.declared_type.clone()))
        }
    }

    output
}

impl ParsedParam {
    pub(crate) fn quote_definition(&self) -> syn::Result<TokenStream> {
        let module_base = "::olympia_core::derive";
        let pt_expr: syn::Expr = match &self.param_type {
            ParsedParamType::Inner { pos, ty } => {
                let type_name: syn::Path =
                    syn::parse_str(&format!("{}::InnerParam::{:?}", module_base, ty))?;
                let pos = quote_opcode_position(*pos);
                parse_quote!(
                    ::olympia_core::derive::ParamType::Inner {
                        pos: #pos,
                        ty: #type_name,
                    }
                )
            }
            ParsedParamType::Appended(appended) => {
                let type_name: syn::Path =
                    syn::parse_str(&format!("{}::AppendedParam::{:?}", module_base, appended))?;
                parse_quote!(
                    ::olympia_core::derive::ParamType::Appended(#type_name)
                )
            }
            ParsedParamType::Constant { value, ty } => {
                let type_name: syn::Path =
                    syn::parse_str(&format!("{}::ConstantParam::{:?}", module_base, ty))?;
                let ConstantValue(path) = value;
                parse_quote!(
                    ::olympia_core::derive::ParamType::Constant(
                        #type_name(#path)
                    )
                )
            }
        };
        let position: syn::Expr =
            syn::parse_str(&format!("{}::ParamPosition::{:?}", module_base, self.pos,))?;
        let quoted = quote!(::olympia_core::derive::ParamDefinition {
            pos: #position,
            param_type: #pt_expr,
        });

        Ok(quoted)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_build_param_mask() {
        fn assert_param_mask(instr_mask: u32, param_mask_value: DeclaredMask, result: RuntimeMask) {
            let mask = find_runtime_mask(instr_mask, param_mask_value);
            assert_eq!(mask, result);
        }
        assert_param_mask(0x11BB_AA01, DeclaredMask(0xB), RuntimeMask(0x30));
        assert_param_mask(0x1100_00A1, DeclaredMask(0xA), RuntimeMask(0x02));
    }

    #[test]
    fn test_build_param_shift() {
        fn assert_param_shift(instr_mask: u32, param_mask_value: DeclaredMask, result: u8) {
            let shift = find_shift(instr_mask, param_mask_value);
            assert_eq!(shift, result);
        }
        assert_param_shift(0x11BB_AA01, DeclaredMask(0xB), 4);
        assert_param_shift(0x1100_00A1, DeclaredMask(0xA), 1);
        assert_param_shift(0x1100_00AA, DeclaredMask(0xA), 0);
        assert_param_shift(0xB100_00AA, DeclaredMask(0xB), 7);
        assert_param_shift(0x0100_00AA, DeclaredMask(0xB), 0);
    }
}
