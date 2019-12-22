pub(crate) mod cpu;
mod dma;

use crate::decoder;
use crate::gameboy::cpu::Cpu;
use crate::gameboy::dma::DmaUnit;
use crate::instructions;
use crate::registers;
use crate::registers::{ByteRegister as br, WordRegister as wr};
use crate::rom;
use crate::rom::TargetConsole;

use core::convert::TryFrom;

pub struct GameBoy {
    cpu: Cpu,
    dma: DmaUnit,
    cpuram: [u8; 127],
    oamram: [u8; 160],
    sysram: [u8; 0x2000],
    vram: [u8; 0x2000],
    cartridge: rom::Cartridge,
    decoder: decoder::Decoder,
    clocks_elapsed: u64,
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum MemoryError {
    InvalidRomAddress(u16),
    InvalidRamAddress(u16),
    UnmappedAddress(u16),
}

pub type MemoryResult<T> = Result<T, MemoryError>;

#[derive(PartialEq, Eq, Debug)]
enum SetZeroMode {
    Test,
    Clear,
}

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
    pub fn new(cartridge: rom::Cartridge, model: GameBoyModel) -> GameBoy {
        GameBoy {
            cpu: Cpu::new(model, cartridge.target),
            dma: Default::default(),
            cpuram: [0u8; 127],
            oamram: [0u8; 160],
            sysram: [0u8; 0x2000],
            vram: [0u8; 0x2000],
            cartridge,
            decoder: decoder::Decoder::new(),
            clocks_elapsed: 0,
        }
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
        } else if addr >= 0xfe00 && addr < 0xfea0 {
            Ok(self.oamram[(addr - 0xfe00) as usize])
        } else if addr >= 0xff80 && addr <= 0xfffe {
            Ok(self.cpuram[(addr - 0xff80) as usize])
        } else if GameBoy::is_mem_register(addr) {
            Ok(self.read_mem_register(addr))
        } else {
            Err(MemoryError::UnmappedAddress(addr))
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
        } else if addr >= 0xfe00 && addr < 0xfea0 {
            self.oamram[(addr - 0xfe00) as usize] = value;
            Ok(())
        } else if GameBoy::is_mem_register(addr) {
            self.write_mem_register(addr, value);
            Ok(())
        } else if addr >= 0xff80 && addr <= 0xfffe {
            self.cpuram[(addr - 0xff80) as usize] = value;
            Ok(())
        } else {
            Err(MemoryError::UnmappedAddress(addr))
        }
    }

    fn is_mem_register(addr: u16) -> bool {
        (addr >= 0xff00 && addr < 0xff80) || addr == 0xffff
    }

    fn read_mem_register(&self, addr: u16) -> u8 {
        if addr == 0xff46 {
            self.dma.register_value
        } else {
            0
        }
    }

    fn write_mem_register(&mut self, addr: u16, value: u8) {
        if addr == 0xff46 {
            self.dma.start(value);
        }
    }

    pub fn write_memory_u16(&mut self, addr: u16, value: u16) -> MemoryResult<()> {
        let bytes = value.to_le_bytes();

        self.write_memory_u8(addr, bytes[0])?;
        self.write_memory_u8(addr.wrapping_add(1), bytes[1])?;
        Ok(())
    }

    pub fn read_register_u16(&self, reg: registers::WordRegister) -> u16 {
        self.cpu.read_register_u16(reg)
    }

    pub fn write_register_u16(&mut self, reg: registers::WordRegister, val: u16) {
        self.cpu.write_register_u16(reg, val)
    }

    pub fn read_register_u8(&self, reg: registers::ByteRegister) -> u8 {
        self.cpu.read_register_u8(reg)
    }

    pub fn write_register_u8(&mut self, reg: registers::ByteRegister, val: u8) {
        self.cpu.write_register_u8(reg, val)
    }

    fn memory_iter(&self, start: u16) -> MemoryIterator {
        MemoryIterator {
            addr: start,
            gb: &self,
        }
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

    fn exec_read_inc_pc(&mut self) -> StepResult<u8> {
        let pc_value = self.cpu.read_register_u16(wr::PC);
        let val = self.read_memory_u8(pc_value)?;
        self.cpu
            .write_register_u16(wr::PC, pc_value.wrapping_add(1));
        self.cycle();
        Ok(val)
    }

    fn exec_push(&mut self, value: u16) -> StepResult<()> {
        let stack_addr = self.cpu.read_register_u16(registers::WordRegister::SP);
        let [low, high] = value.to_le_bytes();
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

    fn exec_pop(&mut self) -> StepResult<u16> {
        let stack_addr = self.cpu.read_register_u16(registers::WordRegister::SP);
        let low = self.read_memory_u8(stack_addr)?;
        self.cycle();
        let stack_addr = stack_addr.wrapping_add(1);
        let high = self.read_memory_u8(stack_addr)?;
        self.cycle();
        self.cpu
            .write_register_u16(registers::WordRegister::SP, stack_addr.wrapping_add(1));
        Ok(u16::from_le_bytes([low, high]))
    }

    fn exec_load(&mut self, instr: instructions::Load) -> StepResult<()> {
        use instructions::Load;
        match instr {
            Load::RegisterRegister(dest, src) => {
                let value = self.cpu.read_register_u8(src);
                self.cpu.write_register_u8(dest, value);
                self.cycle();
            }
            Load::MemoryRegister(dest, src) => {
                let value = self.cpu.read_register_u8(src);
                let target_addr = self.cpu.read_register_u16(dest);
                self.write_memory_u8(target_addr, value)?;
                self.cycle();
                self.cycle();
            }
            Load::Constant(dest, _) => {
                let val = self.exec_read_inc_pc()?;
                self.cpu.write_register_u8(dest, val);
                self.cycle();
            }
            Load::Constant16(reg, _) => {
                let first_byte = self.exec_read_inc_pc()?;
                let second_byte = self.exec_read_inc_pc()?;
                let value = u16::from_le_bytes([first_byte, second_byte]);
                self.write_register_u16(reg.into(), value);
                self.cycle();
            }
            Load::RegisterMemory(dest, src) => {
                let addr = self.cpu.read_register_u16(src);
                let value = self.read_memory_u8(addr)?;
                self.cycle();
                self.cpu.write_register_u8(dest, value);
                self.cycle();
            }
            Load::ConstantMemory(_) => {
                let val = self.exec_read_inc_pc()?;
                let addr = self.cpu.read_register_u16(wr::HL);
                self.write_memory_u8(addr, val)?;
                self.cycle();
                self.cycle();
            }
            Load::AMemoryOffset => {
                let addr = u16::from(self.read_register_u8(br::C)) + 0xff00;
                let value = self.read_memory_u8(addr)?;
                self.cycle();
                self.write_register_u8(br::A, value);
                self.cycle();
            }
            Load::MemoryOffsetA => {
                let addr = u16::from(self.read_register_u8(br::C)) + 0xff00;
                let value = self.read_register_u8(br::A);
                self.write_memory_u8(addr, value)?;
                self.cycle();
                self.cycle();
            }
            Load::AIndirect(_) => {
                let addr_first_byte = self.exec_read_inc_pc()?;
                let addr_second_byte = self.exec_read_inc_pc()?;
                let addr = u16::from_le_bytes([addr_first_byte, addr_second_byte]);
                let value = self.read_memory_u8(addr)?;
                self.cycle();
                self.write_register_u8(br::A, value);
                self.cycle();
            }
            Load::IndirectA(_) => {
                let addr_first_byte = self.exec_read_inc_pc()?;
                let addr_second_byte = self.exec_read_inc_pc()?;
                let addr = u16::from_le_bytes([addr_first_byte, addr_second_byte]);
                let value = self.read_register_u8(br::A);
                self.write_memory_u8(addr, value)?;
                self.cycle();
                self.cycle();
            }
            Load::AHighOffset(_) => {
                let offset = self.exec_read_inc_pc()?;
                let addr = u16::from(offset) + 0xff00;
                let value = self.read_memory_u8(addr)?;
                self.cycle();
                self.write_register_u8(br::A, value);
                self.cycle();
            }
            Load::HighOffsetA(_) => {
                let offset = self.exec_read_inc_pc()?;
                let addr = u16::from(offset) + 0xff00;
                self.cycle();
                let value = self.read_register_u8(br::A);
                self.write_memory_u8(addr, value)?;
                self.cycle();
            }
            Load::Increment16A(inc) => {
                let addr = self.read_register_u16(wr::HL);
                self.write_memory_u8(addr, self.read_register_u8(br::A))?;
                self.cycle();
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
                self.cycle();
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

    fn exec_al(&mut self, op: instructions::ALOp, arg: u8) -> u8 {
        let current_value = self.cpu.read_register_u8(br::A);
        use instructions::ALOp;
        match op {
            ALOp::Add => {
                let (new, overflow) = current_value.overflowing_add(arg);
                self.cpu.set_flag_to(registers::Flag::Carry, overflow);
                self.cpu.reset_flag(registers::Flag::AddSubtract);
                self.cpu.set_flag_to(registers::Flag::Zero, new == 0);
                self.set_add_half_carry(current_value, arg);
                new
            }
            ALOp::AddCarry => {
                let carry_bit = u8::from(self.cpu.read_flag(registers::Flag::Carry));
                let (tmp, overflow) = current_value.overflowing_add(arg);
                let (new, overflow_carry) = tmp.overflowing_add(carry_bit);
                self.cpu
                    .set_flag_to(registers::Flag::Carry, overflow | overflow_carry);
                self.cpu.reset_flag(registers::Flag::AddSubtract);
                self.cpu.set_flag_to(registers::Flag::Zero, new == 0);
                self.set_add_half_carry(current_value, arg + carry_bit);
                new
            }
            ALOp::Sub => {
                let (new, overflow) = current_value.overflowing_sub(arg);
                self.cpu.set_flag_to(registers::Flag::Carry, overflow);
                self.cpu.set_flag(registers::Flag::AddSubtract);
                self.cpu.set_flag_to(registers::Flag::Zero, new == 0);
                self.set_sub_half_carry(current_value, arg);
                new
            }
            ALOp::SubCarry => {
                let carry_bit = u8::from(self.cpu.read_flag(registers::Flag::Carry));
                let (tmp, overflow) = current_value.overflowing_sub(arg);
                let (new, overflow_carry) = tmp.overflowing_sub(carry_bit);
                self.cpu
                    .set_flag_to(registers::Flag::Carry, overflow | overflow_carry);
                self.cpu.set_flag(registers::Flag::AddSubtract);
                self.cpu.set_flag_to(registers::Flag::Zero, new == 0);
                self.set_sub_half_carry(current_value, arg + carry_bit);
                new
            }
            ALOp::Compare => {
                let (new, overflow) = current_value.overflowing_sub(arg);
                self.cpu.set_flag_to(registers::Flag::Carry, overflow);
                self.cpu.set_flag(registers::Flag::AddSubtract);
                self.cpu.set_flag_to(registers::Flag::Zero, new == 0);
                self.set_sub_half_carry(current_value, arg);
                current_value
            }
            ALOp::And => {
                let new = current_value & arg;
                self.cpu.reset_flag(registers::Flag::Carry);
                self.cpu.set_flag(registers::Flag::HalfCarry);
                self.cpu.reset_flag(registers::Flag::AddSubtract);
                self.cpu.set_flag_to(registers::Flag::Zero, new == 0);
                new
            }
            ALOp::Or => {
                let new = current_value | arg;
                self.cpu.reset_flag(registers::Flag::Carry);
                self.cpu.reset_flag(registers::Flag::HalfCarry);
                self.cpu.reset_flag(registers::Flag::AddSubtract);
                self.cpu.set_flag_to(registers::Flag::Zero, new == 0);
                new
            }
            ALOp::Xor => {
                let new = current_value ^ arg;
                self.cpu.reset_flag(registers::Flag::Carry);
                self.cpu.reset_flag(registers::Flag::HalfCarry);
                self.cpu.reset_flag(registers::Flag::AddSubtract);
                self.cpu.set_flag_to(registers::Flag::Zero, new == 0);
                new
            }
        }
    }

    fn set_add_half_carry(&mut self, a: u8, b: u8) {
        let half_add = ((a & 0xF) + (b & 0xF)) & 0xF0;
        self.cpu
            .set_flag_to(registers::Flag::HalfCarry, half_add != 0);
    }

    fn set_sub_half_carry(&mut self, a: u8, b: u8) {
        let sub = (a & 0x1F).wrapping_sub(b & 0x0F);
        let half_carry = (sub & 0x10) != a & 0x10;
        self.cpu.set_flag_to(registers::Flag::HalfCarry, half_carry);
    }

    fn exec_register_al(&mut self, instr: instructions::RegisterAL) -> StepResult<()> {
        use instructions::RegisterAL;
        match instr {
            RegisterAL::ByteOp(op, reg) => {
                let reg_value = self.cpu.read_register_u8(reg);
                let new_value = self.exec_al(op, reg_value);
                self.cpu.write_register_u8(br::A, new_value);
            }
            RegisterAL::Increment(reg) => {
                let reg_value = self.cpu.read_register_u8(reg);
                let (new, carry) = reg_value.overflowing_add(1);
                self.cpu.set_flag_to(registers::Flag::Zero, new == 0);
                self.cpu.set_flag_to(registers::Flag::Carry, carry);
                self.cpu.reset_flag(registers::Flag::AddSubtract);
                self.set_add_half_carry(reg_value, 1);
                self.cpu.write_register_u8(reg, new);
            }
            RegisterAL::Decrement(reg) => {
                let reg_value = self.cpu.read_register_u8(reg);
                let (new, carry) = reg_value.overflowing_sub(1);
                self.cpu.set_flag_to(registers::Flag::Zero, new == 0);
                self.cpu.set_flag_to(registers::Flag::Carry, carry);
                self.cpu.set_flag(registers::Flag::AddSubtract);
                self.set_sub_half_carry(reg_value, 1);
                self.cpu.write_register_u8(reg, new);
            }
            RegisterAL::Decrement16(reg) => {
                let reg_value = self.cpu.read_register_u16(reg.into());
                let (new, _carry) = reg_value.overflowing_sub(1);
                self.cpu.write_register_u16(reg.into(), new);
                self.cycle();
            }
            RegisterAL::Increment16(reg) => {
                let reg_value = self.cpu.read_register_u16(reg.into());
                let (new, _carry) = reg_value.overflowing_add(1);
                self.cpu.write_register_u16(reg.into(), new);
                self.cycle();
            }
            RegisterAL::Add16(reg) => {
                let current_value = self.cpu.read_register_u16(registers::WordRegister::HL);
                let value_to_add = self.cpu.read_register_u16(reg.into());
                let (new, carry) = current_value.overflowing_add(value_to_add);
                self.cpu.set_flag_to(registers::Flag::Zero, new == 0);
                self.cpu.set_flag_to(registers::Flag::Carry, carry);
                self.cpu.reset_flag(registers::Flag::AddSubtract);
                let has_half_carry =
                    (((current_value & 0x0FFF) + (value_to_add & 0x0FFF)) & 0xF000) != 0;
                self.cpu
                    .set_flag_to(registers::Flag::HalfCarry, has_half_carry);
                self.cpu
                    .write_register_u16(registers::WordRegister::HL, new);
                self.cycle();
            }
        };
        self.cycle();
        Ok(())
    }

    fn exec_constant_al(&mut self, op: instructions::ALOp) -> StepResult<()> {
        let arg = self.exec_read_inc_pc()?;
        let new_value = self.exec_al(op, arg);
        self.cpu.write_register_u8(br::A, new_value);
        self.cycle();
        Ok(())
    }

    fn exec_jump(&mut self, instr: instructions::Jump) -> StepResult<()> {
        use instructions::Jump;
        match instr {
            Jump::Jump(_) => {
                let addr = u16::from_le_bytes([self.exec_read_inc_pc()?, self.exec_read_inc_pc()?]);
                self.cpu.write_register_u16(wr::PC, addr);
                self.cycle();
                self.cycle();
                Ok(())
            }
            Jump::JumpIf(cond, _) => {
                let addr = u16::from_le_bytes([self.exec_read_inc_pc()?, self.exec_read_inc_pc()?]);
                if self.should_jump(cond) {
                    self.cpu.write_register_u16(wr::PC, addr);
                    self.cycle();
                }
                self.cycle();
                Ok(())
            }
            Jump::RegisterJump => {
                let addr = self.cpu.read_register_u16(wr::HL);
                self.cpu.write_register_u16(wr::PC, addr);
                self.cycle();
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
                self.cpu.write_register_u16(wr::PC, new_pc);
                self.cycle();
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

                    self.cpu.write_register_u16(wr::PC, new_pc);
                }
                self.cycle();
                Ok(())
            }
            Jump::Call(_) => {
                let low = self.exec_read_inc_pc()?;
                let high = self.exec_read_inc_pc()?;
                let addr = u16::from_le_bytes([low, high]);
                self.exec_push(self.cpu.read_register_u16(wr::PC))?;
                self.cpu.write_register_u16(wr::PC, addr);
                self.cycle();
                Ok(())
            }
            Jump::CallIf(cond, _) => {
                let low = self.exec_read_inc_pc()?;
                let high = self.exec_read_inc_pc()?;
                let addr = u16::from_le_bytes([low, high]);
                if self.should_jump(cond) {
                    self.exec_push(self.cpu.read_register_u16(wr::PC))?;
                    self.cpu.write_register_u16(wr::PC, addr);
                }
                self.cycle();
                Ok(())
            }
            Jump::CallSystem(addr_literal) => {
                let crate::types::LiteralAddress(addr) = addr_literal;
                self.exec_push(self.cpu.read_register_u16(wr::PC))?;
                self.cpu.write_register_u16(wr::PC, addr);
                self.cycle();
                Ok(())
            }
            Jump::Return => {
                let return_addr = self.exec_pop()?;
                self.cpu.write_register_u16(wr::PC, return_addr);
                self.cycle();
                self.cycle();
                Ok(())
            }
            Jump::ReturnIf(cond) => {
                if self.should_jump(cond) {
                    let return_addr = self.exec_pop()?;
                    self.cpu.write_register_u16(wr::PC, return_addr);
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
                let value = self.cpu.read_register_u16(reg.into());
                self.exec_push(value)?;
                self.cycle();
                Ok(())
            }
            Stack::Pop(reg) => {
                let val = self.exec_pop()?;
                self.cpu.write_register_u16(reg.into(), val);
                self.cycle();
                Ok(())
            }
            _ => Err(StepError::Unimplemented(instr.into())),
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
        self.cycle();
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
                self.cycle();
                Ok(())
            }
            Extended::SetMemoryBit(bit) => {
                let addr = self.cpu.read_register_u16(wr::HL);
                self.cycle();
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
                self.cycle();
                Ok(())
            }
            Extended::ResetMemoryBit(bit) => {
                let addr = self.cpu.read_register_u16(wr::HL);
                self.cycle();
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
                self.cycle();
                Ok(())
            }
            Extended::TestMemoryBit(bit) => {
                let addr = self.cpu.read_register_u16(wr::HL);
                self.cycle();
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
                self.cycle();
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
                self.cycle();
                Ok(())
            }
            Extended::SwapMemory => {
                let addr = self.cpu.read_register_u16(registers::WordRegister::HL);
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
                let value = self.cpu.read_register_u8(reg);
                let (shifted_value, carry) = match dir {
                    Left => (value << 1, value & 0x80 != 0),
                    Right => (value >> 1, value & 0x01 != 0),
                };
                self.cpu.set_flag_to(registers::Flag::Carry, carry);
                self.cpu.write_register_u8(reg, shifted_value);
                self.cycle();
                Ok(())
            }
            Extended::ShiftRightExtend(reg) => {
                let value = self.cpu.read_register_u8(reg);
                let value16 = u16::from(value);
                let extra_bit = (value16 << 1) & 0xff00;
                let shifted_value = (extra_bit + value16) >> 1;
                let actual_byte = shifted_value.to_le_bytes()[0];
                self.cpu.write_register_u8(reg, actual_byte);
                self.cycle();
                Ok(())
            }
            Extended::ShiftMemoryRightExtend => {
                let addr = self.cpu.read_register_u16(registers::WordRegister::HL);
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
                let addr = self.cpu.read_register_u16(registers::WordRegister::HL);
                self.cycle();
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
                self.cpu.invert_flag(registers::Flag::Carry);
                self.cpu.reset_flag(registers::Flag::HalfCarry);
                self.cpu.reset_flag(registers::Flag::AddSubtract);
                self.cycle();
                Ok(())
            }
            Instruction::SetCarry => {
                self.cpu.set_flag(registers::Flag::Carry);
                self.cpu.reset_flag(registers::Flag::HalfCarry);
                self.cpu.reset_flag(registers::Flag::AddSubtract);
                self.cycle();
                Ok(())
            }
            Instruction::Rotate(dir, carry) => {
                self.exec_rotate(dir, carry, br::A, SetZeroMode::Clear)
            }
            _ => Err(StepError::Unimplemented(instr)),
        }
    }

    pub fn step(&mut self) -> StepResult<()> {
        let instruction = self.current_instruction()?;
        let pc_value = self.cpu.read_register_u16(wr::PC);
        self.cpu
            .write_register_u16(wr::PC, pc_value.wrapping_add(1));
        self.exec(instruction)?;
        Ok(())
    }

    pub fn current_instruction(&self) -> StepResult<instructions::Instruction> {
        let pc_value = self.cpu.read_register_u16(wr::PC);
        let instruction_value = self.read_memory_u8(pc_value)?;
        let next_pc = pc_value.wrapping_add(1);
        let instruction = self
            .decoder
            .decode(instruction_value, &mut self.memory_iter(next_pc))?;
        Ok(instruction)
    }

    fn cycle(&mut self) {
        // TODO: Use this. a memory error can occur if the DMA operation tries to
        // write to cartridge RAM that is not present. As with actual hardware,
        // the DMA operation continues, and so we shouldn't abort emulation early,
        // but it would be useful to surface this information somewhere for ROM developers.
        let _dma_result = DmaUnit::run_cycle(self);
        self.clocks_elapsed += 4;
    }

    pub fn clocks_elapsed(&self) -> u64 {
        self.clocks_elapsed
    }
}

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
    fn test_mem_write_u8_read_u8_sysram() -> MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_memory_u8(0xc100, 0x32)?;
        assert_eq!(gb.read_memory_u8(0xc100), Ok(0x32));
        Ok(())
    }

    #[test]
    fn test_mem_write_u16_read_u16_sysram() -> MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_memory_u16(0xc100, 0x1032)?;
        assert_eq!(gb.read_memory_u16(0xc100), Ok(0x1032));
        Ok(())
    }

    #[test]
    fn test_mem_write_u8_read_u16_sysram() -> MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_memory_u8(0xc100, 0x48)?;
        gb.write_memory_u8(0xc101, 0x94)?;

        assert_eq!(gb.read_memory_u16(0xc100), Ok(0x9448));
        Ok(())
    }

    #[test]
    fn test_mem_write_u16_read_u8_sysram() -> MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_memory_u16(0xc200, 0x1345)?;

        assert_eq!(gb.read_memory_u8(0xc200), Ok(0x45));
        assert_eq!(gb.read_memory_u8(0xc201), Ok(0x13));
        Ok(())
    }

    #[test]
    fn test_write_u8_read_i8_sysram() -> MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);
        let signed_value = i8::from_le_bytes([0xa2]);

        gb.write_memory_u8(0xc200, 0xa2)?;

        assert_eq!(gb.read_memory_i8(0xc200), Ok(signed_value));
        Ok(())
    }

    #[test]
    fn test_mem_write_u8_read_u8_vram() -> MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_memory_u8(0x8100, 0x32)?;
        assert_eq!(gb.read_memory_u8(0x8100), Ok(0x32));
        Ok(())
    }

    #[test]
    fn test_mem_write_u8_read_u8_cpuram() -> MemoryResult<()> {
        let mut gb = GameBoy::new(make_cartridge(), GameBoyModel::GameBoy);

        gb.write_memory_u8(0xff80, 0x32)?;
        assert_eq!(gb.read_memory_u8(0xff80), Ok(0x32));
        Ok(())
    }
}
