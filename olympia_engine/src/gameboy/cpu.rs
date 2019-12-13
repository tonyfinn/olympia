use crate::decoder;
use crate::instructions;
use crate::registers;
use crate::rom;

use core::convert::TryFrom;

pub use crate::registers::{ByteRegister, WordRegister};

pub struct GameBoy {
    af_register: u16,
    bc_register: u16,
    de_register: u16,
    hl_register: u16,
    sp_register: u16,
    pc_register: u16,
    sysram: [u8; 0x2000],
    vram: [u8; 0x2000],
    cpuram: [u8; 0x200],
    cartridge: rom::Cartridge,
    decoder: decoder::Decoder,
    clocks_elapsed: u64,
}

#[derive(PartialEq, Eq, Debug)]
pub enum MemoryError {
    InvalidRomAddress(u16),
    InvalidRamAddress(u16),
}

pub type MemoryResult<T> = Result<T, MemoryError>;

#[derive(PartialEq, Eq, Debug)]
pub enum StepError {
    Memory(MemoryError),
    Decode(decoder::DecodeError),
    Unimplemented(instructions::Instruction),
}

impl From<MemoryError> for StepError {
    fn from(err: MemoryError) -> Self {
        StepError::Memory(err)
    }
}

impl From<decoder::DecodeError> for StepError {
    fn from(err: decoder::DecodeError) -> Self {
        StepError::Decode(err)
    }
}

pub type StepResult<T> = Result<T, StepError>;

struct MemoryIterator<'a> {
    addr: u16,
    gb: &'a GameBoy,
}

impl<'a> Iterator for MemoryIterator<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let val = self.gb.read_memory_u8(self.addr);
        self.addr += 1;
        Some(val.unwrap_or(0))
    }
}

impl GameBoy {
    pub fn new(cartridge: rom::Cartridge, model: super::GameBoyModel) -> GameBoy {
        GameBoy {
            af_register: model.default_af(),
            bc_register: model.default_bc(),
            de_register: model.default_de(cartridge.target),
            hl_register: model.default_hl(cartridge.target),
            sp_register: 0xfffe,
            pc_register: 0x100,
            sysram: [0u8; 0x2000],
            vram: [0u8; 0x2000],
            cpuram: [0u8; 0x200],
            cartridge,
            decoder: decoder::Decoder::new(),
            clocks_elapsed: 0,
        }
    }

    pub fn read_register_u8(&self, reg: registers::ByteRegister) -> u8 {
        match reg {
            registers::ByteRegister::A => self.af_register.to_le_bytes()[1],
            registers::ByteRegister::F => self.af_register.to_le_bytes()[0],
            registers::ByteRegister::B => self.bc_register.to_le_bytes()[1],
            registers::ByteRegister::C => self.bc_register.to_le_bytes()[0],
            registers::ByteRegister::D => self.de_register.to_le_bytes()[1],
            registers::ByteRegister::E => self.de_register.to_le_bytes()[0],
            registers::ByteRegister::H => self.hl_register.to_le_bytes()[1],
            registers::ByteRegister::L => self.hl_register.to_le_bytes()[0],
        }
    }

    pub fn read_register_u16(&self, reg: registers::WordRegister) -> u16 {
        match reg {
            registers::WordRegister::AF => self.af_register.to_le(),
            registers::WordRegister::BC => self.bc_register.to_le(),
            registers::WordRegister::DE => self.de_register.to_le(),
            registers::WordRegister::HL => self.hl_register.to_le(),
            registers::WordRegister::SP => self.sp_register.to_le(),
            registers::WordRegister::PC => self.pc_register.to_le(),
        }
    }

    pub fn write_register_u8(&mut self, reg: registers::ByteRegister, value: u8) {
        let full_register = reg.lookup_word_register();
        let byte = reg.lookup_byte();

        match byte {
            registers::WordByte::High => self.write_high_register_byte(full_register, value),
            registers::WordByte::Low => self.write_low_register_byte(full_register, value),
        }
    }

    fn write_high_register_byte(
        &mut self,
        full_register: registers::WordRegister,
        value_to_write: u8,
    ) {
        let register_value = self.read_register_u16(full_register);
        let u16_value = u16::from(value_to_write);
        let masked_register_value = register_value & 0x00ff;
        let shifted_value = u16_value << 8;
        self.write_register_raw(full_register, masked_register_value + shifted_value);
    }

    fn write_low_register_byte(
        &mut self,
        full_register: registers::WordRegister,
        value_to_write: u8,
    ) {
        let register_value = self.read_register_u16(full_register);
        let u16_value = u16::from(value_to_write);
        let masked_register_value = register_value & 0xff00;
        self.write_register_raw(full_register, masked_register_value + u16_value);
    }

    fn write_register_raw(&mut self, reg: registers::WordRegister, value: u16) {
        match reg {
            registers::WordRegister::AF => self.af_register = value & 0xfff0,
            registers::WordRegister::BC => self.bc_register = value,
            registers::WordRegister::DE => self.de_register = value,
            registers::WordRegister::HL => self.hl_register = value,
            registers::WordRegister::SP => self.sp_register = value,
            registers::WordRegister::PC => self.pc_register = value,
        }
    }

    pub fn write_register_u16(&mut self, reg: registers::WordRegister, value: u16) {
        self.write_register_raw(reg, value.to_le());
    }

    pub fn read_memory_u8(&self, addr: u16) -> MemoryResult<u8> {
        if addr < 0x8000 {
            self.cartridge
                .read(addr)
                .map_err(|_| MemoryError::InvalidRomAddress(addr))
        } else if addr <= 0x9fff {
            Ok(self.vram[(addr - 0x8000) as usize])
        } else if addr <= 0xbfff {
            self.cartridge
                .read(addr)
                .map_err(|_| MemoryError::InvalidRamAddress(addr))
        } else if addr <= 0xdfff {
            Ok(self.sysram[(addr - 0xc000) as usize])
        } else if addr <= 0xfdff {
            Ok(self.sysram[(addr - 0xe000) as usize])
        } else {
            Ok(self.cpuram[(addr - 0xfe00) as usize])
        }
    }

    pub fn read_memory_i8(&self, addr: u16) -> MemoryResult<i8> {
        Ok(i8::from_le_bytes([self.read_memory_u8(addr)?]))
    }

    pub fn read_memory_u16(&self, addr: u16) -> MemoryResult<u16> {
        Ok(u16::from_le_bytes([
            self.read_memory_u8(addr)?,
            self.read_memory_u8(addr.wrapping_add(1))?,
        ]))
    }

    pub fn write_memory_u8(&mut self, addr: u16, value: u8) -> MemoryResult<()> {
        if addr < 0x8000 {
            self.cartridge
                .write(addr, value)
                .map_err(|_| MemoryError::InvalidRomAddress(addr))
        } else if addr <= 0x9fff {
            self.vram[(addr - 0x8000) as usize] = value;
            Ok(())
        } else if addr <= 0xbfff {
            self.cartridge
                .write(addr, value)
                .map_err(|_| MemoryError::InvalidRamAddress(addr))
        } else if addr <= 0xdfff {
            self.sysram[(addr - 0xc000) as usize] = value;
            Ok(())
        } else if addr <= 0xfdff {
            self.sysram[(addr - 0xe000) as usize] = value;
            Ok(())
        } else {
            self.cpuram[(addr - 0xfe00) as usize] = value;
            Ok(())
        }
    }

    pub fn write_memory_u16(&mut self, addr: u16, value: u16) -> MemoryResult<()> {
        let bytes = value.to_le_bytes();

        self.write_memory_u8(addr, bytes[0])?;
        self.write_memory_u8(addr.wrapping_add(1), bytes[1])?;
        Ok(())
    }

    fn memory_iter(&self, start: u16) -> MemoryIterator {
        MemoryIterator {
            addr: start,
            gb: &self,
        }
    }

    fn read_flag(&self, flag: registers::Flag) -> bool {
        self.af_register & (1u16 << flag.bit()) != 0
    }

    fn set_flag_to(&mut self, flag: registers::Flag, value: bool) {
        if value {
            self.set_flag(flag)
        } else {
            self.reset_flag(flag)
        }
    }

    fn set_flag(&mut self, flag: registers::Flag) {
        self.af_register |= 1 << flag.bit();
    }

    fn reset_flag(&mut self, flag: registers::Flag) {
        self.af_register &= !(1u16 << flag.bit());
    }

    fn invert_flag(&mut self, flag: registers::Flag) {
        self.af_register ^= 1u16 << flag.bit();
    }

    fn should_jump(&self, cond: instructions::Condition) -> bool {
        use instructions::Condition::*;
        match cond {
            Zero => self.read_flag(registers::Flag::Zero),
            NonZero => !self.read_flag(registers::Flag::Zero),
            Carry => self.read_flag(registers::Flag::Carry),
            NoCarry => !self.read_flag(registers::Flag::Carry),
        }
    }

    fn exec_read_inc_pc(&mut self) -> StepResult<u8> {
        let val = self.read_memory_u8(self.pc_register)?;
        self.pc_register = self.pc_register.wrapping_add(1);
        self.cycle();
        Ok(val)
    }

    fn exec_push(&mut self, value: u16) -> StepResult<()> {
        let stack_addr = self.read_register_u16(registers::WordRegister::SP);
        let [low, high] = value.to_le_bytes();
        let stack_addr = stack_addr.wrapping_sub(1);
        self.write_memory_u8(stack_addr, high)?;
        self.cycle();
        let stack_addr = stack_addr.wrapping_sub(1);
        self.write_memory_u8(stack_addr, low)?;
        self.cycle();
        self.write_register_u16(registers::WordRegister::SP, stack_addr);
        self.cycle();
        Ok(())
    }

    fn exec_pop(&mut self) -> StepResult<u16> {
        let stack_addr = self.read_register_u16(registers::WordRegister::SP);
        let low = self.read_memory_u8(stack_addr)?;
        self.cycle();
        let stack_addr = stack_addr.wrapping_add(1);
        let high = self.read_memory_u8(stack_addr)?;
        self.cycle();
        self.sp_register = stack_addr.wrapping_add(1);
        Ok(u16::from_le_bytes([low, high]))
    }

    fn exec_load(&mut self, instr: instructions::Load) -> StepResult<()> {
        use instructions::Load;
        match instr {
            Load::RegisterRegister(dest, src) => {
                let value = self.read_register_u8(src);
                self.write_register_u8(dest, value);
                self.cycle();
            }
            Load::MemoryRegister(dest, src) => {
                let value = self.read_register_u8(src);
                let target_addr = self.read_register_u16(dest);
                self.write_memory_u8(target_addr, value)?;
                self.cycle();
                self.cycle();
            }
            Load::Constant(dest, _) => {
                let val = self.exec_read_inc_pc()?;
                self.write_register_u8(dest, val);
                self.cycle();
            }
            Load::RegisterMemory(dest, src) => {
                let addr = self.read_register_u16(src);
                let value = self.read_memory_u8(addr)?;
                self.cycle();
                self.write_register_u8(dest, value);
                self.cycle();
            }
            Load::ConstantMemory(_) => {
                let val = self.exec_read_inc_pc()?;
                let addr = self.hl_register;
                self.write_memory_u8(addr, val)?;
                self.cycle();
                self.cycle();
            }
            _ => return Err(StepError::Unimplemented(instr.into())),
        }

        Ok(())
    }

    fn exec_al(&mut self, op: instructions::ALOp, arg: u8) -> u8 {
        let current_value = self.read_register_u8(registers::ByteRegister::A);
        use instructions::ALOp;
        match op {
            ALOp::Add => {
                let (new, overflow) = current_value.overflowing_add(arg);
                self.set_flag_to(registers::Flag::Carry, overflow);
                self.reset_flag(registers::Flag::AddSubtract);
                self.set_flag_to(registers::Flag::Zero, new == 0);
                self.set_add_half_carry(current_value, arg);
                new
            }
            ALOp::AddCarry => {
                let carry_bit = u8::from(self.read_flag(registers::Flag::Carry));
                let (tmp, overflow) = current_value.overflowing_add(arg);
                let (new, overflow_carry) = tmp.overflowing_add(carry_bit);
                self.set_flag_to(registers::Flag::Carry, overflow | overflow_carry);
                self.reset_flag(registers::Flag::AddSubtract);
                self.set_flag_to(registers::Flag::Zero, new == 0);
                self.set_add_half_carry(current_value, arg + carry_bit);
                new
            }
            ALOp::Sub => {
                let (new, overflow) = current_value.overflowing_sub(arg);
                self.set_flag_to(registers::Flag::Carry, overflow);
                self.set_flag(registers::Flag::AddSubtract);
                self.set_flag_to(registers::Flag::Zero, new == 0);
                self.set_sub_half_carry(current_value, arg);
                new
            }
            ALOp::SubCarry => {
                let carry_bit = u8::from(self.read_flag(registers::Flag::Carry));
                let (tmp, overflow) = current_value.overflowing_sub(arg);
                let (new, overflow_carry) = tmp.overflowing_sub(carry_bit);
                self.set_flag_to(registers::Flag::Carry, overflow | overflow_carry);
                self.set_flag(registers::Flag::AddSubtract);
                self.set_flag_to(registers::Flag::Zero, new == 0);
                self.set_sub_half_carry(current_value, arg + carry_bit);
                new
            }
            ALOp::Compare => {
                let (new, overflow) = current_value.overflowing_sub(arg);
                self.set_flag_to(registers::Flag::Carry, overflow);
                self.set_flag(registers::Flag::AddSubtract);
                self.set_flag_to(registers::Flag::Zero, new == 0);
                self.set_sub_half_carry(current_value, arg);
                current_value
            }
            ALOp::And => {
                let new = current_value & arg;
                self.reset_flag(registers::Flag::Carry);
                self.set_flag(registers::Flag::HalfCarry);
                self.reset_flag(registers::Flag::AddSubtract);
                self.set_flag_to(registers::Flag::Zero, new == 0);
                new
            }
            ALOp::Or => {
                let new = current_value | arg;
                self.reset_flag(registers::Flag::Carry);
                self.reset_flag(registers::Flag::HalfCarry);
                self.reset_flag(registers::Flag::AddSubtract);
                self.set_flag_to(registers::Flag::Zero, new == 0);
                new
            }
            ALOp::Xor => {
                let new = current_value ^ arg;
                self.reset_flag(registers::Flag::Carry);
                self.reset_flag(registers::Flag::HalfCarry);
                self.reset_flag(registers::Flag::AddSubtract);
                self.set_flag_to(registers::Flag::Zero, new == 0);
                new
            }
        }
    }

    fn set_add_half_carry(&mut self, a: u8, b: u8) {
        let half_add = ((a & 0xF) + (b & 0xF)) & 0xF0;
        self.set_flag_to(registers::Flag::HalfCarry, half_add != 0);
    }

    fn set_sub_half_carry(&mut self, a: u8, b: u8) {
        let sub = (a & 0x1F).wrapping_sub(b & 0x0F);
        let half_carry = (sub & 0x10) != a & 0x10;
        self.set_flag_to(registers::Flag::HalfCarry, half_carry);
    }

    fn exec_register_al(&mut self, instr: instructions::RegisterAL) -> StepResult<()> {
        use instructions::RegisterAL;
        match instr {
            RegisterAL::ByteOp(op, reg) => {
                let reg_value = self.read_register_u8(reg);
                let new_value = self.exec_al(op, reg_value);
                self.write_register_u8(registers::ByteRegister::A, new_value);
            }
            RegisterAL::Increment(reg) => {
                let reg_value = self.read_register_u8(reg);
                let (new, carry) = reg_value.overflowing_add(1);
                self.set_flag_to(registers::Flag::Zero, new == 0);
                self.set_flag_to(registers::Flag::Carry, carry);
                self.reset_flag(registers::Flag::AddSubtract);
                self.set_add_half_carry(reg_value, 1);
                self.write_register_u8(reg, new);
            }
            RegisterAL::Decrement(reg) => {
                let reg_value = self.read_register_u8(reg);
                let (new, carry) = reg_value.overflowing_sub(1);
                self.set_flag_to(registers::Flag::Zero, new == 0);
                self.set_flag_to(registers::Flag::Carry, carry);
                self.set_flag(registers::Flag::AddSubtract);
                self.set_sub_half_carry(reg_value, 1);
                self.write_register_u8(reg, new);
            }
            RegisterAL::Decrement16(reg) => {
                let reg_value = self.read_register_u16(reg.into());
                let (new, _carry) = reg_value.overflowing_sub(1);
                self.write_register_u16(reg.into(), new);
                self.cycle();
            }
            RegisterAL::Increment16(reg) => {
                let reg_value = self.read_register_u16(reg.into());
                let (new, _carry) = reg_value.overflowing_add(1);
                self.write_register_u16(reg.into(), new);
                self.cycle();
            }
            RegisterAL::Add16(reg) => {
                let current_value = self.read_register_u16(registers::WordRegister::HL);
                let value_to_add = self.read_register_u16(reg.into());
                let (new, carry) = current_value.overflowing_add(value_to_add);
                self.set_flag_to(registers::Flag::Zero, new == 0);
                self.set_flag_to(registers::Flag::Carry, carry);
                self.reset_flag(registers::Flag::AddSubtract);
                let has_half_carry =
                    (((current_value & 0x0FFF) + (value_to_add & 0x0FFF)) & 0xF000) != 0;
                self.set_flag_to(registers::Flag::HalfCarry, has_half_carry);
                self.write_register_u16(registers::WordRegister::HL, new);
                self.cycle();
            }
        };
        self.cycle();
        Ok(())
    }

    fn exec_constant_al(&mut self, op: instructions::ALOp) -> StepResult<()> {
        let arg = self.exec_read_inc_pc()?;
        let new_value = self.exec_al(op, arg);
        self.write_register_u8(registers::ByteRegister::A, new_value);
        self.cycle();
        Ok(())
    }

    fn exec_jump(&mut self, instr: instructions::Jump) -> StepResult<()> {
        use instructions::Jump;
        match instr {
            Jump::Jump(_) => {
                let addr = u16::from_le_bytes([self.exec_read_inc_pc()?, self.exec_read_inc_pc()?]);
                self.pc_register = addr;
                self.cycle();
                self.cycle();
                Ok(())
            }
            Jump::JumpIf(cond, _) => {
                let addr = u16::from_le_bytes([self.exec_read_inc_pc()?, self.exec_read_inc_pc()?]);
                if self.should_jump(cond) {
                    self.pc_register = addr;
                    self.cycle();
                }
                self.cycle();
                Ok(())
            }
            Jump::RegisterJump => {
                let addr = self.hl_register;
                self.pc_register = addr;
                self.cycle();
                Ok(())
            }
            Jump::RelativeJump(_) => {
                let offset = i8::from_le_bytes([self.exec_read_inc_pc()?]);
                let pc = self.pc_register;
                let new_pc = if offset > 0 {
                    pc.wrapping_add(u16::try_from(offset).unwrap())
                } else {
                    pc.wrapping_sub(u16::try_from(offset.abs()).unwrap())
                };
                self.cycle();
                self.pc_register = new_pc;
                self.cycle();
                Ok(())
            }
            Jump::RelativeJumpIf(cond, _) => {
                let offset = i8::from_le_bytes([self.exec_read_inc_pc()?]);
                let pc = self.pc_register;
                if self.should_jump(cond) {
                    let new_pc = if offset > 0 {
                        pc.wrapping_add(u16::try_from(offset).unwrap())
                    } else {
                        pc.wrapping_sub(u16::try_from(offset.abs()).unwrap())
                    };
                    self.cycle();
                    self.pc_register = new_pc;
                }
                self.cycle();
                Ok(())
            }
            Jump::Call(_) => {
                let low = self.exec_read_inc_pc()?;
                let high = self.exec_read_inc_pc()?;
                let addr = u16::from_le_bytes([low, high]);
                self.exec_push(self.pc_register)?;
                self.pc_register = addr;
                self.cycle();
                Ok(())
            }
            Jump::CallIf(cond, _) => {
                let low = self.exec_read_inc_pc()?;
                let high = self.exec_read_inc_pc()?;
                let addr = u16::from_le_bytes([low, high]);
                if self.should_jump(cond) {
                    self.exec_push(self.pc_register)?;
                    self.pc_register = addr;
                }
                self.cycle();
                Ok(())
            }
            Jump::CallSystem(addr_literal) => {
                let crate::types::LiteralAddress(addr) = addr_literal;
                self.exec_push(self.pc_register)?;
                self.pc_register = addr;
                self.cycle();
                Ok(())
            }
            Jump::Return => {
                let return_addr = self.exec_pop()?;
                self.pc_register = return_addr;
                self.cycle();
                self.cycle();
                Ok(())
            }
            Jump::ReturnIf(cond) => {
                if self.should_jump(cond) {
                    let return_addr = self.exec_pop()?;
                    self.pc_register = return_addr;
                    self.cycle();
                }
                self.cycle();
                self.cycle();
                Ok(())
            }
            Jump::ReturnInterrupt => Err(StepError::Unimplemented(instr.into())),
        }
    }

    fn exec_stack(&mut self, instr: instructions::Stack) -> StepResult<()> {
        use instructions::Stack;
        match instr {
            Stack::Push(reg) => {
                let value = self.read_register_u16(reg.into());
                self.exec_push(value)?;
                self.cycle();
                Ok(())
            }
            Stack::Pop(reg) => {
                let val = self.exec_pop()?;
                self.write_register_u16(reg.into(), val);
                self.cycle();
                Ok(())
            }
            _ => Err(StepError::Unimplemented(instr.into())),
        }
    }

    fn exec_extended(&mut self, _instr: instructions::Extended) -> StepResult<()> {
        use decoder::{idecoders, TwoByteDataDecoder};
        use instructions::Carry::Carry;
        use instructions::Extended;
        use instructions::RotateDirection::{Left, Right};
        use registers::WordRegister as wr;
        let data_byte = self.exec_read_inc_pc()?;
        let actual_instruction = idecoders::Extended.decode(0xCB, data_byte).unwrap();
        let ext = match actual_instruction {
            instructions::Instruction::Extended(ex) => ex,
            _ => unreachable!(),
        };
        match ext {
            Extended::SetBit(bit, reg) => {
                let val = self.read_register_u8(reg);
                let new_val = val | (1 << bit);
                self.write_register_u8(reg, new_val);
                self.cycle();
                Ok(())
            }
            Extended::SetMemoryBit(bit) => {
                let addr = self.read_register_u16(wr::HL);
                self.cycle();
                let val = self.read_memory_u8(addr)?;
                self.cycle();
                let new_val = val | (1 << bit);
                self.write_memory_u8(addr, new_val)?;
                self.cycle();
                Ok(())
            }
            Extended::ResetBit(bit, reg) => {
                let val = self.read_register_u8(reg);
                let new_val = val & !(1 << bit);
                self.write_register_u8(reg, new_val);
                self.cycle();
                Ok(())
            }
            Extended::ResetMemoryBit(bit) => {
                let addr = self.read_register_u16(wr::HL);
                self.cycle();
                let val = self.read_memory_u8(addr)?;
                self.cycle();
                let new_val = val & !(1 << bit);
                self.write_memory_u8(addr, new_val)?;
                self.cycle();
                Ok(())
            }
            Extended::TestBit(bit, reg) => {
                let val = self.read_register_u8(reg);
                let bit_test = val & (1 << bit);
                self.set_flag_to(registers::Flag::Zero, bit_test == 0);
                self.set_flag(registers::Flag::HalfCarry);
                self.set_flag_to(registers::Flag::AddSubtract, false);
                self.cycle();
                Ok(())
            }
            Extended::TestMemoryBit(bit) => {
                let addr = self.read_register_u16(wr::HL);
                self.cycle();
                let val = self.read_memory_u8(addr)?;
                self.cycle();
                let bit_test = val & (1 << bit);
                self.set_flag_to(registers::Flag::Zero, bit_test == 0);
                self.set_flag(registers::Flag::HalfCarry);
                self.set_flag_to(registers::Flag::AddSubtract, false);

                Ok(())
            }
            Extended::Rotate(dir, carry, reg) => {
                let current_value = self.read_register_u8(reg);
                let high_byte = if carry != Carry {
                    if self.read_flag(registers::Flag::Carry) {
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
                self.write_register_u8(reg, rotated);
                self.set_flag_to(registers::Flag::Carry, carry);
                self.cycle();
                Ok(())
            }
            Extended::RotateMemory(dir, carry) => {
                let addr = self.read_register_u16(wr::HL);
                self.cycle();
                let current_value = self.read_memory_u8(addr)?;
                self.cycle();
                let high_byte = if carry != Carry {
                    if self.read_flag(registers::Flag::Carry) {
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
                self.set_flag_to(registers::Flag::Carry, carry);
                self.cycle();
                Ok(())
            }
            Extended::Swap(reg) => {
                let value = self.read_register_u8(reg);
                let low_nibble = value & 0x0F;
                let high_nibble = value & 0xF0;
                let new_value = (low_nibble.rotate_left(4)) + (high_nibble.rotate_right(4));
                self.write_register_u8(reg, new_value);
                self.cycle();
                Ok(())
            }
            Extended::SwapMemory => {
                let addr = self.read_register_u16(registers::WordRegister::HL);
                self.cycle();
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
                let value = self.read_register_u8(reg);
                let (shifted_value, carry) = match dir {
                    Left => (value << 1, value & 0x80 != 0),
                    Right => (value >> 1, value & 0x01 != 0),
                };
                self.set_flag_to(registers::Flag::Carry, carry);
                self.write_register_u8(reg, shifted_value);
                self.cycle();
                Ok(())
            }
            Extended::ShiftRightExtend(reg) => {
                let value = self.read_register_u8(reg);
                let value16 = u16::from(value);
                let extra_bit = (value16 << 1) & 0xff00;
                let shifted_value = (extra_bit + value16) >> 1;
                let actual_byte = shifted_value.to_le_bytes()[0];
                self.write_register_u8(reg, actual_byte);
                self.cycle();
                Ok(())
            }
            Extended::ShiftMemoryRightExtend => {
                let addr = self.read_register_u16(registers::WordRegister::HL);
                self.cycle();
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
                let addr = self.read_register_u16(registers::WordRegister::HL);
                self.cycle();
                let value = self.read_memory_u8(addr)?;
                let (shifted_value, carry) = match dir {
                    Left => (value << 1, value & 0x80 != 0),
                    Right => (value >> 1, value & 0x01 != 0),
                };
                self.set_flag_to(registers::Flag::Carry, carry);
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
            Instruction::Load(l) => self.exec_load(l),
            Instruction::Jump(j) => self.exec_jump(j),
            Instruction::RegisterAL(reg) => self.exec_register_al(reg),
            Instruction::ConstantAL(op, _) => self.exec_constant_al(op),
            Instruction::Stack(s) => self.exec_stack(s),
            Instruction::Extended(ex) => self.exec_extended(ex),
            Instruction::NOP => {
                self.cycle();
                Ok(())
            }
            Instruction::InvertCarry => {
                self.invert_flag(registers::Flag::Carry);
                self.reset_flag(registers::Flag::HalfCarry);
                self.reset_flag(registers::Flag::AddSubtract);
                self.cycle();
                Ok(())
            }
            Instruction::SetCarry => {
                self.set_flag(registers::Flag::Carry);
                self.reset_flag(registers::Flag::HalfCarry);
                self.reset_flag(registers::Flag::AddSubtract);
                self.cycle();
                Ok(())
            }
            _ => Err(StepError::Unimplemented(instr)),
        }
    }

    pub fn step(&mut self) -> StepResult<()> {
        let instruction = self.current_instruction()?;
        self.pc_register = self.pc_register.wrapping_add(1);
        self.exec(instruction)?;
        Ok(())
    }

    pub fn current_instruction(&self) -> StepResult<instructions::Instruction> {
        let pc_value = self.read_memory_u8(self.pc_register)?;
        let next_pc = self.pc_register.wrapping_add(1);
        let instruction = self
            .decoder
            .decode(pc_value, &mut self.memory_iter(next_pc))?;
        Ok(instruction)
    }

    fn cycle(&mut self) {
        self.clocks_elapsed += 4;
    }

    pub fn clocks_elapsed(&self) -> u64 {
        self.clocks_elapsed
    }
}

#[cfg(test)]
mod alu_tests;

#[cfg(test)]
mod extended_opcode_tests;

#[cfg(test)]
mod jump_tests;

#[cfg(test)]
mod load_tests;

#[cfg(test)]
mod stack_tests;

#[cfg(test)]
pub(crate) mod testutils {
    use super::*;
    use crate::gameboy;

    pub const PROGRAM_START: u16 = 0x200;
    pub const PROG_MEMORY_OFFSET: usize = 0x200;

    pub fn make_cartridge() -> rom::Cartridge {
        rom::Cartridge::from_data(vec![0u8; 0x8000]).unwrap()
    }

    pub fn make_cartridge_with(program: &[u8]) -> rom::Cartridge {
        let mut data = vec![0u8; 0x8000];
        data[PROG_MEMORY_OFFSET..PROG_MEMORY_OFFSET + program.len()].clone_from_slice(program);
        rom::Cartridge::from_data(data).unwrap()
    }

    pub fn run_program(steps: u64, program: &[u8]) -> StepResult<GameBoy> {
        let cartridge = make_cartridge_with(program);
        let mut gb = GameBoy::new(cartridge, gameboy::GameBoyModel::GameBoy);
        gb.write_register_u16(registers::WordRegister::PC, PROGRAM_START);
        for _ in 0..steps {
            gb.step()?
        }
        Ok(gb)
    }
}

#[cfg(test)]
mod tests {
    use super::testutils::*;
    use super::*;
    use crate::gameboy;

    #[test]
    fn test_reg_write_u8_read_u8() {
        let mut cpu = GameBoy::new(make_cartridge(), gameboy::GameBoyModel::GameBoy);

        cpu.write_register_u8(registers::ByteRegister::A, 0x01);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::A), 0x01);

        cpu.write_register_u8(registers::ByteRegister::B, 0x02);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::B), 0x02);

        cpu.write_register_u8(registers::ByteRegister::C, 0x03);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::C), 0x03);

        cpu.write_register_u8(registers::ByteRegister::D, 0x04);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::D), 0x04);

        cpu.write_register_u8(registers::ByteRegister::E, 0x05);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::E), 0x05);

        cpu.write_register_u8(registers::ByteRegister::F, 0x66);
        // F register lower 4 bytes are not writable
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::F), 0x60);

        cpu.write_register_u8(registers::ByteRegister::H, 0x07);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::H), 0x07);

        cpu.write_register_u8(registers::ByteRegister::L, 0x08);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::L), 0x08);
    }

    #[test]
    fn test_reg_write_u16_read_u16() {
        let mut cpu = GameBoy::new(make_cartridge(), gameboy::GameBoyModel::GameBoy);

        cpu.write_register_u16(registers::WordRegister::AF, 0x1234);
        // F register lower 4 bytes are not writable
        assert_eq!(cpu.read_register_u16(registers::WordRegister::AF), 0x1230);

        cpu.write_register_u16(registers::WordRegister::BC, 0x1235);
        assert_eq!(cpu.read_register_u16(registers::WordRegister::BC), 0x1235);

        cpu.write_register_u16(registers::WordRegister::DE, 0x1236);
        assert_eq!(cpu.read_register_u16(registers::WordRegister::DE), 0x1236);

        cpu.write_register_u16(registers::WordRegister::HL, 0x1237);
        assert_eq!(cpu.read_register_u16(registers::WordRegister::HL), 0x1237);

        cpu.write_register_u16(registers::WordRegister::PC, 0x1238);
        assert_eq!(cpu.read_register_u16(registers::WordRegister::PC), 0x1238);

        cpu.write_register_u16(registers::WordRegister::SP, 0x1239);
        assert_eq!(cpu.read_register_u16(registers::WordRegister::SP), 0x1239);
    }

    #[test]
    fn test_reg_write_u8_read_u16() {
        let mut cpu = GameBoy::new(make_cartridge(), gameboy::GameBoyModel::GameBoy);

        cpu.write_register_u8(registers::ByteRegister::A, 0x15);
        cpu.write_register_u8(registers::ByteRegister::F, 0x12);
        assert_eq!(
            cpu.read_register_u16(registers::WordRegister::AF)
                .to_be_bytes(),
            // F register lower 4 bytes are not writable
            [0x15, 0x10]
        );

        cpu.write_register_u8(registers::ByteRegister::B, 0x25);
        cpu.write_register_u8(registers::ByteRegister::C, 0x22);
        assert_eq!(
            cpu.read_register_u16(registers::WordRegister::BC)
                .to_be_bytes(),
            [0x25, 0x22]
        );

        cpu.write_register_u8(registers::ByteRegister::D, 0x35);
        cpu.write_register_u8(registers::ByteRegister::E, 0x32);
        assert_eq!(
            cpu.read_register_u16(registers::WordRegister::DE)
                .to_be_bytes(),
            [0x35, 0x32]
        );

        cpu.write_register_u8(registers::ByteRegister::H, 0x45);
        cpu.write_register_u8(registers::ByteRegister::L, 0x42);
        assert_eq!(
            cpu.read_register_u16(registers::WordRegister::HL)
                .to_be_bytes(),
            [0x45, 0x42]
        );
    }

    #[test]
    fn test_reg_write_u16_read_u8() {
        let mut cpu = GameBoy::new(make_cartridge(), gameboy::GameBoyModel::GameBoy);

        cpu.write_register_u16(registers::WordRegister::AF, 0x9876);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::A), 0x98);
        // F register lower 4 bytes are not writable
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::F), 0x70);

        cpu.write_register_u16(registers::WordRegister::BC, 0x9775);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::B), 0x97);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::C), 0x75);

        cpu.write_register_u16(registers::WordRegister::DE, 0x9674);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::D), 0x96);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::E), 0x74);

        cpu.write_register_u16(registers::WordRegister::HL, 0x9573);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::H), 0x95);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::L), 0x73);
    }

    #[test]
    fn test_mem_write_u8_read_u8_sysram() -> MemoryResult<()> {
        let mut cpu = GameBoy::new(make_cartridge(), gameboy::GameBoyModel::GameBoy);

        cpu.write_memory_u8(0xc100, 0x32)?;
        assert_eq!(cpu.read_memory_u8(0xc100), Ok(0x32));
        Ok(())
    }

    #[test]
    fn test_mem_write_u16_read_u16_sysram() -> MemoryResult<()> {
        let mut cpu = GameBoy::new(make_cartridge(), gameboy::GameBoyModel::GameBoy);

        cpu.write_memory_u16(0xc100, 0x1032)?;
        assert_eq!(cpu.read_memory_u16(0xc100), Ok(0x1032));
        Ok(())
    }

    #[test]
    fn test_mem_write_u8_read_u16_sysram() -> MemoryResult<()> {
        let mut cpu = GameBoy::new(make_cartridge(), gameboy::GameBoyModel::GameBoy);

        cpu.write_memory_u8(0xc100, 0x48)?;
        cpu.write_memory_u8(0xc101, 0x94)?;

        assert_eq!(cpu.read_memory_u16(0xc100), Ok(0x9448));
        Ok(())
    }

    #[test]
    fn test_mem_write_u16_read_u8_sysram() -> MemoryResult<()> {
        let mut cpu = GameBoy::new(make_cartridge(), gameboy::GameBoyModel::GameBoy);

        cpu.write_memory_u16(0xc200, 0x1345)?;

        assert_eq!(cpu.read_memory_u8(0xc200), Ok(0x45));
        assert_eq!(cpu.read_memory_u8(0xc201), Ok(0x13));
        Ok(())
    }

    #[test]
    fn test_write_u8_read_i8_sysram() -> MemoryResult<()> {
        let mut cpu = GameBoy::new(make_cartridge(), gameboy::GameBoyModel::GameBoy);
        let signed_value = i8::from_le_bytes([0xa2]);

        cpu.write_memory_u8(0xc200, 0xa2)?;

        assert_eq!(cpu.read_memory_i8(0xc200), Ok(signed_value));
        Ok(())
    }

    #[test]
    fn test_mem_write_u8_read_u8_vram() -> MemoryResult<()> {
        let mut cpu = GameBoy::new(make_cartridge(), gameboy::GameBoyModel::GameBoy);

        cpu.write_memory_u8(0x8100, 0x32)?;
        assert_eq!(cpu.read_memory_u8(0x8100), Ok(0x32));
        Ok(())
    }

    #[test]
    fn test_mem_write_u8_read_u8_cpuram() -> MemoryResult<()> {
        let mut cpu = GameBoy::new(make_cartridge(), gameboy::GameBoyModel::GameBoy);

        cpu.write_memory_u8(0xff80, 0x32)?;
        assert_eq!(cpu.read_memory_u8(0xff80), Ok(0x32));
        Ok(())
    }

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
}
