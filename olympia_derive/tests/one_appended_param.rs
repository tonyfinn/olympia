use olympia_core::address::{AddressOffset, LiteralAddress};
use olympia_core::disasm::Disassemble;
use olympia_core::instructions::{
    AppendedParam, ExtensionType, Instruction, InstructionOpcode, ParamDefinition, ParamPosition,
    ParamType, SerializableInstruction,
};
use olympia_derive::OlympiaInstruction;

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x1100_0011, label = "JP")]
struct Jump {
    #[olympia(single)]
    dest: LiteralAddress,
}

#[derive(Debug, PartialEq, Eq, OlympiaInstruction)]
#[olympia(opcode = 0x0001_1000, label = "JR")]
struct JumpRelative {
    #[olympia(single)]
    addr: AddressOffset,
}

#[test]
fn appended_definition() {
    let definition = JumpRelative::definition();
    assert_eq!(definition.label, "JR");
    assert_eq!(definition.opcodes, &[0x18]);
    assert_eq!(definition.extension_type, ExtensionType::None);

    assert_eq!(
        definition.params,
        &[ParamDefinition {
            pos: ParamPosition::Single,
            param_type: ParamType::Appended(AppendedParam::AddressOffset),
        }]
    );
}

#[test]
fn appended_parsing() {
    let data = vec![0xFEu8];
    let opcode = JumpRelativeOpcode::from_opcode(0x18);
    assert_eq!(
        opcode.build_instruction(&mut data.into_iter()),
        JumpRelative {
            addr: AddressOffset(-2)
        }
    )
}

#[test]
fn appended_as_bytes() {
    let instruction = JumpRelative {
        addr: AddressOffset(-3),
    };
    assert_eq!(instruction.as_bytes(), vec![0x18, 0xFD]);
}

#[test]
fn appended_16_as_bytes() {
    let instruction = Jump {
        dest: LiteralAddress(0x12FE),
    };
    assert_eq!(instruction.as_bytes(), vec![0xC3, 0xFE, 0x12]);
}

#[test]
fn appended_16_disasm() {
    let instruction = Jump {
        dest: LiteralAddress(0x12FE),
    };
    assert_eq!(instruction.disassemble(), "JP $12FEh");
}
