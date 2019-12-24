use olympia_core::instructions::{
    AppendedParam, ByteRegisterTarget, ExtensionType, InnerParam, Instruction, InstructionOpcode,
    OpcodePosition, ParamDefinition, ParamPosition, ParamType,
};
use olympia_derive::OlympiaInstruction;

#[derive(Debug, PartialEq, Eq, OlympiaInstruction)]
#[olympia(opcode = 0x00AA_A110, label = "LD")]
struct LoadLiteral {
    #[olympia(dest, mask = 0xA)]
    dest: ByteRegisterTarget,
    #[olympia(src)]
    src: u8,
}

#[test]
fn mixed_inner_appended_definition() {
    let definition = LoadLiteral::definition();
    assert_eq!(definition.label, "LD");
    assert_eq!(
        definition.opcodes,
        &[0x06, 0x0E, 0x16, 0x1E, 0x26, 0x2E, 0x36, 0x3E]
    );
    assert_eq!(definition.extension_type, ExtensionType::None);

    assert_eq!(
        definition.params,
        &[
            ParamDefinition {
                pos: ParamPosition::Dest,
                param_type: ParamType::Inner {
                    pos: OpcodePosition {
                        mask: 0b0011_1000,
                        shift: 3,
                    },
                    ty: InnerParam::ByteRegisterTarget,
                }
            },
            ParamDefinition {
                pos: ParamPosition::Src,
                param_type: ParamType::Appended(AppendedParam::Literal8),
            },
        ]
    );
}

#[test]
fn mixed_inner_appended_expansion() {
    let opcode = LoadLiteralOpcode::from_opcode(0x16);

    assert_eq!(opcode.dest, ByteRegisterTarget::D);

    let data = vec![0x12, 0x34];
    let instruction = opcode.into_instruction(&mut data.into_iter());

    assert_eq!(
        instruction,
        LoadLiteral {
            dest: ByteRegisterTarget::D,
            src: 0x12
        }
    )
}
