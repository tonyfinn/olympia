extern crate proc_macro;

mod errors;
mod params;

use crate::errors::DeriveError;

use olympia_core::derive::{ExtensionType, ParamPosition};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::iter::Iterator;
use std::vec::Vec;
use syn::spanned::Spanned;
use syn::{parse_macro_input, DeriveInput};

#[derive(Debug)]
struct InstructionBuilder {
    opcode_mask: Option<u32>,
    base_opcode: Option<u8>,
    visibility: Option<syn::Visibility>,
    excluded_opcodes: Vec<u8>,
    label: Option<String>,
    extension_type: ExtensionType,
    generate_disasm: bool,
    params: Vec<params::ParamBuilder>,
    span: Option<proc_macro2::Span>,
}

impl Default for InstructionBuilder {
    fn default() -> Self {
        InstructionBuilder {
            opcode_mask: None,
            base_opcode: None,
            excluded_opcodes: Vec::new(),
            label: None,
            extension_type: ExtensionType::None,
            visibility: None,
            generate_disasm: true,
            params: Vec::new(),
            span: None,
        }
    }
}

impl InstructionBuilder {
    fn build(self) -> errors::DeriveResult<ParsedInstruction> {
        let opcode_mask = self
            .opcode_mask
            .ok_or(errors::DeriveErrorEnum::Instruction(
                errors::InstructionError::MissingOpcodeMask,
            ))?;
        let label: String = self.label.ok_or(errors::DeriveErrorEnum::Instruction(
            errors::InstructionError::MissingLabel,
        ))?;
        let mut params = Vec::new();
        for param in self.params.iter() {
            params.push(param.build(opcode_mask)?)
        }
        Ok(ParsedInstruction {
            excluded_opcodes: self.excluded_opcodes.clone(),
            visibility: self.visibility.unwrap(),
            base_opcode: self.base_opcode.unwrap(),
            extension_type: self.extension_type,
            opcode_mask,
            generate_disasm: self.generate_disasm,
            label,
            params,
        })
    }
}

struct ParsedInstruction {
    opcode_mask: u32,
    base_opcode: u8,
    excluded_opcodes: Vec<u8>,
    visibility: syn::Visibility,
    label: String,
    generate_disasm: bool,
    extension_type: ExtensionType,
    params: Vec<params::ParsedParam>,
}

fn base_opcode(mask: u32) -> u8 {
    let mut base = 0;
    for i in 0..8 {
        let shift = i * 4;
        let hex_digit = (mask >> shift) & 0xF;
        if hex_digit == 1 {
            base |= 1 << i;
        }
    }
    base
}

fn build_opcodes(mask: u32, excluded_opcodes: &[u8]) -> Vec<u8> {
    let mut required_mask: u8 = 0;
    let mut test_value: u8 = 0;
    for i in 0..8 {
        let shift = i * 4;
        let hex_digit = (mask >> shift) & 0xF;
        if hex_digit == 0 || hex_digit == 1 {
            required_mask |= 1 << i;
            test_value |= (hex_digit as u8) << i;
        }
    }
    let mut codes = Vec::new();
    for i in 0..=0xFF {
        if ((i & required_mask) == (test_value & required_mask)) && !excluded_opcodes.contains(&i) {
            codes.push(i);
        }
    }
    codes
}

fn parse_instruction_name_value(
    ib: &mut InstructionBuilder,
    attribute_nv: &syn::MetaNameValue,
) -> errors::InstructionResult<()> {
    let path = &attribute_nv.path;
    if path.is_ident("label") {
        let label = match &attribute_nv.lit {
            syn::Lit::Str(litstr) => litstr.value(),
            _ => panic!("Labels must be strings"),
        };
        ib.label = Some(label);
        Ok(())
    } else if path.is_ident("opcode") {
        let opcode_mask = match &attribute_nv.lit {
            syn::Lit::Int(num) => num.base10_parse().expect("Must be able to parse opcode"),
            other => return Err(errors::InstructionError::InvalidOpcodeMask(other.clone())),
        };
        ib.opcode_mask = Some(opcode_mask);
        Ok(())
    } else {
        Err(errors::InstructionError::UnknownField(path.clone()))
    }
}

fn parse_instruction_path(
    ib: &mut InstructionBuilder,
    path: &syn::Path,
) -> errors::InstructionResult<()> {
    if path.is_ident("extended") {
        ib.extension_type = ExtensionType::Extended;
        Ok(())
    } else if path.is_ident("nodisasm") {
        ib.generate_disasm = false;
        Ok(())
    } else {
        Err(errors::InstructionError::UnknownField(path.clone()))
    }
}

fn parse_instruction_meta_list(
    ib: &mut InstructionBuilder,
    ml: &syn::MetaList,
) -> errors::InstructionResult<()> {
    if ml.path.is_ident("excluded") {
        let mut excluded: Vec<u8> = Vec::new();
        for nested in ml.nested.iter() {
            match nested {
                syn::NestedMeta::Lit(syn::Lit::Int(li)) => {
                    excluded.push(li.base10_parse()?);
                }
                syn::NestedMeta::Lit(lit) => {
                    return Err(errors::InstructionError::UnexpectedLiteral(lit.clone()))
                }
                syn::NestedMeta::Meta(meta) => {
                    return Err(errors::InstructionError::InvalidExclude(meta.clone()))
                }
            }
        }
        ib.excluded_opcodes = excluded;
        Ok(())
    } else {
        Err(errors::InstructionError::UnknownField(ml.path.clone()))
    }
}

fn build_definition(instr: &ParsedInstruction) -> errors::InstructionResult<TokenStream> {
    let opcodes = build_opcodes(instr.opcode_mask, &instr.excluded_opcodes);
    let label = &instr.label;
    let params: Vec<TokenStream> = instr
        .params
        .iter()
        .map(|p| p.quote_definition().unwrap())
        .collect();
    let extension_type = syn::Ident::new(
        &format!("{:?}", instr.extension_type),
        proc_macro2::Span::call_site(),
    );
    Ok(quote! {
        ::olympia_core::derive::InstructionDefinition {
            label: #label,
            opcodes: &[#(#opcodes),*],
            extension_type: ::olympia_core::derive::ExtensionType::#extension_type,
            params: &[#(#params),*],
        }
    })
}

fn parse_instruction(
    ib: &mut InstructionBuilder,
    attribute: &syn::Attribute,
) -> errors::InstructionResult<()> {
    let meta = attribute
        .parse_meta()
        .or_else(|err| Err(errors::InstructionError::SynError(err)))?;
    match meta {
        syn::Meta::List(list) => {
            let mut errors = Vec::new();
            for attribute in list.nested.iter() {
                let result = match attribute {
                    syn::NestedMeta::Meta(syn::Meta::NameValue(nv)) => {
                        parse_instruction_name_value(ib, nv)
                    }
                    syn::NestedMeta::Meta(syn::Meta::Path(path)) => {
                        parse_instruction_path(ib, path)
                    }
                    syn::NestedMeta::Meta(syn::Meta::List(ml)) => {
                        parse_instruction_meta_list(ib, ml)
                    }
                    syn::NestedMeta::Lit(lit) => {
                        Err(errors::InstructionError::UnexpectedLiteral(lit.clone()))
                    }
                };

                if let Err(result_err) = result {
                    errors.push(result_err);
                }
            }
            errors::InstructionError::ok_or_group_errors(errors)
        }
        _ => Err(errors::InstructionError::MissingPrereq),
    }
}

fn build_into_instruction(instruction_name: &syn::Ident, instr: &ParsedInstruction) -> TokenStream {
    let param_names = params::get_param_names(&instr.params);
    let opcode_extractor = params::build_inner_param_extractor(&instr.params);
    let constant_param_extractors = params::build_into_instruction_constant_params(&instr.params);
    let appended_param_extractors = params::build_into_instruction_appended_params(&instr.params);
    quote! {
        fn build_instruction(&self, iter: &mut dyn Iterator<Item=u8>) -> Self::FullInstruction {
            #opcode_extractor
            #(#appended_param_extractors)*
            #(#constant_param_extractors)*
            #instruction_name {
                #(#param_names: #param_names.clone()),*
            }
        }
    }
}

fn build_disassemble(
    name: &syn::Ident,
    instr: &ParsedInstruction,
) -> errors::InstructionResult<TokenStream> {
    let params = params::params_by_position(&instr.params);
    let label = &instr.label;
    let src = params.get(&ParamPosition::Src);
    let addsrc = params.get(&ParamPosition::AddSrc);
    let dest = params.get(&ParamPosition::Dest);
    let disassemble: syn::Path =
        syn::parse_str("::olympia_core::derive::Disassemble::disassemble")?;
    if params.is_empty() {
        Ok(quote! {
            impl ::olympia_core::derive::Disassemble for #name {
                fn disassemble(&self) -> ::alloc::string::String {
                    ::alloc::string::String::from(#label)
                }
            }
        })
    } else if params.len() == 1 {
        let param_name = &params.values().nth(0).unwrap().name;
        Ok(quote! {
            impl ::olympia_core::derive::Disassemble for #name {
                fn disassemble(&self) -> ::alloc::string::String {
                    format!("{} {}", #label, #disassemble(&self.#param_name))
                }
            }
        })
    } else if src.is_some() && dest.is_some() && addsrc.is_none() {
        let src_name = &src.unwrap().name;
        let dest_name = &dest.unwrap().name;
        Ok(quote! {
            impl ::olympia_core::derive::Disassemble for #name {
                fn disassemble(&self) -> ::alloc::string::String {
                    format!("{} {}, {}", #label, #disassemble(&self.#dest_name), #disassemble(&self.#src_name))
                }
            }
        })
    } else if src.is_some() && dest.is_some() && addsrc.is_some() {
        let src_name = &src.unwrap().name;
        let addsrc_name = &addsrc.unwrap().name;
        let dest_name = &dest.unwrap().name;
        Ok(quote! {
            impl ::olympia_core::derive::Disassemble for #name {
                fn disassemble(&self) -> ::alloc::string::String {
                    format!("{} {}, {} + {}", #label, #disassemble(&self.#dest_name), #disassemble(&self.#src_name), #disassemble(&self.#addsrc_name))
                }
            }
        })
    } else {
        Err(errors::InstructionError::InvalidFieldCombination)
    }
}

fn build_opcode_struct(
    instruction_name: &syn::Ident,
    definition_ident: &syn::Ident,
    instr: &ParsedInstruction,
) -> TokenStream {
    let name = format_ident!("{}Opcode", instruction_name);
    let visibility = &instr.visibility;

    let constructor_statements = params::build_opcode_constructor_statements(&instr.params);
    let fields = params::get_inner_fields(&instr.params);

    let field_declarations: Vec<TokenStream> = fields
        .iter()
        .map(|(ident, ty)| {
            quote! {
                #ident: #ty
            }
        })
        .collect();

    let field_names: Vec<&syn::Ident> = fields.iter().map(|(ident, _)| ident).collect();

    let into_instruction = build_into_instruction(instruction_name, &instr);

    quote! {
        #visibility struct #name {
            #(#field_declarations),*
        }

        impl ::olympia_core::derive::InstructionOpcode for #name {
            type FullInstruction = #instruction_name;

            fn definition() -> &'static ::olympia_core::derive::InstructionDefinition {
                &#definition_ident
            }

            fn from_opcode(opcode: u8) -> #name {
                #(#constructor_statements)*
                #name {
                    #(#field_names),*
                }
            }

            #into_instruction
        }
    }
}

fn parse_fields(input: &DeriveInput) -> errors::InstructionResult<Vec<syn::Field>> {
    if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(fields),
        ..
    }) = &input.data
    {
        Ok(fields.named.iter().cloned().collect())
    } else if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Unit,
        ..
    }) = &input.data
    {
        Ok(Vec::new())
    } else {
        Err(errors::InstructionError::NotAStruct)
    }
}

fn parse_all(input: &DeriveInput) -> syn::Result<InstructionBuilder> {
    let mut instruction_builder = InstructionBuilder::default();
    let mut instr_span = input.span();
    instruction_builder.span = Some(input.span());
    instruction_builder.visibility = Some(input.vis.clone());
    let mut errors = Vec::new();
    for attribute in &input.attrs {
        if attribute.path.is_ident("olympia") {
            instruction_builder.span = Some(attribute.span());
            instr_span = attribute.span();
            let result = parse_instruction(&mut instruction_builder, &attribute);
            if let Some(err) = result.err() {
                errors.push(err.to_syn_error(attribute.span()));
            }
        }
    }
    instruction_builder.base_opcode = instruction_builder.opcode_mask.map(base_opcode);
    let field_iter = parse_fields(&input).map_err(|e| syn::Error::new(instr_span, e))?;
    let mut params = Vec::new();
    for field in field_iter {
        match params::parse_param(&field) {
            Ok(p) => params.push(p),
            Err(e) => errors.push(e.to_syn_error(field.span())),
        }
    }
    instruction_builder.params = params;
    if let Some(error) = errors::merge_syn_errors(&errors) {
        Err(error)
    } else {
        Ok(instruction_builder)
    }
}

fn olympia_instruction_inner(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let definition_ident = format_ident!("{}_DEFINITION", name.to_string().to_uppercase());
    let instruction_builder = parse_all(&input)?;
    let default_span = instruction_builder.span.unwrap();
    let parsed = instruction_builder
        .build()
        .map_err(|e| e.to_syn_error(default_span))?;
    let definition = build_definition(&parsed).map_err(|e| e.to_syn_error(default_span))?;
    let opcode_struct = build_opcode_struct(name, &definition_ident, &parsed);
    let as_bytes = params::build_as_bytes(parsed.base_opcode, &parsed.params);
    let disassemble = if parsed.generate_disasm {
        build_disassemble(name, &parsed).map_err(|e| e.to_syn_error(default_span))?
    } else {
        quote!()
    };
    Ok(quote! {
        const #definition_ident: ::olympia_core::derive::InstructionDefinition = #definition;
        impl ::olympia_core::instructions::Instruction for #name {
            fn definition() -> &'static ::olympia_core::derive::InstructionDefinition {
                &#definition_ident
            }
            #as_bytes
        }
        #disassemble
        #opcode_struct
    })
}

#[proc_macro_derive(OlympiaInstruction, attributes(olympia))]
pub fn olympia_instruction(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    olympia_instruction_inner(parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_build_opcodes() {
        fn assert_opcode_result(mask: u32, exclusions: Vec<u8>, result: Vec<u8>) {
            let opcodes = build_opcodes(mask, &exclusions);
            assert_eq!(opcodes, result);
        }
        assert_opcode_result(0x1000_00AA, vec![], vec![0x80, 0x81, 0x82, 0x83]);
        assert_opcode_result(0x1000_00AA, vec![0x82], vec![0x80, 0x81, 0x83]);
        assert_opcode_result(0xAB00_0000, vec![], vec![0x00, 0x40, 0x80, 0xC0]);
    }
}

#[cfg(test)]
mod coverage_tests {
    // These tests load and compiles the standalone test files to ensure that the expansion
    // that it performs is recognised by tarpaulin for coverage purposes.
    use super::olympia_instruction_inner;
    use std::{env, fs};

    #[test]
    fn basic_instruction_coverage() {
        let mut path = env::current_dir().unwrap();
        path.push("tests");
        path.push("basic_instruction.rs");
        let file = fs::File::open(path).unwrap();
        runtime_macros::emulate_derive_expansion_fallible(file, "OlympiaInstruction", |input| {
            olympia_instruction_inner(input).unwrap()
        })
        .unwrap();
    }

    #[test]
    fn extended_one_inner_coverage() {
        let mut path = env::current_dir().unwrap();
        path.push("tests");
        path.push("extended_one_inner.rs");
        let file = fs::File::open(path).unwrap();
        runtime_macros::emulate_derive_expansion_fallible(file, "OlympiaInstruction", |input| {
            olympia_instruction_inner(input).unwrap()
        })
        .unwrap();
    }

    #[test]
    fn inner_and_appended_coverage() {
        let mut path = env::current_dir().unwrap();
        path.push("tests");
        path.push("inner_and_appended.rs");
        let file = fs::File::open(path).unwrap();
        runtime_macros::emulate_derive_expansion_fallible(file, "OlympiaInstruction", |input| {
            olympia_instruction_inner(input).unwrap()
        })
        .unwrap();
    }

    #[test]
    fn one_appended_param_coverage() {
        let mut path = env::current_dir().unwrap();
        path.push("tests");
        path.push("one_appended_param.rs");
        let file = fs::File::open(path).unwrap();
        runtime_macros::emulate_derive_expansion_fallible(file, "OlympiaInstruction", |input| {
            olympia_instruction_inner(input).unwrap()
        })
        .unwrap();
    }

    #[test]
    fn one_inner_param_coverage() {
        let mut path = env::current_dir().unwrap();
        path.push("tests");
        path.push("one_inner_param.rs");
        let file = fs::File::open(path).unwrap();
        runtime_macros::emulate_derive_expansion_fallible(file, "OlympiaInstruction", |input| {
            olympia_instruction_inner(input).unwrap()
        })
        .unwrap();
    }

    #[test]
    fn two_constant_params_coverage() {
        let mut path = env::current_dir().unwrap();
        path.push("tests");
        path.push("two_constant_params.rs");
        let file = fs::File::open(path).unwrap();
        runtime_macros::emulate_derive_expansion_fallible(file, "OlympiaInstruction", |input| {
            olympia_instruction_inner(input).unwrap()
        })
        .unwrap();
    }

    #[test]
    fn three_params_coverage() {
        let mut path = env::current_dir().unwrap();
        path.push("tests");
        path.push("three_params.rs");
        let file = fs::File::open(path).unwrap();
        runtime_macros::emulate_derive_expansion_fallible(file, "OlympiaInstruction", |input| {
            olympia_instruction_inner(input).unwrap()
        })
        .unwrap();
    }
}
