use super::testutils::*;
use super::*;

#[test]
fn test_jump() -> StepResult<()> {
    let gb = run_program(
        1,
        &[
            0xC3, 0x13, 0x20, // JP 0x2013 - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::PC), 0x2013);
    assert_eq!(gb.clocks_elapsed(), 16);

    Ok(())
}

#[test]
fn test_jump_if_carry() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x37, // SCF - 4 clocks
            0xDA, 0x13, 0x20, // JP C, 0x2013 - 16/12 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::PC), 0x2013);
    assert_eq!(gb.clocks_elapsed(), 20);

    let gb = run_program(
        2,
        &[
            0x3F, // CCF - 4 clocks
            0xDA, 0x13, 0x20, // JP C, 0x2013 - 16/12 clocks
        ],
    )?;

    assert_eq!(
        gb.read_register_u16(registers::WordRegister::PC),
        PROGRAM_START + 4
    );
    assert_eq!(gb.clocks_elapsed(), 16);

    Ok(())
}

#[test]
fn test_jump_if_nocarry() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x37, // SCF - 4 clocks
            0xD2, 0x13, 0x20, // JP C, 0x2013 - 16/12 clocks
        ],
    )?;

    assert_eq!(
        gb.read_register_u16(registers::WordRegister::PC),
        PROGRAM_START + 4
    );
    assert_eq!(gb.clocks_elapsed(), 16);

    let gb = run_program(
        2,
        &[
            0x3F, // CCF - 4 clocks
            0xD2, 0x13, 0x20, // JP C, 0x2013 - 16/12 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::PC), 0x2013);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_jump_if_zero() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0xBF, // CP A - 4 clocks (set zero flag)
            0xCA, 0x13, 0x20, // JP Z, 0x2013 - 16/12 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::PC), 0x2013);
    assert_eq!(gb.clocks_elapsed(), 20);

    let gb = run_program(
        2,
        &[
            0xC6, 1, // ADD A, 1 - 8 clocks (clear zero flag)
            0xCA, 0x13, 0x20, // JP Z, 0x2013 - 16/12 clocks
        ],
    )?;

    assert_eq!(
        gb.read_register_u16(registers::WordRegister::PC),
        PROGRAM_START + 5
    );
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_jump_if_nonzero() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0xBF, // CP A - 4 clocks (set zero flag)
            0xC2, 0x13, 0x20, // JP NZ, 0x2013 - 16/12 clocks
        ],
    )?;

    assert_eq!(
        gb.read_register_u16(registers::WordRegister::PC),
        PROGRAM_START + 4
    );
    assert_eq!(gb.clocks_elapsed(), 16);

    let gb = run_program(
        2,
        &[
            0xC6, 1, // ADD A, 1 - 8 clocks (clear zero flag)
            0xC2, 0x13, 0x20, // JP NZ, 0x2013 - 16/12 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::PC), 0x2013);
    assert_eq!(gb.clocks_elapsed(), 24);

    Ok(())
}

#[test]
fn test_register_jump() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0x26, 0x20, // LD H, 0x20 - 8 clocks
            0x2E, 0x31, // LD L, 0x31 - 8 blocks
            0xE9, // JP HL - 4 clocks
        ],
    )?;

    assert_eq!(gb.read_register_u16(registers::WordRegister::PC), 0x2031);
    assert_eq!(gb.clocks_elapsed(), 20);

    Ok(())
}

#[test]
fn test_relative_jump() -> StepResult<()> {
    let gb = run_program(
        1,
        &[
            0x18,
            (-4i8).to_le_bytes()[0], // JR -4 - 12 clocks
        ],
    )?;

    assert_eq!(
        gb.read_register_u16(registers::WordRegister::PC),
        PROGRAM_START - 2
    );
    assert_eq!(gb.clocks_elapsed(), 12);

    let gb = run_program(
        1,
        &[
            0x18,
            (4i8).to_le_bytes()[0], // JR -4 - 12 clocks
        ],
    )?;

    assert_eq!(
        gb.read_register_u16(registers::WordRegister::PC),
        PROGRAM_START + 6
    );
    assert_eq!(gb.clocks_elapsed(), 12);

    Ok(())
}

#[test]
fn test_relative_jump_if() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x37, // SCF - 4 blocks
            0x38, 0x02, // JR C, 5 - 12/8 clocks
            0x76, // HALT
            0x30, 0x02, // JR NC, 2 - 12/8 clocks
            0x06, 0x12, // LD B, 0x12 - 8 clocks
            0x00, 0x76, // HALT
        ], // Expected path is SCF - JR C, 5 (jumps) - JR NC, 2 (no jump) - LD B, 0x12
    )?;

    assert_eq!(gb.read_register_u8(registers::ByteRegister::B), 0x12);
    assert_eq!(
        gb.read_register_u16(registers::WordRegister::PC),
        PROGRAM_START + 8
    );
    assert_eq!(gb.clocks_elapsed(), 32);

    Ok(())
}

#[test]
fn test_relative_jump_backwards() -> StepResult<()> {
    let gb = run_program(
        4,
        &[
            0x18, 0x03, // JR 3 - 12 clocks
            0x76, // HALT
            0x06, 0x12, // LD B, 0x12 - 8 clocks
            0x37, // SCF - 4 clocks
            0x38, 0xFB, // JR C, -5 - 12/8 clocks
        ], // Expected path is JR 3, SCF, JR C, -2 (jumps), LD B, 0x12
    )?;

    assert_eq!(gb.read_register_u8(registers::ByteRegister::B), 0x12);
    assert_eq!(
        gb.read_register_u16(registers::WordRegister::PC),
        PROGRAM_START + 5
    );
    assert_eq!(gb.clocks_elapsed(), 36);

    Ok(())
}

#[test]
fn test_call() -> StepResult<()> {
    let gb = run_program(
        1,
        &[
            0xCD, 0x20, 0x30, // CALL 0x3020 - 24 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u16(0xFFFC)?, 0x0203);
    assert_eq!(gb.read_register_u16(registers::WordRegister::PC), 0x3020);
    assert_eq!(gb.read_register_u16(registers::WordRegister::SP), 0xFFFC);
    assert_eq!(gb.clocks_elapsed(), 24);

    Ok(())
}

#[test]
fn test_call_system() -> StepResult<()> {
    let gb = run_program(
        1,
        &[
            0xCF, // RST 0x08 - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u16(0xFFFC)?, 0x0201);
    assert_eq!(gb.read_register_u16(registers::WordRegister::PC), 0x08);
    assert_eq!(gb.read_register_u16(registers::WordRegister::SP), 0xFFFC);
    assert_eq!(gb.clocks_elapsed(), 16);

    Ok(())
}

#[test]
fn test_return() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0xCD, 0x06, 0x02, // CALL 0x206 - 24 clocks
            0x00, 0x00, 0x00, // NOP, NOP, NOP
            0xC9, // RET - 16 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u16(0xFFFC)?, 0x0203);
    assert_eq!(gb.read_register_u16(registers::WordRegister::PC), 0x203);
    assert_eq!(gb.read_register_u16(registers::WordRegister::SP), 0xFFFE);
    assert_eq!(gb.clocks_elapsed(), 40);

    Ok(())
}

#[test]
fn test_return_if() -> StepResult<()> {
    let gb = run_program(
        3,
        &[
            0xCD, 0x06, 0x02, // CALL 0x206 - 24 clocks
            0x00, 0x00, 0x00, // NOP, NOP, NOP
            0x37, // SCF - 4 clocks
            0xD8, // RET C -  20/8 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u16(0xFFFC)?, 0x0203);
    assert_eq!(gb.read_register_u16(registers::WordRegister::SP), 0xFFFE);
    assert_eq!(gb.read_register_u16(registers::WordRegister::PC), 0x203);
    assert_eq!(gb.clocks_elapsed(), 48);

    let gb = run_program(
        3,
        &[
            0xCD, 0x06, 0x02, // CALL 0x206 - 24 clocks
            0x00, 0x00, 0x00, // NOP, NOP, NOP
            0x3F, // CCF - 4 clocks
            0xD8, // RET C -  20/8 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u16(0xFFFC)?, 0x0203);
    assert_eq!(gb.read_register_u16(registers::WordRegister::SP), 0xFFFC);
    assert_eq!(gb.read_register_u16(registers::WordRegister::PC), 0x208);
    assert_eq!(gb.clocks_elapsed(), 36);

    Ok(())
}

#[test]
fn test_call_if() -> StepResult<()> {
    let gb = run_program(
        2,
        &[
            0x37, // SCF - 4 clocks
            0xDC, 0x20, 0x30, // CALL C, 0x3020 - 24/12 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u16(0xFFFC)?, 0x0204);
    assert_eq!(gb.read_register_u16(registers::WordRegister::PC), 0x3020);
    assert_eq!(gb.clocks_elapsed(), 28);

    let gb = run_program(
        2,
        &[
            0x3F, // CCF - 4 clocks
            0xDC, 0x20, 0x30, // CALL C, 0x3020 - 24/12 clocks
        ],
    )?;

    assert_eq!(gb.read_memory_u16(0xFFFC)?, 0x0000);
    assert_eq!(
        gb.read_register_u16(registers::WordRegister::PC),
        PROGRAM_START + 4
    );
    assert_eq!(gb.clocks_elapsed(), 16);

    Ok(())
}
