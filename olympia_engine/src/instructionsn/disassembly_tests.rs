use crate::instructionsn::{RuntimeDecoder};

pub use crate::disasm::Disassemble;

fn assert_dissembly(bytes: &[u8], result: &str) {
    let runtime_decoder = RuntimeDecoder::new();
    let val = bytes[0];
    let iter = bytes.iter().skip(1);
    let decoded = runtime_decoder.decode_from_iter(val, &mut iter.copied());
    assert_eq!(decoded.unwrap().disassemble(), result);
}

#[test]
fn test_basic_instructions() {
    assert_dissembly(&[0], "NOP");
    assert_dissembly(&[0x10], "STOP");
    assert_dissembly(&[0x76], "HALT");
    assert_dissembly(&[0x27], "DAA");
    assert_dissembly(&[0x2F], "CPL");
    assert_dissembly(&[0x37], "SCF");
    assert_dissembly(&[0x3F], "CCF");
    assert_dissembly(&[0xFB], "EI");
    assert_dissembly(&[0xF3], "DI");
}

#[test]
fn test_basic_al() {
    assert_dissembly(&[0x86], "ADD (HL)");
    assert_dissembly(&[0xC6, 0x20], "ADD 20h");
    assert_dissembly(&[0x34], "INC (HL)");
    assert_dissembly(&[0x35], "DEC (HL)");
}

#[test]
fn test_rotate_a() {
    assert_dissembly(&[0x07], "RLCA");
    assert_dissembly(&[0x0F], "RRCA");
    assert_dissembly(&[0x17], "RLA");
    assert_dissembly(&[0x1F], "RRA");
}

#[test]
fn test_disassemble_stack() {
    assert_dissembly(&[0xC5], "PUSH BC");
    assert_dissembly(&[0xE1], "POP HL");
    assert_dissembly(&[0xE8, 0x12], "ADD SP, 12h");
    assert_dissembly(&[0xF8, 0x34], "LD HL, SP + 34h");
    assert_dissembly(&[0xF9], "LD SP, HL");
    assert_dissembly(&[0x08, 0x34, 0x26], "LD $2634h, SP");
}

#[test]
fn test_disassemble_register_al_byte_op() {
    assert_dissembly(&[0x87], "ADD A");
    assert_dissembly(&[0x88], "ADC B");
    assert_dissembly(&[0x91], "SUB C");
    assert_dissembly(&[0x9A], "SBC D");
    assert_dissembly(&[0xA3], "AND E");
    assert_dissembly(&[0xB6], "OR (HL)");
    assert_dissembly(&[0xAC], "XOR H");
    assert_dissembly(&[0xBD], "CP L");
}

#[test]
fn test_disassemble_register_al_inc_byte() {
    assert_dissembly(&[0x3C], "INC A");
    assert_dissembly(&[0x05], "DEC B");
}

#[test]
fn test_disassemble_register_al_word_op() {
    assert_dissembly(&[0x03], "INC BC");
    assert_dissembly(&[0x1B], "DEC DE");
    assert_dissembly(&[0x39], "ADD HL, SP");
}

#[test]
fn test_jump_uncond() {
    assert_dissembly(&[0xC9], "RET");
    assert_dissembly(&[0xD9], "RETI");
    assert_dissembly(&[0xE9], "JP HL");
}

#[test]
fn test_jump_uncond_addr() {
    assert_dissembly(&[0xC3, 0x12, 0x00], "JP $12h");
    assert_dissembly(&[0xCD, 0x24, 0x00], "CALL $24h");
    assert_dissembly(&[0xEF], "RST $28h");
    assert_dissembly(&[0x18, 0x15], "JR 15h");
}

#[test]
fn test_jump_cond() {
    assert_dissembly(&[0xD8], "RET C");
    assert_dissembly(&[0xCA, 0x12, 0x00], "JP Z, $12h");
    assert_dissembly(&[0xC4, 0x24, 0x00], "CALL NZ, $24h");
    assert_dissembly(&[0x30, 0x15], "JR NC, 15h");
}

#[test]
fn test_load_constant() {
    assert_dissembly(&[0x3E, 0x23], "LD A, 23h");
    assert_dissembly(&[0x36, 0x25], "LD (HL), 25h");
    assert_dissembly(&[0x21, 0x45, 0x23], "LD HL, 2345h");
}

#[test]
fn test_load_move() {
    assert_dissembly(&[0x7D], "LD A, L");
    assert_dissembly(&[0x5E], "LD E, (HL)");
    assert_dissembly(&[0x12], "LD (DE), A");
}

#[test]
fn test_load_indirect() {
    assert_dissembly(&[0xF2], "LD A, (C)");
    assert_dissembly(&[0xE2], "LD (C), A");
    assert_dissembly(&[0xF0, 0x23], "LD A, $FF23h");
    assert_dissembly(&[0xFA, 0x23, 0x00], "LD A, $23h");
    assert_dissembly(&[0xEA, 0x23, 0x00], "LD $23h, A");
}

#[test]
fn test_load_increment() {
    assert_dissembly(&[0x22], "LD (HL+), A");
    assert_dissembly(&[0x32], "LD (HL-), A");
    assert_dissembly(&[0x2A], "LD A, (HL+)");
    assert_dissembly(&[0x3A], "LD A, (HL-)");
}

#[test]
fn test_extended_rotate() {
    assert_dissembly(&[0xCB, 0x01], "RLC C");
    assert_dissembly(&[0xCB, 0x12], "RL D");
    assert_dissembly(&[0xCB, 0x0B], "RRC E");
    assert_dissembly(&[0xCB, 0x1C], "RR H");
}

#[test]
fn test_extended_rotate_mem() {
    assert_dissembly(&[0xCB, 0x06], "RLC (HL)");
    assert_dissembly(&[0xCB, 0x16], "RL (HL)");
    assert_dissembly(&[0xCB, 0x0E], "RRC (HL)");
    assert_dissembly(&[0xCB, 0x1E], "RR (HL)");
}

#[test]
fn test_extended_shift() {
    assert_dissembly(&[0xCB, 0x25], "SLA L");
    assert_dissembly(&[0xCB, 0x3F], "SRL A");
    assert_dissembly(&[0xCB, 0x26], "SLA (HL)");
    assert_dissembly(&[0xCB, 0x3E], "SRL (HL)");
    assert_dissembly(&[0xCB, 0x28], "SRA B");
    assert_dissembly(&[0xCB, 0x2E], "SRA (HL)");
}

#[test]
fn test_extended_swap() {
    assert_dissembly(&[0xCB, 0x35], "SWAP L");
    assert_dissembly(&[0xCB, 0x36], "SWAP (HL)");
}

#[test]
fn test_extended_bit_op() {
    assert_dissembly(&[0xCB, 0xC7], "SET 0h, A");
    assert_dissembly(&[0xCB, 0x88], "RES 1h, B");
    assert_dissembly(&[0xCB, 0x51], "BIT 2h, C");
    assert_dissembly(&[0xCB, 0xC6], "SET 0h, (HL)");
    assert_dissembly(&[0xCB, 0x8E], "RES 1h, (HL)");
    assert_dissembly(&[0xCB, 0x56], "BIT 2h, (HL)");
}