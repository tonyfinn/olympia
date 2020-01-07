use olympia_core::address::AddressOffset;
use olympia_core::instructions::{
    AppendedParam, ExtensionType, Instruction, InstructionOpcode, ParamDefinition, ParamPosition,
    ParamType,
};
use olympia_derive::OlympiaInstruction;

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
