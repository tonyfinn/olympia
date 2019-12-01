use crate::decoder;
use crate::instructions;
use crate::registers;
use crate::rom;

use std::convert::TryFrom;

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
                let has_half_carry = (((current_value & 0x0FFF) + (value_to_add & 0x0FFF)) & 0xF000) != 0;
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

    fn exec_extended(&mut self, instr: instructions::Extended) -> StepResult<()> {
        use decoder::{idecoders, TwoByteDataDecoder};
        use instructions::Extended;
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
            _ => Err(StepError::Unimplemented(instr.into())),
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
mod tests {
    use super::*;
    use crate::gameboy;

    const PROGRAM_START: u16 = 0x200;
    const PROG_MEMORY_OFFSET: usize = 0x200;

    fn make_cartridge() -> rom::Cartridge {
        rom::Cartridge::from_data(vec![0u8; 0x8000]).unwrap()
    }

    fn make_cartridge_with(program: &[u8]) -> rom::Cartridge {
        let mut data = vec![0u8; 0x8000];
        data[PROG_MEMORY_OFFSET..PROG_MEMORY_OFFSET + program.len()].clone_from_slice(program);
        rom::Cartridge::from_data(data).unwrap()
    }

    fn run_program(steps: u64, program: &[u8]) -> StepResult<GameBoy> {
        let cartridge = make_cartridge_with(program);
        let mut gb = GameBoy::new(cartridge, gameboy::GameBoyModel::GameBoy);
        gb.write_register_u16(registers::WordRegister::PC, PROGRAM_START);
        for _ in 0..steps {
            gb.step()?
        }
        Ok(gb)
    }

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
    fn test_loads() -> StepResult<()> {
        let gb = run_program(
            6,
            &[
                0x26, 0x80, // LD H, 0x80 - 8 clocks
                0x2E, 0x00, // LD L, 0x00 - 8 clocks
                0x06, 0x25, // LD B, 0x25 - 8 clocks
                0x50, // LD D, B - 4 clocks
                0x72, // LD (HL), D - 8 clocks
                0x5E, // LD E, (HL) - 8 clocks
            ],
        )?;

        assert_eq!(gb.read_register_u8(registers::ByteRegister::B), 0x25);
        assert_eq!(gb.read_register_u8(registers::ByteRegister::D), 0x25);
        assert_eq!(gb.read_register_u8(registers::ByteRegister::E), 0x25);
        assert_eq!(gb.read_memory_u8(0x8000)?, 0x25);
        assert_eq!(gb.clocks_elapsed(), 44);

        Ok(())
    }

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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0xFF);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::HalfCarry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x10);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::HalfCarry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x00);
        assert_eq!(gb.read_flag(registers::Flag::Zero), true);
        assert_eq!(gb.read_flag(registers::Flag::Carry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x01);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x01);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x0F);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::HalfCarry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x00);
        assert_eq!(gb.read_flag(registers::Flag::Zero), true);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::HalfCarry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0xFF);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), true);
        assert_eq!(gb.read_flag(registers::Flag::HalfCarry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0xFE);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0xFF);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x00);
        assert_eq!(gb.read_flag(registers::Flag::Zero), true);
        assert_eq!(gb.read_flag(registers::Flag::Carry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x00);
        assert_eq!(gb.read_flag(registers::Flag::Zero), true);
        assert_eq!(gb.read_flag(registers::Flag::Carry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x01);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x01);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0xF6);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0xF5);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x00);
        assert_eq!(gb.read_flag(registers::Flag::Zero), true);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x00);
        assert_eq!(gb.read_flag(registers::Flag::Zero), true);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0xFF);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0xFF);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x06 & 0x05);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
        assert_eq!(gb.clocks_elapsed(), 20);

        let gb = run_program(
            3,
            &[
                0x3E, 0x06, // LD A, 0x06 - 8 clocks
                0x06, 0x10, // LD B, 0x05 - 8 clocks
                0xA0, // AND A, B - 4 clocks
            ],
        )?;

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0);
        assert_eq!(gb.read_flag(registers::Flag::Zero), true);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x07);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
        assert_eq!(gb.clocks_elapsed(), 20);

        let gb = run_program(
            3,
            &[
                0x3E, 0x00, // LD A, 0x06 - 8 clocks
                0x06, 0x00, // LD B, 0x05 - 8 clocks
                0xB0, // OR A, B - 4 clocks
            ],
        )?;

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x0);
        assert_eq!(gb.read_flag(registers::Flag::Zero), true);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x03);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
        assert_eq!(gb.clocks_elapsed(), 20);

        let gb = run_program(
            3,
            &[
                0x3E, 0x0F, // LD A, 0x06 - 8 clocks
                0x06, 0x0F, // LD B, 0x05 - 8 clocks
                0xA8, // XOR A, B - 4 clocks
            ],
        )?;

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x0);
        assert_eq!(gb.read_flag(registers::Flag::Zero), true);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x0C);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x0C);
        assert_eq!(gb.read_flag(registers::Flag::Zero), true);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::A), 0x0C);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
        assert_eq!(gb.clocks_elapsed(), 20);

        Ok(())
    }

    #[test]
    fn test_stack() -> StepResult<()> {
        let gb = run_program(
            7,
            &[
                0x06, 0x05, // LD B, 0x05 - 8 clocks
                0x0E, 0x08, // LD C, 0x08 - 8 clocks
                0xC5, // PUSH BC - 16 blocks
                0xC5, // PUSH BC - 16 blocks
                0xC5, // PUSH BC - 16 blocks
                0xD1, // POP DE - 12 blocks
                0xE1, // POP HL - 12 blocks
            ],
        )?;

        assert_eq!(gb.read_register_u16(registers::WordRegister::DE), 0x0508);
        assert_eq!(gb.read_register_u16(registers::WordRegister::HL), 0x0508);
        assert_eq!(gb.read_register_u16(registers::WordRegister::SP), 0xFFFC);
        assert_eq!(gb.read_memory_u16(0xFFFA)?, 0x0508);
        assert_eq!(gb.read_memory_u16(0xFFFC)?, 0x0508);
        assert_eq!(gb.read_memory_u16(0xFFF8)?, 0x0508);
        assert_eq!(gb.clocks_elapsed(), 88);

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

    #[test]
    fn test_set_bit() -> StepResult<()> {
        let gb = run_program(
            2,
            &[
                0x2E, 0x00, // LD L, 0x00 - 8 clocks
                0xCB, 0xFD, // BIT 7, L - 8 clocks
            ],
        )?;

        assert_eq!(gb.read_register_u8(registers::ByteRegister::L), 0x80);
        assert_eq!(gb.clocks_elapsed(), 16);

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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::L), 0xFF);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
        assert_eq!(gb.clocks_elapsed(), 12);

        let gb = run_program(
            2,
            &[
                0x2E, 0xFF, // LD L, 0x00 - 8 clocks
                0x2C, // INC L - 4 clocks
            ],
        )?;

        assert_eq!(gb.read_register_u8(registers::ByteRegister::L), 0x00);
        assert_eq!(gb.read_flag(registers::Flag::Zero), true);
        assert_eq!(gb.read_flag(registers::Flag::Carry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
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

        assert_eq!(gb.read_register_u8(registers::ByteRegister::L), 0x01);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
        assert_eq!(gb.clocks_elapsed(), 12);

        let gb = run_program(
            2,
            &[
                0x2E, 0x01, // LD L, 0x00 - 8 clocks
                0x2D, // DEC L - 4 clocks
            ],
        )?;

        assert_eq!(gb.read_register_u8(registers::ByteRegister::L), 0x00);
        assert_eq!(gb.read_flag(registers::Flag::Zero), true);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
        assert_eq!(gb.clocks_elapsed(), 12);

        let gb = run_program(
            2,
            &[
                0x2E, 0x00, // LD L, 0x00 - 8 clocks
                0x2D, // DEC L - 4 clocks
            ],
        )?;

        assert_eq!(gb.read_register_u8(registers::ByteRegister::L), 0xFF);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), true);
        assert_eq!(gb.clocks_elapsed(), 12);

        Ok(())
    }


    #[test]
    fn test_increment_16() -> StepResult<()> {
        let gb = run_program(
            3,
            &[
                0x26, 0x01, // LD H, 0x01 - 8 blocks
                0x2E, 0xFF, // LD L, 0xFF - 8 clocks
                0x23, // INC HL - 8 clocks
            ],
        )?;

        assert_eq!(gb.read_register_u16(registers::WordRegister::HL), 0x200);
        assert_eq!(gb.clocks_elapsed(), 24);

        Ok(())
    }

    #[test]
    fn test_decrement_16() -> StepResult<()> {
        let gb = run_program(
            3,
            &[
                0x26, 0x01, // LD H, 0x01 - 8 blocks
                0x2E, 0x00, // LD L, 0xFF - 8 clocks
                0x2B, // DEC HL - 8 clocks
            ],
        )?;

        assert_eq!(gb.read_register_u16(registers::WordRegister::HL), 0xFF);
        assert_eq!(gb.clocks_elapsed(), 24);

        Ok(())
    }

    #[test]
    fn test_add_16() -> StepResult<()> {
        let gb = run_program(
            5,
            &[
                0x26, 0x0F, // LD H, 0x0F - 8 blocks
                0x2E, 0xFF, // LD L, 0xFF - 8 clocks
                0x06, 0x00, // LD B, 0 - 8 clocks
                0x0E, 0x01, // LD C, 1 - 8 blocks
                0x09, // ADD HL, BC - 8 clocks
            ],
        )?;

        assert_eq!(gb.read_register_u16(registers::WordRegister::HL), 0x1000);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), false);
        assert_eq!(gb.read_flag(registers::Flag::HalfCarry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
        assert_eq!(gb.clocks_elapsed(), 40);

        Ok(())
    }

    #[test]
    fn test_add_16_carry() -> StepResult<()> {
        let gb = run_program(
            5,
            &[
                0x26, 0x0F, // LD H, 0x0F - 8 blocks
                0x2E, 0xFF, // LD L, 0xFF - 8 clocks
                0x06, 0xF0, // LD B, 0 - 8 clocks
                0x0E, 0x02, // LD C, 1 - 8 blocks
                0x09, // ADD HL, BC - 8 clocks
            ],
        )?;

        assert_eq!(gb.read_register_u16(registers::WordRegister::HL), 0x0001);
        assert_eq!(gb.read_flag(registers::Flag::Zero), false);
        assert_eq!(gb.read_flag(registers::Flag::Carry), true);
        assert_eq!(gb.read_flag(registers::Flag::HalfCarry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
        assert_eq!(gb.clocks_elapsed(), 40);

        Ok(())
    }

    #[test]
    fn test_add_16_zero() -> StepResult<()> {
        let gb = run_program(
            5,
            &[
                0x26, 0xFF, // LD H, 0x0F - 8 blocks
                0x2E, 0xFF, // LD L, 0xFF - 8 clocks
                0x06, 0x00, // LD B, 0 - 8 clocks
                0x0E, 0x01, // LD C, 1 - 8 blocks
                0x09, // ADD HL, BC - 8 clocks
            ],
        )?;

        assert_eq!(gb.read_register_u16(registers::WordRegister::HL), 0x0);
        assert_eq!(gb.read_flag(registers::Flag::Zero), true);
        assert_eq!(gb.read_flag(registers::Flag::Carry), true);
        assert_eq!(gb.read_flag(registers::Flag::HalfCarry), true);
        assert_eq!(gb.read_flag(registers::Flag::AddSubtract), false);
        assert_eq!(gb.clocks_elapsed(), 40);

        Ok(())
    }
}
