use olympia_core::instructions::{
    ExtensionType, InnerParam, Instruction, InstructionOpcode, OpcodePosition, ParamDefinition,
    ParamPosition, ParamType,
};
use olympia_core::registers::AccRegister;
use olympia_derive::OlympiaInstruction;

#[test]
#[allow(dead_code)]
fn one_arg() {
    #[derive(OlympiaInstruction)]
    #[olympia(opcode = 0x11AA_0101, label = "PUSH")]
    struct Push {
        #[olympia(single, mask = 0xA)]
        reg: AccRegister,
    };

    let definition = Push::definition();
    assert_eq!(definition.label, "PUSH");
    assert_eq!(definition.opcodes, &[0xC5, 0xD5, 0xE5, 0xF5]);
    assert_eq!(definition.extension_type, ExtensionType::None);

    assert_eq!(
        definition.params,
        &[ParamDefinition {
            pos: ParamPosition::Single,
            param_type: ParamType::Inner {
                pos: OpcodePosition {
                    mask: 0b0011_0000,
                    shift: 4,
                },
                ty: InnerParam::AccRegister
            },
        }]
    );
}

#[test]
fn one_arg_opcode() {
    #[derive(OlympiaInstruction)]
    #[olympia(opcode = 0x11AA_0101, label = "PUSH")]
    #[allow(dead_code)]
    struct Push {
        #[olympia(single, mask = 0xA)]
        reg: AccRegister,
    };

    let extracted = PushOpcode::from_opcode(0b1101_0101);

    assert_eq!(extracted.reg, AccRegister::DE);
}
