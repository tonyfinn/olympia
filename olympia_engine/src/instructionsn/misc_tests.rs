#![allow(clippy::bool_assert_comparison)]

use crate::gameboy::{testutils::*, StepResult};

use crate::registers;
use crate::registers::ByteRegister as br;

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

#[test]
fn test_invert_a() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x3E, 0xAA, // LD A, 0xAA - 8 clocks
            0x2F, // CPL - 4 clocks
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 12);
    assert_eq!(gb.read_register_u8(br::A), !0xAA);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), true);

    Ok(())
}

fn run_add_daa(add_subtract: bool, a_val: u8, add_val: u8) -> StepResult<crate::gameboy::GameBoy> {
    let instruction = if add_subtract {
        0xD6 // SUB d8
    } else {
        0xC6 // ADD d8
    };
    let gb = run_program(
        5,
        &[
            0x37, // SCF - 4 clocks
            0x3F, // CCF - 4 clocks
            0x3E,
            a_val, // LD A, a_val - 8 clocks
            instruction,
            add_val, // <op> add_val - 8 blocks
            0x27,    // DAA - 4 clocks
        ],
    )?;
    assert_eq!(gb.clocks_elapsed(), 28);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), false);

    Ok(gb)
}

fn assert_add_daa(a: u8, b: u8, carry: bool, result: u8) {
    let gb = run_add_daa(false, a, b).unwrap();
    let binary_add = a.wrapping_add(b);
    let actual = gb.read_register_u8(br::A);

    assert_eq!(
        actual, result,
        "Expected 0x{:X} + 0x{:X} to adjust to 0x{:X}, binary add 0x{:X}, found 0x{:X}",
        a, b, result, binary_add, actual,
    );
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), carry);
}

fn assert_sub_daa(a: u8, b: u8, carry: bool, result: u8) {
    let gb = run_add_daa(true, a, b).unwrap();
    let actual = gb.read_register_u8(br::A);

    assert_eq!(
        gb.read_register_u8(br::A),
        result,
        "Expected 0x{:X} - 0x{:X} to adjust to 0x{:X}, found 0x{:X}",
        a,
        b,
        result,
        actual
    );
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), carry);
}

#[test]
fn test_daa_add() {
    // !carry, !half_carry
    assert_add_daa(0x45, 0x23, false, 0x68);
    // !carry, decimal only half carry
    assert_add_daa(0x45, 0x38, false, 0x83);
    assert_add_daa(0x46, 0x45, false, 0x91);
    assert_add_daa(0x49, 0x49, false, 0x98);
    // decimal only carry, decimal only half carry
    assert_add_daa(0x56, 0x45, true, 0x01);
    // decimal only carry, half carry
    assert_add_daa(0x89, 0x38, true, 0x27);
    // decimal only carry, no half carry
    assert_add_daa(0x90, 0x20, true, 0x10);
    // carry, no half carry
    assert_add_daa(0x80, 0x90, true, 0x70);
    // carry, decimal only half carry
    assert_add_daa(0x85, 0x95, true, 0x80);
    // out of bcd range carry
    assert_add_daa(0x89, 0x3F, false, 0xC8);
    // carry, half carry
    assert_add_daa(0x99, 0x99, true, 0x98);
}

#[test]
fn test_add_sub() {
    // carry, no half carry
    assert_sub_daa(0x56, 0x45, false, 0x11);
    // !carry, half carry
    assert_sub_daa(0x56, 0x09, false, 0x47);
    // carry, half carry
    assert_sub_daa(0x02, 0x03, true, 0x99);
    assert_sub_daa(0x02, 0x95, true, 0x07);
    assert_sub_daa(0x05, 0x92, true, 0x13);
}
