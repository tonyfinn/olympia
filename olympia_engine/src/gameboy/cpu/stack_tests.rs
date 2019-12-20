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
