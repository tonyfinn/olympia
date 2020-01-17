use crate::gameboy::memory;

pub const OAM_BASE: u16 = 0xFE00;

#[derive(PartialEq, Eq, Debug)]
enum DmaState {
    Idle,
    Copying,
}

pub(crate) struct DmaUnit {
    state: DmaState,
    idx: u16,
    offset: u16,
    pub(crate) register_value: u8,
}

impl DmaUnit {
    pub(crate) fn start(&mut self, reg_value: u8) {
        self.register_value = reg_value;
        self.offset = u16::from(reg_value) * 0x100;
        self.idx = 0;
        self.state = DmaState::Copying;
    }

    pub(crate) fn run_cycle(&mut self, mem: &mut memory::Memory) -> memory::MemoryResult<()> {
        if mem.registers.dma != self.register_value {
            self.start(mem.registers.dma);
        }
        if self.state == DmaState::Copying {
            let index_to_try = self.idx;
            self.idx += 1;
            if self.idx == 160 {
                self.state = DmaState::Idle
            }
            let mem_value = mem.read_u8(self.offset + index_to_try)?;
            mem.write_u8(OAM_BASE + index_to_try, mem_value)?;
            Ok(())
        } else {
            Ok(())
        }
    }
}

impl Default for DmaUnit {
    fn default() -> DmaUnit {
        DmaUnit {
            state: DmaState::Idle,
            idx: 0,
            offset: 0,
            register_value: 0,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::gameboy::testutils;
    use crate::gameboy::{GameBoy, GameBoyModel};
    use crate::rom::Cartridge;
    use alloc::vec::Vec;

    fn make_gameboy_dma_data(start_index: usize, sample_data: Vec<u8>) -> GameBoy {
        let mut rom_data = vec![0; 0x8000];
        for i in 0..sample_data.len() {
            rom_data[start_index + i] = *sample_data.get(i).unwrap();
        }
        GameBoy::new(
            Cartridge::from_data(rom_data).unwrap(),
            GameBoyModel::GameBoy,
        )
    }

    #[test]
    fn test_idle() {
        let dma_data = vec![0x23; 160];
        let mut gameboy = make_gameboy_dma_data(0x2000, dma_data);
        for _ in 0..200 {
            gameboy.dma.run_cycle(&mut gameboy.mem).unwrap();
        }

        for i in 0xfe00..0xfea0 {
            assert_eq!(0, gameboy.read_memory_u8(i).unwrap());
        }
    }

    #[test]
    fn test_copy() {
        let dma_data = vec![0x23; 160];
        let mut gameboy = make_gameboy_dma_data(0x2000, dma_data);
        gameboy.mem.registers.dma = 0x20;

        for _ in 0..200 {
            gameboy.dma.run_cycle(&mut gameboy.mem).unwrap();
        }

        assert_eq!(gameboy.dma.state, DmaState::Idle);

        for i in 0xfe00..0xfea0 {
            assert_eq!(0x23, gameboy.read_memory_u8(i).unwrap());
        }
    }

    #[test]
    fn test_dma_full() {
        let dma_code: Vec<u8> = vec![
            0x3e, 0xc0, // LD A, 0xC0 - 8 clocks
            0xe0, 0x46, // LDH (DMA), A - 12 clocks -- (DMA = 0xFF00 + 0x46)
            0x3e, 40,   // LD A, 40 - 8 clocks
            0x3d, // DEC A - 4 clocks
            0x20, 0xfd, // JR NZ, -2 - 12 clocks
            0xc9, // RET - 16 clocks
        ];
        let dma_code_len = dma_code.len() as u64;

        let dma_cycles: u64 = (2 + 3 + 2 + 4) + 159; // Last cycle is shorter as it doesn't jump
        let dma_instruction_cnt: u64 = 3 + (2 * 40) + 1;

        let mut loader_code: Vec<u8> = vec![];
        for (i, byte) in dma_code.iter().enumerate() {
            let offset = 0x80 + (i as u8);
            loader_code.append(&mut vec![
                0x3e, *byte, // LD A, byte - 8 clocks
                0xE0, offset, // LDH (offset), A - 12 clocks
            ]);
        }
        let mut loader_instruction_cnt = dma_code_len * 2;
        let mut loader_cycles = dma_code_len * 5;

        loader_code.append(&mut vec![
            0x26, 0xC0, // LD H, 0xC0 - 8 clocks
            0x2E, 0x00, // LD L, 0 - 8 clocks
        ]);
        loader_instruction_cnt += 2;
        loader_cycles += 4;

        for i in 0..160 {
            loader_code.append(&mut vec![
                0x36, i,    // LD (HL), i - 12 clocks
                0x2C, // INC L - 4 clocks
            ]);
            loader_instruction_cnt += 2;
            loader_cycles += 4;
        }

        loader_code.append(&mut vec![
            0xcd, 0x80, 0xff, // CALL 0xff80 - 24 clocks
        ]);
        loader_instruction_cnt += 1;
        loader_cycles += 6;

        let total_ins_cnt = dma_instruction_cnt + loader_instruction_cnt;

        let gb = testutils::run_program(total_ins_cnt, &loader_code).unwrap();

        assert_eq!(gb.dma.register_value, 0xc0);
        assert_eq!(gb.dma.state, DmaState::Idle);
        assert_eq!(gb.dma.offset, 0xC000);
        assert_eq!(gb.dma.idx, 160);
        for addr in 0xfe00..0xfea0 {
            let value = gb.read_memory_u8(addr).unwrap();
            let expected_value = (addr - 0xfe00) as u8;
            assert_eq!(
                value, expected_value,
                "Expecting addr 0x{:X} to be 0x{:X}, but is 0x{:X}",
                addr, expected_value, value
            );
        }
        assert_eq!(gb.clocks_elapsed(), (loader_cycles + dma_cycles) * 4);
    }
}
