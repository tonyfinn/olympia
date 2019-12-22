use super::testutils::*;
use super::*;
use crate::gameboy::StepResult;
use registers::ByteRegister as br;
use registers::Flag as f;

#[test]
fn test_set_bit() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0xCB, 0xFD, // SET 7, L - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::L), 0x80);
    assert_eq!(gb.clocks_elapsed(), 16);

    Ok(())
}

#[test]
fn test_set_memory_bit() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0xCB, 0xD6, // SET 2, (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0x4);
    assert_eq!(gb.clocks_elapsed(), 32);

    Ok(())
}

#[test]
fn reset_memory_bit() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x36, 0xFF, // LD (HL), 0xFF - 12 clocks
            0xCB, 0x96, // SET 2, (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0xFB);
    assert_eq!(gb.clocks_elapsed(), 44);

    Ok(())
}

#[test]
fn test_reset_bit() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x2E, 0xFF, // LD L, 0xFF - 8 clocks
            0xCB, 0x85, // RES 0, L - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::L), 0xFE);
    assert_eq!(gb.clocks_elapsed(), 16);

    Ok(())
}

#[test]
fn test_test_bit_set() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x1E, 0x08, // LD E, 0x08 - 8 clocks
            0xCB, 0x5B, // BIT 3, E - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 16);

    Ok(())
}

#[test]
fn test_test_bit_not_set() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x1E, 0x08, // LD E, 0x08 - 8 clocks
            0xCB, 0x63, // BIT 4, E - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_flag(f::Zero), true);
    assert_eq!(gb.clocks_elapsed(), 16);

    Ok(())
}

#[test]
fn test_test_memory_bit_set() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x36, 0x08, // LD (HL), 0x08 - 8 clocks
            0xCB, 0x5E, // BIT 3, (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 40);

    Ok(())
}

#[test]
fn test_test_memory_bit_not_set() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x36, 0x08, // LD (HL), 0x08 - 8 clocks
            0xCB, 0x66, // BIT 4, (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_flag(f::Zero), true);
    assert_eq!(gb.clocks_elapsed(), 40);

    Ok(())
}

#[test]
fn test_rotate_reg_left_carry() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3F, // CCF - 4 clocks
            0x1E, 0x88, // LD E, 0x88 - 8 clocks
            0xCB, 0x03, // RLC E - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::E), 0x11);
    assert_eq!(gb.cpu.read_flag(f::Carry), true);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    let gb = run_program(
        3,
        &[
            0x37, // SCF - 4 clocks
            0x1E, 0x48, // LD E, 0x48 - 8 clocks
            0xCB, 0x03, // RLC E - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::E), 0x90);
    assert_eq!(gb.cpu.read_flag(f::Carry), false);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_rotate_reg_left() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3F, // CCF - 4 clocks
            0x1E, 0x88, // LD E, 0x88 - 8 clocks
            0xCB, 0x13, // RL E - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::E), 0x10);
    assert_eq!(gb.cpu.read_flag(f::Carry), true);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    let gb = run_program(
        3,
        &[
            0x37, // SCF - 4 clocks
            0x1E, 0x48, // LD E, 0x48 - 8 clocks
            0xCB, 0x13, // RL E - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::E), 0x91);
    assert_eq!(gb.cpu.read_flag(f::Carry), false);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_rotate_reg_left_zero() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3F, // CCF - 4 clocks
            0x1E, 0x80, // LD E, 0x80 - 8 clocks
            0xCB, 0x13, // RL E - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::E), 0x00);
    assert_eq!(gb.cpu.read_flag(f::Carry), true);
    assert_eq!(gb.cpu.read_flag(f::Zero), true);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_rotate_reg_right_carry() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3F, // CCF - 4 clocks
            0x1E, 0x89, // LD E, 0x88 - 8 clocks
            0xCB, 0x0B, // RRC E - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::E), 0xC4);
    assert_eq!(gb.cpu.read_flag(f::Carry), true);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    let gb = run_program(
        3,
        &[
            0x37, // SCF - 4 clocks
            0x1E, 0x48, // LD E, 0x48 - 8 clocks
            0xCB, 0x0B, // RRC E - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::E), 0x24);
    assert_eq!(gb.cpu.read_flag(f::Carry), false);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_rotate_reg_right() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3F, // CCF - 4 clocks
            0x1E, 0x89, // LD E, 0x89 - 8 clocks
            0xCB, 0x1B, // RR E - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::E), 0x44);
    assert_eq!(gb.cpu.read_flag(f::Carry), true);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    let gb = run_program(
        3,
        &[
            0x37, // SCF - 4 clocks
            0x1E, 0x48, // LD E, 0x48 - 8 clocks
            0xCB, 0x1B, // RR E - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::E), 0xA4);
    assert_eq!(gb.cpu.read_flag(f::Carry), false);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_rotate_reg_right_zero() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3F, // CCF - 4 clocks
            0x1E, 0x01, // LD E, 0x89 - 8 clocks
            0xCB, 0x1B, // RR E - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::E), 0x00);
    assert_eq!(gb.cpu.read_flag(f::Carry), true);
    assert_eq!(gb.cpu.read_flag(f::Zero), true);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_rotate_mem_left_carry() -> StepResult<()> {
    let gb = run_program(
        5,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x3F, // CCF - 4 clocks
            0x36, 0x88, // LD (HL), 0x88 - 12 clocks
            0xCB, 0x06, // RLC (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0x11);
    assert_eq!(gb.cpu.read_flag(f::Carry), true);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 48);

    let gb = run_program(
        5,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x37, // SCF - 4 clocks
            0x36, 0x48, // LD (HL), 0x48 - 12 clocks
            0xCB, 0x06, // RLC (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0x90);
    assert_eq!(gb.cpu.read_flag(f::Carry), false);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 48);

    Ok(())
}

#[test]
fn test_rotate_mem_left() -> StepResult<()> {
    let gb = run_program(
        5,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x3F, // CCF - 4 clocks
            0x36, 0x88, // LD (HL), 0x88 - 12 clocks
            0xCB, 0x16, // RL (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0x10);
    assert_eq!(gb.cpu.read_flag(f::Carry), true);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 48);

    let gb = run_program(
        5,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x37, // SCF - 4 clocks
            0x36, 0x48, // LD (HL), 0x48 - 12 clocks
            0xCB, 0x16, // RL (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0x91);
    assert_eq!(gb.cpu.read_flag(f::Carry), false);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 48);

    Ok(())
}

#[test]
fn test_rotate_mem_right_carry() -> StepResult<()> {
    let gb = run_program(
        5,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x3F, // CCF - 4 clocks
            0x36, 0x89, // LD (HL), 0x88 - 12 clocks
            0xCB, 0x0E, // RRC (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0xC4);
    assert_eq!(gb.cpu.read_flag(f::Carry), true);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 48);

    let gb = run_program(
        5,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x37, // SCF - 4 clocks
            0x36, 0x48, // LD (HL), 0x48 - 12 clocks
            0xCB, 0x0e, // RRC (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0x24);
    assert_eq!(gb.cpu.read_flag(f::Carry), false);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 48);

    Ok(())
}

#[test]
fn test_rotate_mem_right() -> StepResult<()> {
    let gb = run_program(
        5,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x3F, // CCF - 4 clocks
            0x36, 0x89, // LD (HL), 0x89 - 12 clocks
            0xCB, 0x1E, // RR (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0x44);
    assert_eq!(gb.cpu.read_flag(f::Carry), true);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 48);

    let gb = run_program(
        5,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x37, // SCF - 4 clocks
            0x36, 0x48, // LD (HL), 0x48 - 8 clocks
            0xCB, 0x1E, // RR (HL) - 8 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0xA4);
    assert_eq!(gb.cpu.read_flag(f::Carry), false);
    assert_eq!(gb.cpu.read_flag(f::Zero), false);
    assert_eq!(gb.clocks_elapsed(), 48);

    Ok(())
}

#[test]
fn test_reg_shift_low_right() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x06, 0xF0, // LD B, 0xF0 - 8 clocks
            0xCB, 0x38, // SRL B - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::B), 0x78);
    assert_eq!(gb.cpu.read_flag(f::Carry), false);
    assert_eq!(gb.clocks_elapsed(), 16);

    let gb = run_program(
        2,
        &[
            0x06, 0xF1, // LD B, 0xF1 - 8 clocks
            0xCB, 0x38, // SRL B - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::B), 0x78);
    assert_eq!(gb.cpu.read_flag(f::Carry), true);
    assert_eq!(gb.clocks_elapsed(), 16);

    Ok(())
}

#[test]
fn test_reg_shift_low_left() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x06, 0x71, // LD B, 0x71 - 8 clocks
            0xCB, 0x20, // SLA B - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::B), 0xE2);
    assert_eq!(gb.cpu.read_flag(f::Carry), false);
    assert_eq!(gb.clocks_elapsed(), 16);

    let gb = run_program(
        2,
        &[
            0x06, 0xF1, // LD B, 0xF1 - 8 clocks
            0xCB, 0x20, // SLA B - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::B), 0xE2);
    assert_eq!(gb.cpu.read_flag(f::Carry), true);
    assert_eq!(gb.clocks_elapsed(), 16);

    Ok(())
}
#[test]
fn test_mem_shift_low_right() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x36, 0xF0, // LD (HL), 0xF0 - 12 clocks
            0xCB, 0x3E, // SRL (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0x78);
    assert_eq!(gb.cpu.read_flag(f::Carry), false);
    assert_eq!(gb.clocks_elapsed(), 44);

    let gb = run_program(
        4,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x36, 0xF1, // LD (HL), 0xF1 - 12 clocks
            0xCB, 0x3E, // SRL (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0x78);
    assert_eq!(gb.cpu.read_flag(f::Carry), true);
    assert_eq!(gb.clocks_elapsed(), 44);

    Ok(())
}

#[test]
fn test_mem_shift_low_left() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x36, 0x71, // LD (HL), 0x71 - 8 clocks
            0xCB, 0x26, // SLA B - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0xE2);
    assert_eq!(gb.cpu.read_flag(f::Carry), false);
    assert_eq!(gb.clocks_elapsed(), 44);

    let gb = run_program(
        4,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x36, 0xF1, // LD (HL), 0xF1 - 8 clocks
            0xCB, 0x26, // SLA (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0xE2);
    assert_eq!(gb.cpu.read_flag(f::Carry), true);
    assert_eq!(gb.clocks_elapsed(), 44);

    Ok(())
}

#[test]
fn test_reg_swap() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x16, 0xFA, // LD D, 0xFA - 8 clocks
            0xCB, 0x32, // SWAP D - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::D), 0xAF);
    assert_eq!(gb.clocks_elapsed(), 16);

    Ok(())
}

#[test]
fn test_mem_swap() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x36, 0xFA, // LD (HL), 0xFA - 12 clocks
            0xCB, 0x36, // SWAP (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0xAF);
    assert_eq!(gb.clocks_elapsed(), 44);

    Ok(())
}

#[test]
fn test_reg_shift_extend_right() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x06, 0x80, // LB B, 0x80 - 8 clocks
            0xCB, 0x28, // SRA B - 8 clocks
            0xCB, 0x28, // SRA B - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(br::B), 0xE0);
    assert_eq!(gb.clocks_elapsed(), 24);

    Ok(())
}

#[test]
fn test_mem_shift_extend_right() -> StepResult<()> {
    let gb = run_program(
        5,
        &[
            0x26, 0x80, // LD H, 0x80 - 8 clocks
            0x2E, 0x00, // LD L, 0x80 - 8 clocks
            0x36, 0x80, // LD (HL), 0x80, 12 clocks
            0xCB, 0x2E, // SRA (HL) - 16 clocks
            0xCB, 0x2E, // SRA (HL) - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u8(0x8000)?, 0xE0);
    assert_eq!(gb.clocks_elapsed(), 60);

    Ok(())
}
