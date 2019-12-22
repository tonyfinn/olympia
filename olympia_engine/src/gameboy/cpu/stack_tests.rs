use super::testutils::*;
use super::*;
use crate::gameboy::StepResult;

#[test]
fn test_stack() -> StepResult<()> {
    let gb = run_program(
        7,
        &[
            0x06, 0x05, // LD B, 0x05 - 8 clocks
            0x0E, 0x08, // LD C, 0x08 - 8 clocks
            0xC5, // PUSH BC - 16 clocks
            0xC5, // PUSH BC - 16 clocks
            0xC5, // PUSH BC - 16 clocks
            0xD1, // POP DE - 12 clocks
            0xE1, // POP HL - 12 clocks
        ],
    )?;

    assert_eq!(
        gb.cpu.read_register_u16(registers::WordRegister::DE),
        0x0508
    );
    assert_eq!(
        gb.cpu.read_register_u16(registers::WordRegister::HL),
        0x0508
    );
    assert_eq!(
        gb.cpu.read_register_u16(registers::WordRegister::SP),
        0xFFFC
    );
    assert_eq!(gb.read_memory_u16(0xFFFA)?, 0x0508);
    assert_eq!(gb.read_memory_u16(0xFFFC)?, 0x0508);
    assert_eq!(gb.read_memory_u16(0xFFF8)?, 0x0508);
    assert_eq!(gb.clocks_elapsed(), 88);

    Ok(())
}

#[test]
fn test_store_stack_pointer_memory() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x31, 0xAB, 0x12, // LD SP, 0x12AB - 12 clocks
            0x08, 0x00, 0xC0, // LD (0xC000), SP - 20 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u16(0xC000)?, 0x12AB);
    assert_eq!(gb.clocks_elapsed(), 32);

    Ok(())
}

#[test]
fn test_set_stack_pointer() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x21, 0xAB, 0x12, // LD HL, 0x12AB - 12 clocks
            0xF9, // LD SP, HL - 8 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::SP), 0x12AB);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_add_stack_pointer_no_carry() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x31, 0x80, 0xFF, // LD SP, 0xFF80 - 12 clocks
            0xE8, 0x21, // ADD SP, 0x21 - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::SP), 0xFFA1);
    assert_eq!(gb.clocks_elapsed(), 28);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);

    Ok(())
}

#[test]
fn test_add_stack_pointer_carry() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x31, 0xFD, 0xFF, // LD SP, 0xFFFD - 12 clocks
            0xE8, 0x04, // ADD SP, 0x04 - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::SP), 0x1);
    assert_eq!(gb.clocks_elapsed(), 28);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);

    Ok(())
}

#[test]
fn test_sub_stack_pointer_no_carry() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x31, 0x03, 0x00, // LD SP, 0x0003 - 12 clocks
            0xE8, 0xFE, // ADD SP, -2 - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::SP), 0x01);
    assert_eq!(gb.clocks_elapsed(), 28);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);

    Ok(())
}

#[test]
fn test_sub_stack_pointer_carry() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x31, 0x03, 0x00, // LD SP, 0x0003 - 12 clocks
            0xE8, 0xFC, // ADD SP, -4 - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::SP), 0xFFFF);
    assert_eq!(gb.clocks_elapsed(), 28);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);

    Ok(())
}

#[test]
fn test_load_stack_offset_add_no_carry() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x31, 0x80, 0xFF, // LD SP, 0xFF80 - 12 clocks
            0xF8, 0x21, // LD HL, SP + 0x21 - 12 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::HL), 0xFFA1);
    assert_eq!(gb.clocks_elapsed(), 24);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);

    Ok(())
}

#[test]
fn test_load_stack_offset_add_carry() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x31, 0xFD, 0xFF, // LD SP, 0xFFFD - 12 clocks
            0xF8, 0x04, // LD HL, SP + 0x04 - 12 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::HL), 0x0001);
    assert_eq!(gb.clocks_elapsed(), 24);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);

    Ok(())
}

#[test]
fn test_load_stack_offset_sub_no_carry() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x31, 0x04, 0x00, // LD SP, 0x0004 - 12 clocks
            0xF8, 0xFD, // LD HL, SP - 3 - 12 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::HL), 0x0001);
    assert_eq!(gb.clocks_elapsed(), 24);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);

    Ok(())
}

#[test]
fn test_load_stack_offset_sub_carry() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x31, 0x04, 0x00, // LD SP, 0x0004 - 12 clocks
            0xF8, 0xFB, // LD HL, SP - 5 - 12 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::HL), 0xFFFF);
    assert_eq!(gb.clocks_elapsed(), 24);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);

    Ok(())
}
