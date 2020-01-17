use crate::gameboy::{cpu::Interrupt, testutils::*, StepResult};
use crate::registers::{ByteRegister as br, WordRegister as wr};

#[test]
fn test_vblank_handling() -> StepResult<()> {
    let gb = run_program_with(
        6,
        &[
            (
                PROG_MEMORY_OFFSET,
                &[
                    // Enable VBLANK
                    0xFB, // EI - 4 clocks
                    0x3E, 0x1, // LD A, 1 - 8 clocks
                    0xEA, 0xFF, 0xFF, // LD (0xFFFF), A - 16 clocks
                    0xEA, 0x0F, 0xFF, // LD (0xFF0F), A - 16 clocks
                ],
                // Read PC - 4 clocks
                // Interrupted - 20 clocks
            ),
            (
                Interrupt::VBlank.handler_address(),
                &[
                    0x06, 0x12, // LD B, 0x12 - 8 clocks
                ],
            ),
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 76);
    assert_eq!(gb.read_register_u16(wr::PC), 0x42);
    assert_eq!(gb.read_register_u8(br::B), 0x12);
    Ok(())
}

#[test]
fn test_lcdstatus_handling() -> StepResult<()> {
    let gb = run_program_with(
        6,
        &[
            (
                PROG_MEMORY_OFFSET,
                &[
                    // Enable LCDstatus
                    0xFB, // EI - 4 clocks
                    0x3E, 0x2, // LD A, 2 - 8 clocks
                    0xEA, 0xFF, 0xFF, // LD (0xFFFF), A - 16 clocks
                    0xEA, 0x0F, 0xFF, // LD (0xFF0F), A - 16 clocks
                ],
                // Read PC - 4 clocks
                // Interrupted - 20 clocks
            ),
            (
                Interrupt::LCDStatus.handler_address(),
                &[
                    0x06, 0x12, // LD B, 0x12 - 8 clocks
                ],
            ),
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 76);
    assert_eq!(gb.read_register_u16(wr::PC), 0x4A);
    assert_eq!(gb.read_register_u8(br::B), 0x12);
    Ok(())
}

#[test]
fn test_timer_handling() -> StepResult<()> {
    let gb = run_program_with(
        6,
        &[
            (
                PROG_MEMORY_OFFSET,
                &[
                    // Enable Timer
                    0xFB, // EI - 4 clocks
                    0x3E, 0x4, // LD A, 4 - 8 clocks
                    0xEA, 0xFF, 0xFF, // LD (0xFFFF), A - 16 clocks
                    0xEA, 0x0F, 0xFF, // LD (0xFF0F), A - 16 clocks
                ],
                // Read PC - 4 clocks
                // Interrupted - 20 clocks
            ),
            (
                Interrupt::Timer.handler_address(),
                &[
                    0x06, 0x12, // LD B, 0x12 - 8 clocks
                ],
            ),
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 76);
    assert_eq!(gb.read_register_u16(wr::PC), 0x52);
    assert_eq!(gb.read_register_u8(br::B), 0x12);
    Ok(())
}

#[test]
fn test_serial_handling() -> StepResult<()> {
    let gb = run_program_with(
        6,
        &[
            (
                PROG_MEMORY_OFFSET,
                &[
                    // Enable Serial
                    0xFB, // EI - 4 clocks
                    0x3E, 0x8, // LD A, 8 - 8 clocks
                    0xEA, 0xFF, 0xFF, // LD (0xFFFF), A - 16 clocks
                    0xEA, 0x0F, 0xFF, // LD (0xFF0F), A - 16 clocks
                ],
                // Read PC - 4 clocks
                // Interrupted - 20 clocks
            ),
            (
                Interrupt::Serial.handler_address(),
                &[
                    0x06, 0x12, // LD B, 0x12 - 8 clocks
                ],
            ),
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 76);
    assert_eq!(gb.read_register_u16(wr::PC), 0x5A);
    assert_eq!(gb.read_register_u8(br::B), 0x12);
    Ok(())
}

#[test]
fn test_input_handling() -> StepResult<()> {
    let gb = run_program_with(
        6,
        &[
            (
                PROG_MEMORY_OFFSET,
                &[
                    // Enable Input
                    0xFB, // EI - 4 clocks
                    0x3E, 0x10, // LD A, 16 - 8 clocks
                    0xEA, 0xFF, 0xFF, // LD (0xFFFF), A - 16 clocks
                    0xEA, 0x0F, 0xFF, // LD (0xFF0F), A - 16 clocks
                ],
                // Read PC - 4 clocks
                // Interrupted - 20 clocks
            ),
            (
                Interrupt::Input.handler_address(),
                &[
                    0x06, 0x12, // LD B, 0x12 - 8 clocks
                ],
            ),
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 76);
    assert_eq!(gb.read_register_u16(wr::PC), 0x62);
    assert_eq!(gb.read_register_u8(br::B), 0x12);
    Ok(())
}

#[test]
fn test_disable_interrupts() -> StepResult<()> {
    let gb = run_program_with(
        6,
        &[
            (
                PROG_MEMORY_OFFSET,
                &[
                    // Enable Input
                    0xFB, // EI - 4 clocks
                    0xF3, // DI - 4 clocks
                    0x3E, 0x10, // LD A, 16 - 8 clocks
                    0xEA, 0xFF, 0xFF, // LD (0xFFFF), A - 16 clocks
                    0xEA, 0x0F, 0xFF, // LD (0xFF0F), A - 16 clocks
                    0x06, 0x88, // LD B, 0x88 - 8 clocks
                ],
                // Read PC - 4 clocks
                // Interrupted - 20 clocks
            ),
            (
                Interrupt::Input.handler_address(),
                &[
                    0x06, 0x12, // LD B, 0x12 - 8 clocks
                ],
            ),
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 56);
    assert_eq!(gb.read_register_u8(br::B), 0x88);
    assert_eq!(gb.read_register_u16(wr::PC), 0x20C);
    Ok(())
}

#[test]
fn test_return_interrupts() -> StepResult<()> {
    let gb = run_program_with(
        7,
        &[
            (
                PROG_MEMORY_OFFSET,
                &[
                    // Enable Input
                    0xFB, // EI - 4 clocks
                    0x3E, 0x10, // LD A, 16 - 8 clocks
                    0xEA, 0xFF, 0xFF, // LD (0xFFFF), A - 16 clocks
                    0xEA, 0x0F, 0xFF, // LD (0xFF0F), A - 16 clocks
                    0x06, 0x88, // LD B, 0x88 - 8 clocks
                ],
                // Read PC - 4 clocks
                // Interrupted - 20 clocks
            ),
            (
                Interrupt::Input.handler_address(),
                &[
                    0x06, 0x12, // LD B, 0x12 - 8 clocks
                    0xD9, // RETI - 16 clocks
                ],
            ),
        ],
    )?;

    assert_eq!(gb.clocks_elapsed(), 92);
    assert_eq!(gb.read_register_u8(br::B), 0x12);
    assert_eq!(gb.read_register_u16(wr::PC), 0x209);
    Ok(())
}
