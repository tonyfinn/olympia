use crate::address::LiteralAddress;
use crate::gameboy;
use crate::registers;
use crate::rom;

pub const PROGRAM_START: u16 = 0x200;
pub const PROG_MEMORY_OFFSET: LiteralAddress = LiteralAddress(0x200);

type ProgramSegment<'a> = (LiteralAddress, &'a [u8]);

pub fn make_cartridge_with(segments: &[ProgramSegment]) -> rom::Cartridge {
    let mut data = vec![0u8; 0x8000];
    for segment in segments {
        let (start, segment_data) = segment;
        let LiteralAddress(raw_addr) = start;
        let offset = *raw_addr as usize;
        data[offset..offset + segment_data.len()].clone_from_slice(segment_data);
    }
    rom::Cartridge::from_data(data).unwrap()
}

pub fn run_program_with(
    steps: u64,
    segments: &[ProgramSegment],
) -> gameboy::StepResult<gameboy::GameBoy> {
    let cartridge = make_cartridge_with(segments);
    let mut gb = gameboy::GameBoy::new(cartridge, gameboy::GameBoyModel::GameBoy);
    gb.write_register_u16(registers::WordRegister::PC, PROGRAM_START);
    for _ in 0..steps {
        gb.step()?
    }
    Ok(gb)
}

pub fn run_program(steps: u64, program: &[u8]) -> gameboy::StepResult<gameboy::GameBoy> {
    run_program_with(steps, &[(PROG_MEMORY_OFFSET, program)])
}
