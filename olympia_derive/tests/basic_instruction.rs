use olympia_core::instructions::{ExtensionType, Instruction};
use olympia_derive::OlympiaInstruction;

#[test]
fn simple_opcode() {
    #[derive(OlympiaInstruction)]
    #[olympia(opcode = 0x1100_1001, label = "RET")]
    struct ReturnIf;

    let definition = ReturnIf::definition();
    assert_eq!(definition.label, "RET");
    assert_eq!(definition.opcodes, &[0xC9]);
    assert_eq!(definition.extension_type, ExtensionType::None);
    assert_eq!(definition.params, &[]);
}
