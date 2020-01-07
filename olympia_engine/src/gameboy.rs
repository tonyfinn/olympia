//! This crate represents the emulation core for a Gameboy.
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

pub use memory::{MemoryError, MemoryResult};

use crate::decoder;
use crate::gameboy::cpu::Cpu;
use crate::gameboy::dma::DmaUnit;
use crate::instructions;
use crate::registers;
use crate::registers::{ByteRegister as br, WordRegister as wr};
use crate::rom;
use crate::rom::TargetConsole;

use core::convert::TryFrom;
use alloc::rc::Rc;
use olympia_core::address;
use crate::instructionsn as new_instructions;

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
    dma: DmaUnit,
    decoder: decoder::Decoder,
    runtime_decoder: Rc<new_instructions::RuntimeDecoder>,
    clocks_elapsed: u64,
}

#[derive(PartialEq, Eq, Debug)]
enum SetZeroMode {
    Test,
    Clear,
}

#[derive(PartialEq, Eq, Debug)]
/// Represents an error that occurred while performing
/// an emulated instruction.
pub enum StepError {
    /// Errors related to memory access
    Memory(memory::MemoryError),
    /// Errors related to decode instructions
    Decode(decoder::DecodeError),
    /// Gameboy features that are not yet implemented in this emulator
    Unimplemented(instructions::Instruction),
}

impl From<memory::MemoryError> for StepError {
    fn from(err: memory::MemoryError) -> Self {
        StepError::Memory(err)
    }
}

impl From<decoder::DecodeError> for StepError {
    fn from(err: decoder::DecodeError) -> Self {
        StepError::Decode(err)
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
        GameBoy {
            cpu: Cpu::new(model, cartridge.target),
            dma: Default::default(),
            mem: memory::Memory::new(cartridge),
            decoder: decoder::Decoder::new(),
            runtime_decoder: Rc::new(new_instructions::RuntimeDecoder::new()),
            clocks_elapsed: 0,
        }
    }

    /// Read a value from the given memory address.
    pub fn read_memory_u8<A: Into<address::LiteralAddress>>(
        &self,
        addr: A,
    ) -> memory::MemoryResult<u8> {
        self.mem.read_u8(addr)
    }

    /// Write a value to the given memory address.
    pub fn write_memory_u8<A: Into<address::LiteralAddress>>(
        &mut self,
        addr: A,
        val: u8,
    ) -> memory::MemoryResult<()> {
        self.mem.write_u8(addr, val)
    }

    /// Read an value at the given memory address as a signed integer.
    ///
    /// This is primarily useful for reading the target of a JR instruction.
    pub fn read_memory_i8<A: Into<address::LiteralAddress>>(
        &self,
        addr: A,
    ) -> memory::MemoryResult<i8> {
        Ok(i8::from_le_bytes([self.mem.read_u8(addr)?]))
    }

    /// Read a 16-bit value from the address at `target`
    ///
    /// Note that the value is read in little endian format.
    /// This means that given `0xC000` = `0x12` and `0xC001` = `0x45`,
    /// the value read will be `0x4512`
    pub fn read_memory_u16<A: Into<address::LiteralAddress>>(
        &self,
        target: A,
    ) -> memory::MemoryResult<u16> {
        let addr = target.into();
        Ok(u16::from_le_bytes([
            self.mem.read_u8(addr)?,
            self.mem.read_u8(addr.next())?,
        ]))
    }

    /// Write a 16-bit value to the address at `target`
    ///
    /// Note that the value is written in little endian format.
    /// This means that given value of `0xABCD` and `target` of `0xC000`
    /// then `0xC000` will be set to `0xCD` and `0xC001` will be set to `0xAB`
    pub fn write_memory_u16<A: Into<address::LiteralAddress>>(
        &mut self,
        target: A,
        value: u16,
    ) -> memory::MemoryResult<()> {
        let addr = target.into();
        let bytes = value.to_le_bytes();

        self.mem.write_u8(addr, bytes[0])?;
        self.mem.write_u8(addr.next(), bytes[1])?;
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

    pub(crate) fn exec_read_register_target(&mut self, target: instructions::ByteRegisterTarget) -> StepResult<u8> {
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

    pub(crate) fn exec_write_register_target(&mut self, target: instructions::ByteRegisterTarget, value: u8) -> StepResult<()> {
        match registers::ByteRegister::try_from(target) {
            Ok(reg) => {
                self.write_register_u8(reg, value);
                Ok(())
            },
            Err(_) => {
                let addr = self.read_register_u16(wr::HL);
                self.write_memory_u8(addr, value)?;
                Ok(())
            }
        }
    }

    pub(crate) fn set_interrupt_state(&mut self, state: cpu::InterruptState) {
        self.cpu.interrupts_enabled = state;
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
        CyclingMemoryIterator {
            gb: self
        }
    }

    fn memory_iter(&self, start: address::LiteralAddress) -> memory::MemoryIterator {
        self.mem.offset_iter(start)
    }

    fn should_jump(&self, cond: instructions::Condition) -> bool {
        use instructions::Condition::*;
        match cond {
            Zero => self.cpu.read_flag(registers::Flag::Zero),
            NonZero => !self.cpu.read_flag(registers::Flag::Zero),
            Carry => self.cpu.read_flag(registers::Flag::Carry),
            NoCarry => !self.cpu.read_flag(registers::Flag::Carry),
        }
    }

    fn exec_read_instr_address(&mut self) -> StepResult<address::LiteralAddress> {
        let low = self.exec_read_inc_pc()?;
        let high = self.exec_read_inc_pc()?;
        Ok([low, high].into())
    }

    fn exec_read_inc_pc(&mut self) -> StepResult<u8> {
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

    fn exec_load(&mut self, instr: instructions::Load) -> StepResult<()> {
        use instructions::Load;
        match instr {
            Load::RegisterRegister(dest, src) => {
                let value = self.cpu.read_register_u8(src);
                self.cpu.write_register_u8(dest, value);
            }
            Load::MemoryRegister(dest, src) => {
                let value = self.cpu.read_register_u8(src);
                let target_addr = self.cpu.read_register_u16(dest);
                self.write_memory_u8(target_addr, value)?;
                self.cycle();
            }
            Load::Constant(dest, _) => {
                let val = self.exec_read_inc_pc()?;
                self.cpu.write_register_u8(dest, val);
            }
            Load::Constant16(reg, _) => {
                let first_byte = self.exec_read_inc_pc()?;
                let second_byte = self.exec_read_inc_pc()?;
                let value = u16::from_le_bytes([first_byte, second_byte]);
                self.write_register_u16(reg.into(), value);
            }
            Load::RegisterMemory(dest, src) => {
                let addr = self.cpu.read_register_u16(src);
                let value = self.read_memory_u8(addr)?;
                self.cpu.write_register_u8(dest, value);
                self.cycle();
            }
            Load::ConstantMemory(_) => {
                let val = self.exec_read_inc_pc()?;
                let addr = self.cpu.read_register_u16(wr::HL);
                self.write_memory_u8(addr, val)?;
                self.cycle();
            }
            Load::AMemoryOffset => {
                let addr = address::HighAddress(self.read_register_u8(br::C));
                let value = self.read_memory_u8(addr)?;
                self.cycle();
                self.write_register_u8(br::A, value);
            }
            Load::MemoryOffsetA => {
                let addr = address::HighAddress(self.read_register_u8(br::C));
                let value = self.read_register_u8(br::A);
                self.write_memory_u8(addr, value)?;
                self.cycle();
            }
            Load::AIndirect(_) => {
                let addr = self.exec_read_instr_address()?;
                let value = self.read_memory_u8(addr)?;
                self.cycle();
                self.write_register_u8(br::A, value);
            }
            Load::IndirectA(_) => {
                let addr = self.exec_read_instr_address()?;
                let value = self.read_register_u8(br::A);
                self.write_memory_u8(addr, value)?;
                self.cycle();
            }
            Load::AHighOffset(_) => {
                let offset = self.exec_read_inc_pc()?;
                let addr = address::HighAddress(offset);
                let value = self.read_memory_u8(addr)?;
                self.cycle();
                self.write_register_u8(br::A, value);
            }
            Load::HighOffsetA(_) => {
                let offset = self.exec_read_inc_pc()?;
                let addr = address::HighAddress(offset);
                let value = self.read_register_u8(br::A);
                self.write_memory_u8(addr, value)?;
                self.cycle();
            }
            Load::Increment16A(inc) => {
                let addr = self.read_register_u16(wr::HL);
                self.write_memory_u8(addr, self.read_register_u8(br::A))?;
                let new_addr = match inc {
                    instructions::Increment::Increment => addr.wrapping_add(1),
                    instructions::Increment::Decrement => addr.wrapping_sub(1),
                };
                self.write_register_u16(wr::HL, new_addr);
                self.cycle();
            }
            Load::AIncrement16(inc) => {
                let addr = self.read_register_u16(wr::HL);
                let value = self.read_memory_u8(addr)?;
                let new_addr = match inc {
                    instructions::Increment::Increment => addr.wrapping_add(1),
                    instructions::Increment::Decrement => addr.wrapping_sub(1),
                };
                self.write_register_u8(br::A, value);
                self.write_register_u16(wr::HL, new_addr);
                self.cycle();
            }
        }

        Ok(())
    }

    fn set_pc<A: Into<address::LiteralAddress>>(&mut self, target: A) {
        let address::LiteralAddress(addr) = target.into();
        self.cpu.write_register_u16(wr::PC, addr);
    }

    fn read_pc(&self) -> address::LiteralAddress {
        let value = self.cpu.read_register_u16(wr::PC);
        address::LiteralAddress(value)
    }

    fn exec_jump(&mut self, instr: instructions::Jump) -> StepResult<()> {
        use instructions::Jump;
        match instr {
            Jump::Jump(_) => {
                let addr = self.exec_read_instr_address()?;
                self.set_pc(addr);
                self.cycle();
                Ok(())
            }
            Jump::JumpIf(cond, _) => {
                let addr = self.exec_read_instr_address()?;
                if self.should_jump(cond) {
                    self.set_pc(addr);
                    self.cycle();
                }
                Ok(())
            }
            Jump::RegisterJump => {
                let addr = self.cpu.read_register_u16(wr::HL);
                self.set_pc(addr);
                Ok(())
            }
            Jump::RelativeJump(_) => {
                let offset = i8::from_le_bytes([self.exec_read_inc_pc()?]);
                let pc = self.cpu.read_register_u16(wr::PC);
                let new_pc = if offset > 0 {
                    pc.wrapping_add(u16::try_from(offset).unwrap())
                } else {
                    pc.wrapping_sub(u16::try_from(offset.abs()).unwrap())
                };
                self.cycle();
                self.set_pc(new_pc);
                Ok(())
            }
            Jump::RelativeJumpIf(cond, _) => {
                let offset = i8::from_le_bytes([self.exec_read_inc_pc()?]);
                let pc = self.cpu.read_register_u16(wr::PC);
                if self.should_jump(cond) {
                    let new_pc = if offset > 0 {
                        pc.wrapping_add(u16::try_from(offset).unwrap())
                    } else {
                        pc.wrapping_sub(u16::try_from(offset.abs()).unwrap())
                    };
                    self.cycle();
                    self.set_pc(new_pc);
                }
                Ok(())
            }
            Jump::Call(_) => {
                let addr = self.exec_read_instr_address()?;
                self.exec_push(self.read_pc())?;
                self.set_pc(addr);
                Ok(())
            }
            Jump::CallIf(cond, _) => {
                let addr = self.exec_read_instr_address()?;
                if self.should_jump(cond) {
                    self.exec_push(self.read_pc())?;
                    self.set_pc(addr);
                }
                Ok(())
            }
            Jump::CallSystem(addr) => {
                self.exec_push(self.read_pc())?;
                self.set_pc(addr);
                Ok(())
            }
            Jump::Return => {
                let return_addr: address::LiteralAddress = self.exec_pop()?;
                self.set_pc(return_addr);
                self.cycle();
                Ok(())
            }
            Jump::ReturnIf(cond) => {
                if self.should_jump(cond) {
                    let return_addr: address::LiteralAddress = self.exec_pop()?;
                    self.set_pc(return_addr);
                    self.cycle();
                }
                self.cycle();
                Ok(())
            }
            Jump::ReturnInterrupt => {
                let return_addr: address::LiteralAddress = self.exec_pop()?;
                self.set_pc(return_addr);
                self.cpu.interrupts_enabled = cpu::InterruptState::Enabled;
                self.cycle();
                Ok(())
            }
        }
    }

    fn exec_rotate(
        &mut self,
        dir: instructions::RotateDirection,
        carry: instructions::Carry,
        reg: registers::ByteRegister,
        set_zero: SetZeroMode,
    ) -> StepResult<()> {
        let current_value = self.cpu.read_register_u8(reg);
        let high_byte = if carry != instructions::Carry::Carry {
            if self.cpu.read_flag(registers::Flag::Carry) {
                0x81
            } else {
                0x00
            }
        } else {
            current_value
        };
        let value_to_rotate = u16::from_le_bytes([current_value, high_byte]);
        let (rotated, carry) = if dir == instructions::RotateDirection::Left {
            let rotated = value_to_rotate.rotate_left(1);
            (rotated.to_le_bytes()[0], (rotated & 0x0100) != 0)
        } else {
            let rotated = value_to_rotate.rotate_right(1);
            (rotated.to_le_bytes()[0], (rotated & 0x8000) != 0)
        };
        self.cpu.write_register_u8(reg, rotated);
        self.cpu.set_flag_to(registers::Flag::Carry, carry);
        let zero_flag = match set_zero {
            SetZeroMode::Test => rotated == 0,
            SetZeroMode::Clear => false,
        };
        self.cpu.set_flag_to(registers::Flag::Zero, zero_flag);
        self.cpu.reset_flag(registers::Flag::HalfCarry);
        self.cpu.reset_flag(registers::Flag::AddSubtract);
        Ok(())
    }

    fn exec_extended(&mut self, _instr: instructions::Extended) -> StepResult<()> {
        use decoder::{idecoders, TwoByteDataDecoder};
        use instructions::Carry::Carry;
        use instructions::Extended;
        use instructions::RotateDirection::{Left, Right};
        let data_byte = self.exec_read_inc_pc()?;
        let actual_instruction = idecoders::Extended.decode(0xCB, data_byte).unwrap();
        let ext = match actual_instruction {
            instructions::Instruction::Extended(ex) => ex,
            _ => unreachable!(),
        };
        match ext {
            Extended::SetBit(bit, reg) => {
                let val = self.cpu.read_register_u8(reg);
                let new_val = val | (1 << bit);
                self.cpu.write_register_u8(reg, new_val);
                Ok(())
            }
            Extended::SetMemoryBit(bit) => {
                let addr = self.cpu.read_register_u16(wr::HL);
                let val = self.read_memory_u8(addr)?;
                self.cycle();
                let new_val = val | (1 << bit);
                self.write_memory_u8(addr, new_val)?;
                self.cycle();
                Ok(())
            }
            Extended::ResetBit(bit, reg) => {
                let val = self.cpu.read_register_u8(reg);
                let new_val = val & !(1 << bit);
                self.cpu.write_register_u8(reg, new_val);
                Ok(())
            }
            Extended::ResetMemoryBit(bit) => {
                let addr = self.cpu.read_register_u16(wr::HL);
                let val = self.read_memory_u8(addr)?;
                self.cycle();
                let new_val = val & !(1 << bit);
                self.write_memory_u8(addr, new_val)?;
                self.cycle();
                Ok(())
            }
            Extended::TestBit(bit, reg) => {
                let val = self.cpu.read_register_u8(reg);
                let bit_test = val & (1 << bit);
                self.cpu.set_flag_to(registers::Flag::Zero, bit_test == 0);
                self.cpu.set_flag(registers::Flag::HalfCarry);
                self.cpu.set_flag_to(registers::Flag::AddSubtract, false);
                Ok(())
            }
            Extended::TestMemoryBit(bit) => {
                let addr = self.cpu.read_register_u16(wr::HL);
                let val = self.read_memory_u8(addr)?;
                self.cycle();
                let bit_test = val & (1 << bit);
                self.cpu.set_flag_to(registers::Flag::Zero, bit_test == 0);
                self.cpu.set_flag(registers::Flag::HalfCarry);
                self.cpu.set_flag_to(registers::Flag::AddSubtract, false);

                Ok(())
            }
            Extended::Rotate(dir, carry, reg) => {
                self.exec_rotate(dir, carry, reg, SetZeroMode::Test)
            }
            Extended::RotateMemory(dir, carry) => {
                let addr = self.cpu.read_register_u16(wr::HL);
                let current_value = self.read_memory_u8(addr)?;
                self.cycle();
                let high_byte = if carry != Carry {
                    if self.cpu.read_flag(registers::Flag::Carry) {
                        0x81
                    } else {
                        0x00
                    }
                } else {
                    current_value
                };
                let value_to_rotate = u16::from_le_bytes([current_value, high_byte]);
                let (rotated, carry) = if dir == Left {
                    let rotated = value_to_rotate.rotate_left(1);
                    (rotated.to_le_bytes()[0], (rotated & 0x0100) != 0)
                } else {
                    let rotated = value_to_rotate.rotate_right(1);
                    (rotated.to_le_bytes()[0], (rotated & 0x8000) != 0)
                };
                self.write_memory_u8(addr, rotated)?;
                self.cpu.set_flag_to(registers::Flag::Carry, carry);
                self.cpu.set_flag_to(registers::Flag::Zero, rotated == 0);
                self.cpu.reset_flag(registers::Flag::HalfCarry);
                self.cpu.reset_flag(registers::Flag::AddSubtract);
                self.cycle();
                Ok(())
            }
            Extended::Swap(reg) => {
                let value = self.cpu.read_register_u8(reg);
                let low_nibble = value & 0x0F;
                let high_nibble = value & 0xF0;
                let new_value = (low_nibble.rotate_left(4)) + (high_nibble.rotate_right(4));
                self.cpu.write_register_u8(reg, new_value);
                Ok(())
            }
            Extended::SwapMemory => {
                let addr = self.cpu.read_register_u16(registers::WordRegister::HL);
                let value = self.read_memory_u8(addr)?;
                let low_nibble = value & 0x0F;
                let high_nibble = value & 0xF0;
                let new_value = (low_nibble.rotate_left(4)) + (high_nibble.rotate_right(4));
                self.cycle();
                self.write_memory_u8(addr, new_value)?;
                self.cycle();
                Ok(())
            }
            Extended::ShiftZero(dir, reg) => {
                let value = self.cpu.read_register_u8(reg);
                let (shifted_value, carry) = match dir {
                    Left => (value << 1, value & 0x80 != 0),
                    Right => (value >> 1, value & 0x01 != 0),
                };
                self.cpu.set_flag_to(registers::Flag::Carry, carry);
                self.cpu.write_register_u8(reg, shifted_value);
                Ok(())
            }
            Extended::ShiftRightExtend(reg) => {
                let value = self.cpu.read_register_u8(reg);
                let value16 = u16::from(value);
                let extra_bit = (value16 << 1) & 0xff00;
                let shifted_value = (extra_bit + value16) >> 1;
                let actual_byte = shifted_value.to_le_bytes()[0];
                self.cpu.write_register_u8(reg, actual_byte);
                Ok(())
            }
            Extended::ShiftMemoryRightExtend => {
                let addr = self.cpu.read_register_u16(registers::WordRegister::HL);
                let value = self.read_memory_u8(addr)?;
                self.cycle();
                let value16 = u16::from(value);
                let extra_bit = (value16 << 1) & 0xff00;
                let shifted_value = (extra_bit + value16) >> 1;
                let actual_byte = shifted_value.to_le_bytes()[0];
                self.write_memory_u8(addr, actual_byte)?;
                self.cycle();
                Ok(())
            }
            Extended::ShiftMemoryZero(dir) => {
                let addr = self.cpu.read_register_u16(registers::WordRegister::HL);
                let value = self.read_memory_u8(addr)?;
                let (shifted_value, carry) = match dir {
                    Left => (value << 1, value & 0x80 != 0),
                    Right => (value >> 1, value & 0x01 != 0),
                };
                self.cpu.set_flag_to(registers::Flag::Carry, carry);
                self.cycle();
                self.write_memory_u8(addr, shifted_value)?;
                self.cycle();
                Ok(())
            }
        }
    }

    fn exec(&mut self, instr: instructions::Instruction) -> StepResult<()> {
        use instructions::Instruction;
        match instr {
            Instruction::Jump(j) => self.exec_jump(j),
            Instruction::RegisterAL(_) => panic!("ALU instructions should be in the new handler"),
            Instruction::MemoryAL(_) => panic!("ALU instructions should be in the new handler"),
            Instruction::ConstantAL(_, _) => panic!("ALU instructions should be in the new handler"),
            Instruction::MemoryIncrement(_) => panic!("ALU instructions should be in the new handler"),
            Instruction::Stack(_) => panic!("Stack instructions should be in the new handler"),
            Instruction::Load(l) => self.exec_load(l),
            Instruction::Extended(ex) => self.exec_extended(ex),
            Instruction::Literal(_) => Err(StepError::Unimplemented(instr)),
            Instruction::NOP  
            | Instruction::InvertCarry 
            | Instruction::SetCarry 
            | Instruction::InvertA 
            | Instruction::AToBCD
            | Instruction::EnableInterrupts  
            | Instruction::DisableInterrupts => panic!("Misc instructions should be in the new handler"),
            Instruction::Rotate(dir, carry) => {
                self.exec_rotate(dir, carry, br::A, SetZeroMode::Clear)
            }
            Instruction::Stop => Err(StepError::Unimplemented(instr)),
            Instruction::Halt => Err(StepError::Unimplemented(instr)),
        }
    }

    fn check_interrupts(&mut self) -> StepResult<bool> {
        use cpu::InterruptState::{Disabled, Enabled, Pending};
        match self.cpu.interrupts_enabled {
            Pending => {
                self.cpu.interrupts_enabled = Enabled;
                Ok(false)
            }
            Disabled => Ok(false),
            Enabled => {
                let itest = cpu::Interrupt::test(self.mem.registers.ie, self.mem.registers.iflag);
                if let Some(interrupt) = itest {
                    self.cycle();
                    self.cycle();
                    self.cpu.interrupts_enabled = Disabled;
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
        let pc_value = self.read_pc();
        let opcode = self.read_memory_u8(pc_value)?;
        if self.runtime_decoder.has_opcode(opcode) {
            self.cycle();
            let interrupted = self.check_interrupts()?;
            if !interrupted {
                self.set_pc(pc_value.next());
                let non_borrowing_decoder = self.runtime_decoder.clone();
                non_borrowing_decoder.decode(opcode)
                    .unwrap()
                    .to_executable(&mut self.cycling_memory_iter())
                    .execute(self)?;
            }
        } else {        
            let instruction = self.current_instruction()?;
            self.cycle();
            let interrupted = self.check_interrupts()?;
            if !interrupted {
                self.set_pc(pc_value.next());
                self.exec(instruction)?;
            }
        }
        Ok(())
    }

    /// Returns the instruction at the current PC.
    pub fn current_instruction(&self) -> StepResult<instructions::Instruction> {
        let pc_value = self.read_pc();
        let instruction_value = self.read_memory_u8(pc_value)?;
        let instruction = self
            .decoder
            .decode(instruction_value, &mut self.memory_iter(pc_value.next()))?;
        Ok(instruction)
    }

    pub(crate) fn cycle(&mut self) {
        // TODO: Use this. a memory error can occur if the DMA operation tries to
        // write to cartridge RAM that is not present. As with actual hardware,
        // the DMA operation continues, and so we shouldn't abort emulation early,
        // but it would be useful to surface this information somewhere for ROM developers.
        let _dma_result = self.dma.run_cycle(&mut self.mem);
        self.clocks_elapsed += 4;
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
}

pub(crate) struct CyclingMemoryIterator<'a> {
    gb: &'a mut GameBoy
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
mod test {
    use super::*;
    use crate::gameboy::memory;

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

        gb.write_memory_u16(0xc100, 0x1032)?;
        assert_eq!(gb.read_memory_u16(0xc100), Ok(0x1032));
        Ok(())
    }

    #[test]
    fn test_mem_write_u8_read_u16_sysram() -> memory::MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_memory_u8(0xc100, 0x48)?;
        gb.write_memory_u8(0xc101, 0x94)?;

        assert_eq!(gb.read_memory_u16(0xc100), Ok(0x9448));
        Ok(())
    }

    #[test]
    fn test_mem_write_u16_read_u8_sysram() -> memory::MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_memory_u16(0xc200, 0x1345)?;

        assert_eq!(gb.read_memory_u8(0xc200), Ok(0x45));
        assert_eq!(gb.read_memory_u8(0xc201), Ok(0x13));
        Ok(())
    }

    #[test]
    fn test_write_u8_read_i8_sysram() -> memory::MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);
        let signed_value = i8::from_le_bytes([0xa2]);

        gb.write_memory_u8(0xc200, 0xa2)?;

        assert_eq!(gb.read_memory_i8(0xc200), Ok(signed_value));
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
    fn test_convert_decode_error() {
        let error = decoder::DecodeError::IncompleteInstruction;
        let mapped_error: StepError = error.into();

        assert_eq!(
            mapped_error,
            StepError::Decode(decoder::DecodeError::IncompleteInstruction)
        );
    }
}
