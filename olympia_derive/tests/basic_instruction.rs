use olympia_core::instructions::{ExtensionType, Instruction};
use olympia_derive::OlympiaInstruction;

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1100_1001, label = "RET")]
struct Return;

#[test]
fn simple_opcode() {
    let definition = Return::definition();
    assert_eq!(definition.label, "RET");
    assert_eq!(definition.opcodes, &[0xC9]);
    assert_eq!(definition.extension_type, ExtensionType::None);
    assert_eq!(definition.params, &[]);
}

#[test]
fn simple_opcode_bytes() {
    let instruction = Return {};
    assert_eq!(instruction.as_bytes(), vec![0xC9]);
}
