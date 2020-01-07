use olympia_core::instructions::{
    AppendedParam, ConstantParam, ExtensionType, Instruction, InstructionOpcode, OpcodePosition, ParamDefinition,
    ParamPosition, ParamType,
};
use olympia_core::address;
use olympia_core::registers::WordRegister;
use olympia_derive::OlympiaInstruction;

#[derive(PartialEq, Eq, Debug, OlympiaInstruction)]
#[olympia(opcode = 0x1111_1000, label = "LD")]
struct LoadStackOffset {
    #[olympia(dest, constant(WordRegister::HL))]
    dest: WordRegister,
    #[olympia(src, constant(WordRegister::SP))]
    base: WordRegister,
    #[olympia(addsrc)]
    value: address::AddressOffset,
}

#[test]
fn three_arg_definition() {
    let definition = LoadStackOffset::definition();
    assert_eq!(definition.label, "LD");
    assert_eq!(definition.opcodes, &[0xF8]);
    assert_eq!(definition.extension_type, ExtensionType::None);

    assert_eq!(
        definition.params,
        &[
            ParamDefinition {
                pos: ParamPosition::Dest,
                param_type: ParamType::Constant(ConstantParam::WordRegister(WordRegister::HL)),
            },
            ParamDefinition {
                pos: ParamPosition::Src,
                param_type: ParamType::Constant(ConstantParam::WordRegister(WordRegister::SP)),
            },
            ParamDefinition {
                pos: ParamPosition::AddSrc,
                param_type: ParamType::Appended(AppendedParam::AddressOffset),
            }
        ]
    );
}

#[test]
fn three_arg_expansion() {
    let opcode = LoadStackOffsetOpcode::from_opcode(0xF8);

    assert_eq!(
        opcode.build_instruction(&mut vec![0xFE].into_iter()),
        LoadStackOffset {
            dest: WordRegister::HL,
            base: WordRegister::SP,
            value: address::AddressOffset(-2)
        }
    );
}