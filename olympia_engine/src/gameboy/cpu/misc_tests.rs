use super::testutils::*;
use super::*;
use crate::gameboy::StepResult;

use registers::ByteRegister as br;

#[test]
fn test_nop() -> StepResult<()> {
    let gb = run_program(
        1,
        &[
            0x00, // NOP - 4 clocks
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 4);

    Ok(())
}

#[test]
fn test_set_carry_flag() -> StepResult<()> {
    // 3 step program used to make test independent of initial
    // flag state
    let gb = run_program(
        3,
        &[
            0x37, // SCF - 4 clocks
            0x3F, // CCF - 4 clocks
            0x37, // SCF - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.clocks_elapsed(), 12);
    Ok(())
}

#[test]
fn test_clear_carry_flag() -> StepResult<()> {
    // 3 step program used to make test independent of initial
    // flag state
    let gb = run_program(
        3,
        &[
            0x3F, // CCF - 4 clocks
            0x37, // SCF - 4 clocks
            0x3F, // CCF - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.clocks_elapsed(), 12);
    Ok(())
}

#[test]
fn test_rotate_right_carry() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3F, // CCF - 4 clocks
            0x3E, 0x01, // LD A, 0x01 - 8 clocks
            0x0F, // RRCA - 4 clocks
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 16);
    assert_eq!(gb.read_register_u8(br::A), 0b1000_0000);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    Ok(())
}

#[test]
fn test_rotate_right() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3F, // CCF - 4 clocks
            0x3E, 0x01, // LD A, 0x01 - 8 clocks
            0x1F, // RRA - 4 clocks
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 16);
    assert_eq!(gb.read_register_u8(br::A), 0b0000_0000);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    Ok(())
}

#[test]
fn test_rotate_right_flag_set() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x37, // SCF - 4 clocks
            0x3E, 0x02, // LD A, 0x02 - 8 clocks
            0x1F, // RRA - 4 clocks
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 16);
    assert_eq!(gb.read_register_u8(br::A), 0b1000_0001);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    Ok(())
}

#[test]
fn test_rotate_left_carry() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3F, // CCF - 4 clocks
            0x3E, 0x80, // LD A, 0x80 - 8 clocks
            0x07, // RLCA - 4 clocks
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 16);
    assert_eq!(gb.read_register_u8(br::A), 0b0000_0001);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    Ok(())
}

#[test]
fn test_rotate_left() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3F, // CCF - 4 clocks
            0x3E, 0x80, // LD A, 0x80 - 8 clocks
            0x17, // RLA - 4 clocks
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 16);
    assert_eq!(gb.read_register_u8(br::A), 0b0000_0000);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    Ok(())
}

#[test]
fn test_rotate_left_flag_set() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x37, // SCF - 4 clocks
            0x3E, 0x02, // LD A, 0x02 - 8 clocks
            0x17, // RLA - 4 clocks
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 16);
    assert_eq!(gb.read_register_u8(br::A), 0b0000_0101);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    Ok(())
}
