use crate::gameboy::{testutils::*, StepResult};

use crate::registers::{ByteRegister as br, WordRegister as wr};

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

    assert_eq!(gb.cpu.read_register_u8(br::B), 0x25);
    assert_eq!(gb.cpu.read_register_u8(br::D), 0x25);
    assert_eq!(gb.cpu.read_register_u8(br::E), 0x25);
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

#[test]
fn test_load_constant_16() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x01, 0x23, 0x45, // LD BC, 0x4523 - 12 clocks
            0x11, 0x45, 0x56, // LD DE, 0x5645 - 12 clocks
            0x21, 0x11, 0x22, // LD HL, 0x2211 - 12 clocks
            0x31, 0x45, 0x67, // LD SP, 0x6745 - 12 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(wr::BC), 0x4523);
    assert_eq!(gb.read_register_u16(wr::DE), 0x5645);
    assert_eq!(gb.read_register_u16(wr::HL), 0x2211);
    assert_eq!(gb.read_register_u16(wr::SP), 0x6745);

    assert_eq!(gb.clocks_elapsed(), 48);

    Ok(())
}

#[test]
fn load_post_increment() -> StepResult<()> {
    let gb = run_program(
        7,
        &[
            0x21, 0x05, 0xC0, // LD HL, 0xC005 - 12 clocks
            0x36, 0xAB, // LD (HL), 0xAB - 12 clocks
            0x2E, 0x03, // LD L, 0x03 - 8 clocks
            0x3E, 0x45, // LD A, 0x45 - 8 clocks
            0x22, // LD (HL+), A - 8 clocks
            0x22, // LD (HL+), A - 8 clocks
            0x2A, // LD A, (HL+) - 8 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0xc003)?, 0x45);
    assert_eq!(gb.read_memory_u8(0xc004)?, 0x45);
    assert_eq!(gb.read_memory_u8(0xC005)?, 0xAB);
    assert_eq!(gb.read_register_u8(br::A), 0xAB);
    assert_eq!(gb.read_register_u16(wr::HL), 0xC006);
    assert_eq!(gb.clocks_elapsed(), 64);

    Ok(())
}

#[test]
fn load_post_decrement() -> StepResult<()> {
    let gb = run_program(
        7,
        &[
            0x21, 0x03, 0xC0, // LD HL, 0xC005 - 12 clocks
            0x36, 0xAB, // LD (HL), 0xAB - 12 clocks
            0x2E, 0x05, // LD L, 0x03 - 8 clocks
            0x3E, 0x45, // LD A, 0x45 - 8 clocks
            0x32, // LD (HL-), A - 8 clocks
            0x32, // LD (HL-), A - 8 clocks
            0x3A, // LD A, (HL-) - 8 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0xc003)?, 0xAB);
    assert_eq!(gb.read_memory_u8(0xc004)?, 0x45);
    assert_eq!(gb.read_memory_u8(0xC005)?, 0x45);
    assert_eq!(gb.read_register_u8(br::A), 0xAB);
    assert_eq!(gb.read_register_u16(wr::HL), 0xC002);
    assert_eq!(gb.clocks_elapsed(), 64);

    Ok(())
}

#[test]
fn test_load_memory_offset_a() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x0E, 0x81, // LD C, 0x81 - 8 clocks
            0x3E, 0x45, // LD A, 0x45 - 8 clocks
            0xE2, // LD (C), A - 8 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0xFF81)?, 0x45);
    assert_eq!(gb.clocks_elapsed(), 24);
    Ok(())
}

#[test]
fn test_load_a_memory_offset() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x0E, 0x81, // LD C, 0x81 - 8 clocks
            0x21, 0x81, 0xff, // LD HL, 0xff81 - 12 clocks
            0x36, 0x45, // LD (HL), 0x45 - 12 clocks
            0xF2, // LD A, (C) - 8 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u8(br::A), 0x45);
    assert_eq!(gb.clocks_elapsed(), 40);
    Ok(())
}

#[test]
fn test_load_a_indirect() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x21, 0x02, 0xC0, // LD HL, 0xC002 - 12 clocks
            0x36, 0x78, // LD (HL), 0x78 - 12 clocks
            0xFA, 0x02, 0xC0, // LD A, (0xC002) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u8(br::A), 0x78);
    assert_eq!(gb.clocks_elapsed(), 40);
    Ok(())
}

#[test]
fn test_load_indirect_a() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x3E, 0x87, // LD A, 0x87 - 8 clocks
            0xEA, 0x12, 0xC1, // LD (0xC112), A - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0xC112)?, 0x87);
    assert_eq!(gb.clocks_elapsed(), 24);

    Ok(())
}
