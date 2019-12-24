use olympia_core::instructions::{
    ByteRegisterTarget, ExtensionType, InnerParam, Instruction, OpcodePosition, ParamDefinition,
    ParamPosition, ParamType,
};
use olympia_derive::OlympiaInstruction;

#[test]
#[allow(dead_code)]
fn one_arg_extended_definition() {
    #[derive(OlympiaInstruction)]
    #[olympia(opcode = 0x0011_0AAA, label = "SWAP", extended)]
    struct Swap {
        #[olympia(single, mask = 0xA)]
        reg: ByteRegisterTarget,
    };

    let definition = Swap::definition();
    assert_eq!(definition.label, "SWAP");
    assert_eq!(
        definition.opcodes,
        &[0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37]
    );
    assert_eq!(definition.extension_type, ExtensionType::Extended);

    assert_eq!(
        definition.params,
        &[ParamDefinition {
            pos: ParamPosition::Single,
            param_type: ParamType::Inner {
                pos: OpcodePosition {
                    mask: 0b0000_0111,
                    shift: 0,
                },
                ty: InnerParam::ByteRegisterTarget
            },
        }]
    );
}
