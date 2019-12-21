use super::testutils::*;
use super::*;
use crate::gameboy::StepResult;

// TODO: Dedicated test for Load::ConstantMemory

#[test]
fn test_loads() -> StepResult<()> {
    let gb = run_program(
        6,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x06, 0x25, // LD B, 0x25 - 8 clocks
            0x50, // LD D, B - 4 clocks
            0x72, // LD (HL), D - 8 clocks
            0x5E, // LD E, (HL) - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::B), 0x25);
    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::D), 0x25);
    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::E), 0x25);
    assert_eq!(gb.read_memory_u8(0x8000)?, 0x25);
    assert_eq!(gb.clocks_elapsed(), 44);

    Ok(())
}

#[test]
fn test_offset_load() -> StepResult<()> {
    let gb = run_program(
        6,
        &[
            0x3E, 0xFF, // LD A, 0xFF - 8 clocks
            0xE0, 0xA0, // LDH (0xA0), A - 12 clocks
            0x26, 0xFF, // LD H, 0xFF - 8 clocks
            0x2E, 0xA1, // LD L, 0xA1 - 8 clocks
            0x75, // LD (HL), L - 8 clocks
            0xF0, 0xA1, // LDH A, (0xA1) - 12 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0xFFA1)?, 0xA1);
    assert_eq!(gb.read_memory_u8(0xFFA0)?, 0xFF);
    assert_eq!(gb.read_register_u8(br::A), 0xA1);
    assert_eq!(gb.clocks_elapsed(), 56);

    Ok(())
}
