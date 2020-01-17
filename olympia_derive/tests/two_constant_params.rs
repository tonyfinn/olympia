use olympia_core::disasm::Disassemble;
use olympia_core::instructions::{
    ConstantParam, ExtensionType, Instruction, InstructionOpcode, ParamDefinition, ParamPosition,
    ParamType,
};
use olympia_derive::OlympiaInstruction;

use olympia_core::registers::WordRegister;
use olympia_core::registers::WordRegister::*;

#[derive(Debug, PartialEq, Eq, OlympiaInstruction)]
#[olympia(opcode = 0x1111_1001, label = "LD")]
struct LoadStackPointer {
    #[olympia(dest, constant(SP))]
    dest: WordRegister,
    #[olympia(src, constant(WordRegister::HL))]
    src: WordRegister,
}

#[test]
fn constant_args_definition() {
    let definition = LoadStackPointer::definition();
    assert_eq!(definition.label, "LD");
    assert_eq!(definition.opcodes, &[0xf9]);
    assert_eq!(definition.extension_type, ExtensionType::None);

    assert_eq!(
        definition.params,
        &[
            ParamDefinition {
                pos: ParamPosition::Dest,
                param_type: ParamType::Constant(ConstantParam::WordRegister(WordRegister::SP)),
            },
            ParamDefinition {
                pos: ParamPosition::Src,
                param_type: ParamType::Constant(ConstantParam::WordRegister(WordRegister::HL)),
            },
        ]
    );
}

#[test]
fn constant_args_expansion() {
    let opcode = LoadStackPointerOpcode::from_opcode(0xF9);

    let data = vec![];
    assert_eq!(
        opcode.build_instruction(&mut data.into_iter()),
        LoadStackPointer {
            dest: WordRegister::SP,
            src: WordRegister::HL,
        },
    );
}

#[test]
fn constant_args_disasm() {
    let op = LoadStackPointer {
        dest: WordRegister::SP,
        src: WordRegister::HL,
    };

    assert_eq!(op.disassemble(), "LD SP, HL");
}
