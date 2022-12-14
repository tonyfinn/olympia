//! Emulation core for a Gameboy.
//!
//! This crate implements all instructions and their effects on the gameboy,
//! and provides methods to query the internal state of the gameboy for the frontend.
//!
//! To instantiate a new gameboy, first decide what cartridge is inserted in
//! the gameboy, and which model of gameboy is being emulated. Then you can use
//! [`GameBoy::new(cartridge, model)`][Gameboy::new] to instantiate an emulated gameboy.
//!
//! Note that at this early stage, the emulator focuses primarily on DMG emulation,
//! and does not support CGB or SGB exclusive features. As such, these systems
//! currently only run in the DMG-compatible mode.
//!
//! [Gameboy::new]: struct.GameBoy.html#method.new
pub(crate) mod cpu;
mod dma;
pub(crate) mod memory;
mod ppu;
mod timer;

pub use cpu::CYCLE_FREQ;
pub use memory::{MemoryError, MemoryRegion, MemoryResult, VRAM};
pub use ppu::{GBPixel, Palette};

use crate::events;
use crate::gameboy::cpu::Cpu;
use crate::gameboy::cpu::PowerSavingMode;
use crate::gameboy::dma::DmaUnit;
use crate::instructions;
use crate::instructionsn as new_instructions;
use crate::registers;
use crate::registers::WordRegister as wr;
use crate::rom;
use crate::rom::TargetConsole;

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::convert::TryFrom;
use derive_more::Display;
use olympia_core::address;

use self::cpu::CLOCKS_PER_CYCLE;

/// Primary struct for an emulated gameboy.
///
/// # Example usage:
///
/// ```
/// # use olympia_engine::rom::Cartridge;
/// # use olympia_engine::gameboy::{GameBoy,GameBoyModel};
/// # let cartridge_data = vec![0u8; 0x8000];
/// // let cartridge_data: Vec<u8> = read_from_fs("my.rom");
/// let cartridge = Cartridge::from_data(cartridge_data).unwrap();
/// let mut gb = GameBoy::new(cartridge, GameBoyModel::GameBoy);
///
/// // in your event loop or elsewhere, at a 4mhz interval
/// gb.step();
/// ```
pub struct GameBoy {
    pub(crate) cpu: Cpu,
    pub(crate) mem: memory::Memory,
    pub(crate) ppu: ppu::Ppu,
    pub(crate) timer: timer::Timer,
    dma: DmaUnit,
    runtime_decoder: Rc<new_instructions::RuntimeDecoder>,
    clocks_elapsed: u64,
    time_elapsed: f64,
    pub events: Rc<events::EventEmitter<events::Event>>,
}

#[derive(PartialEq, Eq, Debug, Display)]
/// Represents an error that occurred while performing
/// an emulated instruction.
pub enum StepError {
    /// Errors related to memory access
    #[display(fmt = "Accessing invalid memory location: {0}", _0)]
    Memory(memory::MemoryError),
    /// Opcodes that don't map to a valid instruction
    #[display(fmt = "Attempted to exec invalid opcode {}", _0)]
    InvalidOpcode(u8),
}

#[cfg(feature = "std")]
impl std::error::Error for StepError {}

impl From<memory::MemoryError> for StepError {
    fn from(err: memory::MemoryError) -> Self {
        StepError::Memory(err)
    }
}

pub type StepResult<T> = Result<T, StepError>;
impl GameBoy {
    /// Creates a new gameboy.
    ///
    /// # Arguments
    ///
    /// * `cartridge` is the currently inserted Cartridge
    /// * `model` is the model of gameboy this should represent. Note this
    ///   should be set to the actual hardware type desired, not it's target
    ///   mode. For a Game Boy Color in Game Boy mode, this should be set to
    ///   `GameBoyModel::GameBoyColor`. The actual emulated mode is detected
    ///   based on whether the ROM declares itself to be Game Boy Color enhanced
    ///   or exclusive.
    ///
    pub fn new(cartridge: rom::Cartridge, model: GameBoyModel) -> GameBoy {
        let gb = GameBoy {
            cpu: Cpu::new(model, cartridge.target),
            mem: memory::Memory::new(cartridge),
            dma: Default::default(),
            ppu: Default::default(),
            timer: timer::Timer::default(),
            runtime_decoder: Rc::new(new_instructions::RuntimeDecoder::new()),
            clocks_elapsed: 0,
            time_elapsed: 0.0,
            events: Rc::new(events::EventEmitter::new()),
        };

        events::propagate_events(&gb.cpu.events, gb.events.clone());
        events::propagate_events(&gb.mem.events, gb.events.clone());
        events::propagate_events(&gb.ppu.events, gb.events.clone());

        gb
    }

    pub fn add_exec_time(&mut self, time: f64) {
        self.time_elapsed += time;
    }

    /// Query a value at the given address
    ///
    /// This should be used by external consumers, as it will not trigger read breakpoints
    pub fn get_memory_u8<A: Into<address::LiteralAddress>>(
        &self,
        addr: A,
    ) -> memory::MemoryResult<u8> {
        self.mem.read_u8_internal(addr.into())
    }

    /// Sets a memory value at the given address
    ///
    /// This should be used by external consumers, as it will not trigger write breakpoints
    pub fn set_memory_u8<A: Into<address::LiteralAddress>>(
        &mut self,
        addr: A,
        val: u8,
    ) -> memory::MemoryResult<()> {
        self.mem.write_u8_internal(addr.into(), val)
    }

    /// Read a value from the given memory address.
    ///
    /// This should only be used by the gameboy engine as it will trigger read breakpoints
    pub(crate) fn read_memory_u8<A: Into<address::LiteralAddress>>(
        &self,
        addr: A,
    ) -> memory::MemoryResult<u8> {
        self.mem.read_u8(addr)
    }

    /// Write a value to the given memory address.
    ///
    /// This should only be used by the gameboy engine as it will trigger read breakpoints
    pub(crate) fn write_memory_u8<A: Into<address::LiteralAddress>>(
        &mut self,
        addr: A,
        val: u8,
    ) -> memory::MemoryResult<()> {
        self.mem.write_u8(addr, val)
    }

    /// Read an value at the given memory address as a signed integer.
    ///
    /// This is primarily useful for reading the target of a JR instruction.
    /// This should be used by external consumers, as it will not trigger read breakpoints
    pub fn get_memory_i8<A: Into<address::LiteralAddress>>(
        &self,
        addr: A,
    ) -> memory::MemoryResult<i8> {
        Ok(i8::from_le_bytes([self
            .mem
            .read_u8_internal(addr.into())?]))
    }

    /// Read a 16-bit value from the address at `target`
    ///
    /// Note that the value is read in little endian format.
    /// This means that given `0xC000` = `0x12` and `0xC001` = `0x45`,
    /// the value read will be `0x4512`
    /// This should be used by external consumers, as it will not trigger read breakpoints
    pub fn get_memory_u16<A: Into<address::LiteralAddress>>(
        &self,
        target: A,
    ) -> memory::MemoryResult<u16> {
        let addr = target.into();
        Ok(u16::from_le_bytes([
            self.mem.read_u8_internal(addr)?,
            self.mem.read_u8_internal(addr.next())?,
        ]))
    }

    /// Write a 16-bit value to the address at `target`
    ///
    /// Note that the value is written in little endian format.
    /// This means that given value of `0xABCD` and `target` of `0xC000`
    /// then `0xC000` will be set to `0xCD` and `0xC001` will be set to `0xAB`
    /// This should be used by external consumers, as it will not trigger write breakpoints
    pub fn set_memory_u16<A: Into<address::LiteralAddress>>(
        &mut self,
        target: A,
        value: u16,
    ) -> memory::MemoryResult<()> {
        let addr = target.into();
        let bytes = value.to_le_bytes();

        self.mem.write_u8_internal(addr, bytes[0])?;
        self.mem.write_u8_internal(addr.next(), bytes[1])?;
        Ok(())
    }

    pub(crate) fn exec_write_memory_u16<A: Into<address::LiteralAddress>>(
        &mut self,
        target: A,
        value: u16,
    ) -> memory::MemoryResult<()> {
        let addr = target.into();
        let bytes = value.to_le_bytes();

        self.mem.write_u8(addr, bytes[0])?;
        self.cycle();
        self.mem.write_u8(addr.next(), bytes[1])?;
        self.cycle();
        Ok(())
    }

    /// Read a value from the given 16-bit CPU register
    pub fn read_register_u16(&self, reg: registers::WordRegister) -> u16 {
        self.cpu.read_register_u16(reg)
    }

    /// Write a value to a given 16-bit CPU register
    pub fn write_register_u16(&mut self, reg: registers::WordRegister, val: u16) {
        self.cpu.write_register_u16(reg, val)
    }

    /// Read a value from the given 8-bit CPU register
    pub fn read_register_u8(&self, reg: registers::ByteRegister) -> u8 {
        self.cpu.read_register_u8(reg)
    }

    /// Write a value to the given 8-bit CPU register
    pub fn write_register_u8(&mut self, reg: registers::ByteRegister, val: u8) {
        self.cpu.write_register_u8(reg, val)
    }

    pub(crate) fn exec_read_register_target(
        &mut self,
        target: instructions::ByteRegisterTarget,
    ) -> StepResult<u8> {
        match registers::ByteRegister::try_from(target) {
            Ok(reg) => Ok(self.read_register_u8(reg)),
            Err(_) => {
                let addr = self.read_register_u16(wr::HL);
                let value = self.read_memory_u8(addr)?;
                self.cycle();
                Ok(value)
            }
        }
    }

    pub(crate) fn exec_write_register_target(
        &mut self,
        target: instructions::ByteRegisterTarget,
        value: u8,
    ) -> StepResult<()> {
        match registers::ByteRegister::try_from(target) {
            Ok(reg) => {
                self.write_register_u8(reg, value);
                Ok(())
            }
            Err(_) => {
                let addr = self.read_register_u16(wr::HL);
                self.write_memory_u8(addr, value)?;
                self.cycle();
                Ok(())
            }
        }
    }

    pub(crate) fn set_interrupt_state(&mut self, state: cpu::InterruptState) {
        log::trace!(target: "cpu", "set interrupt mode: {:?}", state);
        self.cpu.interrupts_enabled = state;
    }

    pub fn power_saving_mode(&self) -> cpu::PowerSavingMode {
        self.cpu.power_saving
    }

    pub fn set_power_saving_mode(&mut self, mode: cpu::PowerSavingMode) {
        log::trace!(target: "cpu", "set power saving mode: {:?}", mode);
        self.cpu.power_saving = mode
    }

    pub fn read_flag(&self, flag: registers::Flag) -> bool {
        self.cpu.read_flag(flag)
    }

    pub fn set_flag_to(&mut self, flag: registers::Flag, value: bool) {
        self.cpu.set_flag_to(flag, value);
    }

    pub(crate) fn set_flag(&mut self, flag: registers::Flag) {
        self.cpu.set_flag(flag);
    }

    pub(crate) fn reset_flag(&mut self, flag: registers::Flag) {
        self.cpu.reset_flag(flag);
    }

    pub(crate) fn cycling_memory_iter(&mut self) -> CyclingMemoryIterator {
        CyclingMemoryIterator { gb: self }
    }

    fn memory_iter(&self, start: address::LiteralAddress) -> memory::MemoryIterator {
        self.mem.offset_iter(start)
    }

    pub(crate) fn exec_read_inc_pc(&mut self) -> StepResult<u8> {
        let pc_value = self.cpu.read_register_u16(wr::PC);
        let val = self.read_memory_u8(pc_value)?;
        self.cpu
            .write_register_u16(wr::PC, pc_value.wrapping_add(1));
        self.cycle();
        Ok(val)
    }

    pub(crate) fn exec_push<T: Into<u16>>(&mut self, value: T) -> StepResult<()> {
        let stack_addr = self.cpu.read_register_u16(registers::WordRegister::SP);
        let [low, high] = value.into().to_le_bytes();
        let stack_addr = stack_addr.wrapping_sub(1);
        self.write_memory_u8(stack_addr, high)?;
        self.cycle();
        let stack_addr = stack_addr.wrapping_sub(1);
        self.write_memory_u8(stack_addr, low)?;
        self.cycle();
        self.cpu
            .write_register_u16(registers::WordRegister::SP, stack_addr);
        self.cycle();
        Ok(())
    }

    pub(crate) fn exec_pop<T: From<u16>>(&mut self) -> StepResult<T> {
        let stack_addr = self.cpu.read_register_u16(registers::WordRegister::SP);
        let low = self.read_memory_u8(stack_addr)?;
        self.cycle();
        let stack_addr = stack_addr.wrapping_add(1);
        let high = self.read_memory_u8(stack_addr)?;
        self.cycle();
        self.cpu
            .write_register_u16(registers::WordRegister::SP, stack_addr.wrapping_add(1));
        let value = u16::from_le_bytes([low, high]);
        Ok(T::from(value))
    }

    pub(crate) fn set_pc<A: Into<address::LiteralAddress>>(&mut self, target: A) {
        let address::LiteralAddress(addr) = target.into();
        self.cpu.write_register_u16(wr::PC, addr);
    }

    pub(crate) fn read_pc(&self) -> address::LiteralAddress {
        let value = self.cpu.read_register_u16(wr::PC);
        address::LiteralAddress(value)
    }

    fn check_interrupts(&mut self) -> StepResult<bool> {
        use cpu::InterruptState::{Disabled, Enabled, Pending};
        match self.cpu.interrupts_enabled {
            Pending => {
                self.set_interrupt_state(cpu::InterruptState::Enabled);
                Ok(false)
            }
            Disabled => Ok(false),
            Enabled => {
                let itest =
                    cpu::Interrupt::test(self.mem.registers().ie, self.mem.registers().iflag);
                if let Some(interrupt) = itest {
                    self.cycle();
                    self.cycle();
                    self.set_interrupt_state(cpu::InterruptState::Disabled);
                    interrupt.clear(&mut self.mem.registers_mut().iflag);
                    let addr = interrupt.handler_address();
                    self.exec_push(self.read_pc())?;
                    self.set_pc(addr);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }

    /// Runs a single instruction.
    ///
    /// Note that this instruction may take multiple machine cycles to
    /// execute. All components of the gameboy will run for this many machine
    /// cycles. To find out how many clocks elapsed, use `GameBoy::clocks_elapsed`.
    pub fn step(&mut self) -> StepResult<()> {
        log::trace!(target: "gb", "Step");
        if self.cpu.power_saving == PowerSavingMode::Stop {
            return Ok(());
        }
        let pc_value = self.read_pc();
        let opcode = self.read_memory_u8(pc_value)?;
        self.cycle();
        let interrupted = self.check_interrupts()?;
        if !interrupted {
            self.set_pc(pc_value.next());
            let non_borrowing_decoder = self.runtime_decoder.clone();
            let exe_code = if non_borrowing_decoder.is_extended(opcode) {
                let extended_opcode = self.exec_read_inc_pc()?;
                non_borrowing_decoder.decode_extended(extended_opcode)
            } else if let Some(exe_code) = non_borrowing_decoder.decode(opcode) {
                exe_code
            } else {
                return Err(StepError::InvalidOpcode(opcode));
            };
            exe_code
                .to_instruction(&mut self.cycling_memory_iter())
                .execute(self)?;
        }
        Ok(())
    }

    /// Returns the instruction at the current PC.
    pub fn current_instruction(
        &self,
    ) -> StepResult<Box<dyn crate::instructionsn::RuntimeInstruction>> {
        let mut pc_value = self.read_pc();
        let opcode = self.read_memory_u8(pc_value)?;
        let exe_code = if self.runtime_decoder.is_extended(opcode) {
            pc_value = pc_value.next();
            let extended_opcode = self.read_memory_u8(pc_value)?;
            self.runtime_decoder.decode_extended(extended_opcode)
        } else if let Some(exe_code) = self.runtime_decoder.decode(opcode) {
            exe_code
        } else {
            return Err(StepError::InvalidOpcode(opcode));
        };
        Ok(exe_code.to_instruction(&mut self.memory_iter(pc_value.next())))
    }

    pub(crate) fn cycle(&mut self) {
        // TODO: Use this. a memory error can occur if the DMA operation tries to
        // write to cartridge RAM that is not present. As with actual hardware,
        // the DMA operation continues, and so we shouldn't abort emulation early,
        // but it would be useful to surface this information somewhere for ROM developers.
        let _dma_result = self.dma.run_cycle(&mut self.mem);
        self.ppu.run_cycle(&mut self.mem);
        self.add_clocks_elapsed(4);
    }

    pub fn add_clocks_elapsed(&mut self, count: u64) {
        self.clocks_elapsed += u64::from(CLOCKS_PER_CYCLE);
        self.timer.tick(&mut self.mem, count);
    }

    /// Query how many CPU clocks have elapsed since the emulator started
    pub fn clocks_elapsed(&self) -> u64 {
        self.clocks_elapsed
    }

    /// Query how many machine cycles have elapsed since the emulator started
    ///
    /// Each machine cycle represents 4 CPU clocks.
    pub fn cycles_elapsed(&self) -> u64 {
        self.clocks_elapsed() / 4
    }

    /// Query much clock time has been spent emulating
    pub fn time_elapsed(&self) -> f64 {
        self.time_elapsed
    }
}

pub(crate) struct CyclingMemoryIterator<'a> {
    gb: &'a mut GameBoy,
}

impl<'a> Iterator for CyclingMemoryIterator<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        let addr = self.gb.read_pc();
        let value = self.gb.read_memory_u8(addr).ok();
        self.gb.cycle();
        self.gb.set_pc(addr.next());
        value
    }
}

/// Represents a hardware type running a gameboy game.
///
/// Note that the presence of GBA models do not imply support
/// for GBA ROMs. However, the GBA has some differing behaviors
/// when running GB games compared to standard GB hardware.
pub enum GameBoyModel {
    GameBoy,          // DMG
    GameBoyPocket,    // MGB
    SuperGameBoy,     // SGB
    GameBoyColor,     // GBC
    GameBoyAdvance,   // AGB
    GameBoyAdvanceSP, // AGS
}

impl GameBoyModel {
    pub(crate) fn default_af(&self) -> u16 {
        match self {
            GameBoyModel::GameBoy => 0x01B0,
            GameBoyModel::GameBoyPocket => 0xFFB0,
            GameBoyModel::SuperGameBoy => 0x0100,
            GameBoyModel::GameBoyColor => 0x1180,
            GameBoyModel::GameBoyAdvance => 0x1100,
            GameBoyModel::GameBoyAdvanceSP => 0x1100,
        }
    }

    pub(crate) fn default_bc(&self) -> u16 {
        match self {
            GameBoyModel::GameBoy => 0x0013,
            GameBoyModel::GameBoyPocket => 0x0013,
            GameBoyModel::SuperGameBoy => 0x0014,
            GameBoyModel::GameBoyColor => 0x0000,
            GameBoyModel::GameBoyAdvance => 0x0100,
            GameBoyModel::GameBoyAdvanceSP => 0x0100,
        }
    }

    pub(crate) fn default_de(&self, target: TargetConsole) -> u16 {
        let gbc_mode = target != TargetConsole::GameBoyOnly;
        match self {
            GameBoyModel::GameBoy => 0x00D8,
            GameBoyModel::GameBoyPocket => 0x00D8,
            GameBoyModel::SuperGameBoy => 0x0000,
            GameBoyModel::GameBoyColor if gbc_mode => 0xFF56,
            GameBoyModel::GameBoyColor => 0x0008,
            GameBoyModel::GameBoyAdvance if gbc_mode => 0xFF56,
            GameBoyModel::GameBoyAdvance => 0x0008,
            GameBoyModel::GameBoyAdvanceSP if gbc_mode => 0xFF56,
            GameBoyModel::GameBoyAdvanceSP => 0x0008,
        }
    }

    pub(crate) fn default_hl(&self, target: TargetConsole) -> u16 {
        let gbc_mode = target != TargetConsole::GameBoyOnly;
        match self {
            GameBoyModel::GameBoy => 0x014D,
            GameBoyModel::GameBoyPocket => 0x014D,
            GameBoyModel::SuperGameBoy => 0xC060,
            GameBoyModel::GameBoyColor if gbc_mode => 0x000D,
            GameBoyModel::GameBoyColor => 0x007C,
            GameBoyModel::GameBoyAdvance if gbc_mode => 0x000D,
            GameBoyModel::GameBoyAdvance => 0x007C,
            GameBoyModel::GameBoyAdvanceSP if gbc_mode => 0x000D,
            GameBoyModel::GameBoyAdvanceSP => 0x007C,
        }
    }
}

#[cfg(test)]
pub(crate) mod testutils;

#[cfg(test)]
mod test {
    use super::*;
    use crate::gameboy::memory;
    use alloc::vec::Vec;

    fn make_cartridge() -> rom::Cartridge {
        rom::Cartridge::from_data(vec![0u8; 0x8000]).unwrap()
    }

    #[test]
    fn test_reg_write_u8_read_u8() {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_register_u8(registers::ByteRegister::A, 0x01);
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x01);

        gb.write_register_u8(registers::ByteRegister::B, 0x02);
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::B), 0x02);

        gb.write_register_u8(registers::ByteRegister::C, 0x03);
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::C), 0x03);

        gb.write_register_u8(registers::ByteRegister::D, 0x04);
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::D), 0x04);

        gb.write_register_u8(registers::ByteRegister::E, 0x05);
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::E), 0x05);

        gb.write_register_u8(registers::ByteRegister::F, 0x66);
        // F register lower 4 bytes are not writable
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::F), 0x60);

        gb.write_register_u8(registers::ByteRegister::H, 0x07);
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::H), 0x07);

        gb.write_register_u8(registers::ByteRegister::L, 0x08);
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::L), 0x08);
    }

    #[test]
    fn test_reg_write_u16_read_u16() {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_register_u16(registers::WordRegister::AF, 0x1234);
        // F register lower 4 bytes are not writable
        assert_eq!(
            gb.cpu.read_register_u16(registers::WordRegister::AF),
            0x1230
        );

        gb.write_register_u16(registers::WordRegister::BC, 0x1235);
        assert_eq!(
            gb.cpu.read_register_u16(registers::WordRegister::BC),
            0x1235
        );

        gb.write_register_u16(registers::WordRegister::DE, 0x1236);
        assert_eq!(
            gb.cpu.read_register_u16(registers::WordRegister::DE),
            0x1236
        );

        gb.write_register_u16(registers::WordRegister::HL, 0x1237);
        assert_eq!(
            gb.cpu.read_register_u16(registers::WordRegister::HL),
            0x1237
        );

        gb.write_register_u16(registers::WordRegister::PC, 0x1238);
        assert_eq!(
            gb.cpu.read_register_u16(registers::WordRegister::PC),
            0x1238
        );

        gb.write_register_u16(registers::WordRegister::SP, 0x1239);
        assert_eq!(
            gb.cpu.read_register_u16(registers::WordRegister::SP),
            0x1239
        );
    }

    #[test]
    fn test_reg_write_u8_read_u16() {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_register_u8(registers::ByteRegister::A, 0x15);
        gb.write_register_u8(registers::ByteRegister::F, 0x12);
        assert_eq!(
            gb.cpu
                .read_register_u16(registers::WordRegister::AF)
                .to_be_bytes(),
            // F register lower 4 bytes are not writable
            [0x15, 0x10]
        );

        gb.write_register_u8(registers::ByteRegister::B, 0x25);
        gb.write_register_u8(registers::ByteRegister::C, 0x22);
        assert_eq!(
            gb.cpu
                .read_register_u16(registers::WordRegister::BC)
                .to_be_bytes(),
            [0x25, 0x22]
        );

        gb.write_register_u8(registers::ByteRegister::D, 0x35);
        gb.write_register_u8(registers::ByteRegister::E, 0x32);
        assert_eq!(
            gb.cpu
                .read_register_u16(registers::WordRegister::DE)
                .to_be_bytes(),
            [0x35, 0x32]
        );

        gb.write_register_u8(registers::ByteRegister::H, 0x45);
        gb.write_register_u8(registers::ByteRegister::L, 0x42);
        assert_eq!(
            gb.cpu
                .read_register_u16(registers::WordRegister::HL)
                .to_be_bytes(),
            [0x45, 0x42]
        );
    }

    #[test]
    fn test_reg_write_u16_read_u8() {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_register_u16(registers::WordRegister::AF, 0x9876);
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::A), 0x98);
        // F register lower 4 bytes are not writable
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::F), 0x70);

        gb.write_register_u16(registers::WordRegister::BC, 0x9775);
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::B), 0x97);
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::C), 0x75);

        gb.write_register_u16(registers::WordRegister::DE, 0x9674);
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::D), 0x96);
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::E), 0x74);

        gb.write_register_u16(registers::WordRegister::HL, 0x9573);
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::H), 0x95);
        assert_eq!(gb.cpu.read_register_u8(registers::ByteRegister::L), 0x73);
    }

    #[test]
    fn test_mem_write_u8_read_u8_sysram() -> memory::MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_memory_u8(0xc100, 0x32)?;
        assert_eq!(gb.read_memory_u8(0xc100), Ok(0x32));
        Ok(())
    }

    #[test]
    fn test_mem_write_u16_read_u16_sysram() -> memory::MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.set_memory_u16(0xc100, 0x1032)?;
        assert_eq!(gb.get_memory_u16(0xc100), Ok(0x1032));
        Ok(())
    }

    #[test]
    fn test_mem_write_u8_read_u16_sysram() -> memory::MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_memory_u8(0xc100, 0x48)?;
        gb.write_memory_u8(0xc101, 0x94)?;

        assert_eq!(gb.get_memory_u16(0xc100), Ok(0x9448));
        Ok(())
    }

    #[test]
    fn test_mem_write_u16_read_u8_sysram() -> memory::MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.set_memory_u16(0xc200, 0x1345)?;

        assert_eq!(gb.read_memory_u8(0xc200), Ok(0x45));
        assert_eq!(gb.read_memory_u8(0xc201), Ok(0x13));
        Ok(())
    }

    #[test]
    fn test_write_u8_read_i8_sysram() -> memory::MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);
        let signed_value = i8::from_le_bytes([0xa2]);

        gb.write_memory_u8(0xc200, 0xa2)?;

        assert_eq!(gb.get_memory_i8(0xc200), Ok(signed_value));
        Ok(())
    }

    #[test]
    fn test_mem_write_u8_read_u8_vram() -> memory::MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_memory_u8(0x8100, 0x32)?;
        assert_eq!(gb.read_memory_u8(0x8100), Ok(0x32));
        Ok(())
    }

    #[test]
    fn test_mem_write_u8_read_u8_cpuram() -> memory::MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_memory_u8(0xff80, 0x32)?;
        assert_eq!(gb.read_memory_u8(0xff80), Ok(0x32));
        Ok(())
    }

    #[test]
    fn test_cycle_count() {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);
        gb.clocks_elapsed = 16;
        assert_eq!(gb.cycles_elapsed(), 4);
    }

    #[test]
    fn test_convert_memory_error() {
        let error = memory::MemoryError::InvalidRomAddress(0x7999);
        let mapped_error: StepError = error.into();

        assert_eq!(
            mapped_error,
            StepError::Memory(memory::MemoryError::InvalidRomAddress(0x7999))
        );
    }

    #[test]
    fn test_write_events() {
        use core::cell::RefCell;
        let event_log: Rc<RefCell<Vec<events::Event>>> = Rc::new(RefCell::new(Vec::new()));
        let handler_log = Rc::clone(&event_log);

        let handler: events::EventHandler<events::Event> = Box::new(move |evt| {
            handler_log.borrow_mut().push(evt.clone());
        });
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);
        gb.events.on(handler);

        gb.write_memory_u8(0x9456, 0x24).unwrap();
        gb.write_register_u16(wr::BC, 0x1234);

        let actual_events = event_log.borrow();

        assert_eq!(
            *actual_events,
            vec![
                events::MemoryEvent::write(0x9456.into(), 0x24, 0x24,).into(),
                events::RegisterWriteEvent::new(wr::BC, 0x1234).into(),
            ]
        );
    }
}
