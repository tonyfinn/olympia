use super::testutils::*;
use super::*;
use crate::gameboy::StepResult;

#[test]
fn test_add_no_carry() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0x05, // LD B, 0x05 - 8 clocks
            0x80, // ADD A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0xFF);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_add_half_carry() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3E, 0x0F, // LD A, 0xFA - 8 clocks
            0x06, 0x01, // LD B, 0x05 - 8 clocks
            0x80, // ADD A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x10);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_add_causes_carry_zero() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0x06, // LD B, 0x06 - 8 clocks
            0x80, // ADD A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x00);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_add_causes_carry_nonzero() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0x07, // LD B, 0x07 - 8 clocks
            0x80, // ADD A, B
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x01);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_sub_no_carry() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3E, 0x06, // LD A, 0x06 - 8 clocks
            0x06, 0x05, // LD B, 0x05 - 8 clocks
            0x90, // SUB A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x01);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_sub_half_carry() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3E, 0x10, // LD A, 0x06 - 8 clocks
            0x06, 0x01, // LD B, 0x05 - 8 clocks
            0x90, // SUB A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x0F);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_sub_causes_zero() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3E, 0x06, // LD A, 0x06 - 8 clocks
            0x06, 0x06, // LD B, 0x06 - 8 clocks
            0x90, // SUB A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x00);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_sub_causes_carry() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3E, 0x06, // LD A, 0xFA - 8 clocks
            0x06, 0x07, // LD B, 0x07 - 8 clocks
            0x90, // SUB A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0xFF);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_adc_no_carry() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0x04, // LD B, 0x04 - 8 clocks
            0x3F, // CCF - 4 clocks
            0x88, // ADC A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0xFE);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 24);

    let gb = run_program(
        4,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0x04, // LD B, 0x04 - 8 clocks
            0x37, // SCF - 4 clocks
            0x88, // ADC A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0xFF);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 24);

    Ok(())
}

#[test]
fn test_adc_causes_carry_zero() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0x06, // LD B, 0x06 - 8 clocks
            0x3F, // CCF - 4 clocks
            0x88, // ADC A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x00);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 24);

    let gb = run_program(
        4,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0x05, // LD B, 0x05 - 8 clocks
            0x37, // SCF - 4 clocks
            0x88, // ADC A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x00);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 24);

    Ok(())
}

#[test]
fn test_adc_causes_carry_nonzero() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0x07, // LD B, 0x07 - 8 clocks
            0x3F, // CCF - 4 clocks
            0x88, // ADC A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x01);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 24);

    let gb = run_program(
        4,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0x06, // LD B, 0x06 - 8 clocks
            0x37, // SCF - 4 clocks
            0x88, // ADC A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x01);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 24);

    Ok(())
}

#[test]
fn test_sbc_no_carry() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0x04, // LD B, 0x04 - 8 clocks
            0x3F, // CCF - 4 clocks
            0x98, // SBC A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0xF6);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 24);

    let gb = run_program(
        4,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0x04, // LD B, 0x04 - 8 clocks
            0x37, // SCF - 4 clocks
            0x98, // SBC A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0xF5);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 24);

    Ok(())
}

#[test]
fn test_sbc_zero() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0xFA, // LD B, 0xFA - 8 clocks
            0x3F, // CCF - 4 clocks
            0x98, // SBC A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x00);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 24);

    let gb = run_program(
        4,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0xF9, // LD B, 0xF9 - 8 clocks
            0x37, // SCF - 4 clocks
            0x98, // SBC A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x00);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 24);

    Ok(())
}

#[test]
fn test_sbc_carry() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0xFB, // LD B, 0xFB - 8 clocks
            0x3F, // CCF - 4 clocks
            0x98, // SBC A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0xFF);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 24);

    let gb = run_program(
        4,
        &[
            0x3E, 0xFA, // LD A, 0xFA - 8 clocks
            0x06, 0xFA, // LD B, 0xFA - 8 clocks
            0x37, // SCF - 4 clocks
            0x98, // SBC A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0xFF);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 24);

    Ok(())
}

#[test]
fn test_and() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3E, 0x06, // LD A, 0x06 - 8 clocks
            0x06, 0x05, // LD B, 0x05 - 8 clocks
            0xA0, // AND A, B - 4 clocks
        ],
    )?;

    assert_eq!(
        gb.cpu.read_register_u8(registers::ByteRegister::A),
        0x06 & 0x05
    );
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    let gb = run_program(
        3,
        &[
            0x3E, 0x06, // LD A, 0x06 - 8 clocks
            0x06, 0x10, // LD B, 0x05 - 8 clocks
            0xA0, // AND A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_or() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3E, 0x06, // LD A, 0x06 - 8 clocks
            0x06, 0x05, // LD B, 0x05 - 8 clocks
            0xB0, // OR A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x07);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    let gb = run_program(
        3,
        &[
            0x3E, 0x00, // LD A, 0x06 - 8 clocks
            0x06, 0x00, // LD B, 0x05 - 8 clocks
            0xB0, // OR A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x0);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_xor() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3E, 0x0C, // LD A, 0x0C - 8 clocks
            0x06, 0x0F, // LD B, 0x0F - 8 clocks
            0xA8, // XOR A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x03);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    let gb = run_program(
        3,
        &[
            0x3E, 0x0F, // LD A, 0x06 - 8 clocks
            0x06, 0x0F, // LD B, 0x05 - 8 clocks
            0xA8, // XOR A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x0);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_cp_greater() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3E, 0x0C, // LD A, 0x0C - 8 clocks
            0x06, 0x0F, // LD B, 0x0F - 8 clocks
            0xB8, // CP A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x0C);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_cp_equal() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3E, 0x0C, // LD A, 0x06 - 8 clocks
            0x06, 0x0C, // LD B, 0x05 - 8 clocks
            0xB8, // CP A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x0C);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_cp_less() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x3E, 0x0C, // LD A, 0x06 - 8 clocks
            0x06, 0x08, // LD B, 0x05 - 8 clocks
            0xB8, // CP A, B - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x0C);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_increment_8() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x2E, 0xFE, // LD L, 0x00 - 8 clocks
            0x2C, // INC L - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::L), 0xFF);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 12);

    let gb = run_program(
        2,
        &[
            0x2E, 0xFF, // LD L, 0x00 - 8 clocks
            0x2C, // INC L - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::L), 0x00);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 12);

    Ok(())
}

#[test]
fn test_decrement_8() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x2E, 0x02, // LD L, 0x00 - 8 clocks
            0x2D, // DEC L - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::L), 0x01);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 12);

    let gb = run_program(
        2,
        &[
            0x2E, 0x01, // LD L, 0x00 - 8 clocks
            0x2D, // DEC L - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::L), 0x00);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 12);

    let gb = run_program(
        2,
        &[
            0x2E, 0x00, // LD L, 0x00 - 8 clocks
            0x2D, // DEC L - 4 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::L), 0xFF);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), true);
    assert_eq!(gb.clocks_elapsed(), 12);

    Ok(())
}

#[test]
fn test_increment_16() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x26, 0x01, // LD H, 0x01 - 8 clocks
            0x2E, 0xFF, // LD L, 0xFF - 8 clocks
            0x23, // INC HL - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u16(registers::WordRegister::HL), 0x200);
    assert_eq!(gb.clocks_elapsed(), 24);

    Ok(())
}

#[test]
fn test_decrement_16() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x26, 0x01, // LD H, 0x01 - 8 clocks
            0x2E, 0x00, // LD L, 0xFF - 8 clocks
            0x2B, // DEC HL - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u16(registers::WordRegister::HL), 0xFF);
    assert_eq!(gb.clocks_elapsed(), 24);

    Ok(())
}

#[test]
fn test_add_16() -> StepResult<()> {
    let gb = run_program(
        5,
        &[
            0x26, 0x0F, // LD H, 0x0F - 8 clocks
            0x2E, 0xFF, // LD L, 0xFF - 8 clocks
            0x06, 0x00, // LD B, 0 - 8 clocks
            0x0E, 0x01, // LD C, 1 - 8 clocks
            0x09, // ADD HL, BC - 8 clocks
        ],
    )?;

    assert_eq!(
        gb.cpu.read_register_u16(registers::WordRegister::HL),
        0x1000
    );
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 40);

    Ok(())
}

#[test]
fn test_add_16_carry() -> StepResult<()> {
    let gb = run_program(
        5,
        &[
            0x26, 0x0F, // LD H, 0x0F - 8 clocks
            0x2E, 0xFF, // LD L, 0xFF - 8 clocks
            0x06, 0xF0, // LD B, 0 - 8 clocks
            0x0E, 0x02, // LD C, 1 - 8 clocks
            0x09, // ADD HL, BC - 8 clocks
        ],
    )?;

    assert_eq!(
        gb.cpu.read_register_u16(registers::WordRegister::HL),
        0x0001
    );
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), false);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 40);

    Ok(())
}

#[test]
fn test_add_16_zero() -> StepResult<()> {
    let gb = run_program(
        5,
        &[
            0x26, 0xFF, // LD H, 0x0F - 8 clocks
            0x2E, 0xFF, // LD L, 0xFF - 8 clocks
            0x06, 0x00, // LD B, 0 - 8 clocks
            0x0E, 0x01, // LD C, 1 - 8 clocks
            0x09, // ADD HL, BC - 8 clocks
        ],
    )?;

    assert_eq!(gb.cpu.read_register_u16(registers::WordRegister::HL), 0x0);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Zero), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::Carry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::HalfCarry), true);
    assert_eq!(gb.cpu.read_flag(registers::Flag::AddSubtract), false);
    assert_eq!(gb.clocks_elapsed(), 40);

    Ok(())
}
