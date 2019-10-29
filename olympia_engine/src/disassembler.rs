use alloc::string::{String, ToString};

use crate::instructions::{ALOp, Condition};
use crate::{instructions, registers, types};

pub(crate) trait Disassemble {
    fn disassemble(&self) -> String;
}

impl Disassemble for ALOp {
    fn disassemble(&self) -> String {
        match self {
            ALOp::Add => "ADD",
            ALOp::AddCarry => "ADC",
            ALOp::Sub => "SUB",
            ALOp::SubCarry => "SBC",
            ALOp::And => "AND",
            ALOp::Xor => "XOR",
            ALOp::Or => "OR",
            ALOp::Compare => "CP",
        }
        .to_string()
    }
}

impl Disassemble for Condition {
    fn disassemble(&self) -> String {
        match self {
            Condition::NonZero => "NZ",
            Condition::Zero => "Z",
            Condition::NoCarry => "NC",
            Condition::Carry => "C",
        }
        .to_string()
    }
}

impl Disassemble for types::MemoryAddress {
    fn disassemble(&self) -> String {
        let types::MemoryAddress(raw_addr) = self;
        format!("${:X}h", raw_addr)
    }
}

impl Disassemble for types::HighAddress {
    fn disassemble(&self) -> String {
        let types::HighAddress(raw_addr) = self;
        let value = i8::from_le_bytes([*raw_addr]);
        let addr = if value > 0 {
            let val_u16 = value as u16;
            0xFF00u16.wrapping_add(val_u16)
        } else {
            let val_u16 = i16::from(value).abs() as u16;
            0xFF00u16 - val_u16
        };
        format!("${:X}h", addr)
    }
}

impl Disassemble for types::PCOffset {
    fn disassemble(&self) -> String {
        let types::PCOffset(raw_addr) = self;
        format!("PC+{:X}h", raw_addr)
    }
}

impl<T> Disassemble for T
where
    T: Copy + Into<registers::WordRegister>,
{
    fn disassemble(&self) -> String {
        format!("{:?}", self.clone().into())
    }
}

impl Disassemble for registers::ByteRegister {
    fn disassemble(&self) -> String {
        format!("{:?}", self)
    }
}

impl Disassemble for instructions::Stack {
    fn disassemble(&self) -> String {
        use instructions::Stack;
        match self {
            Stack::Push(reg) => format!("PUSH {}", reg.disassemble()),
            Stack::Pop(reg) => format!("POP {}", reg.disassemble()),
            Stack::AddStackPointer(offset) => format!("ADD SP, {}", offset.disassemble()),
            Stack::LoadStackOffset(offset) => format!("LD HL, (SP+{})", offset.disassemble()),
            Stack::SetStackPointer => "LD SP, HL".into(),
            Stack::StoreStackPointerMemory(addr) => format!("LD {}, SP", addr.disassemble()),
        }
    }
}

impl Disassemble for instructions::RegisterAL {
    fn disassemble(&self) -> String {
        use instructions::RegisterAL;
        match self {
            RegisterAL::ByteOp(op, reg) => format!("{} {}", op.disassemble(), reg.disassemble()),
            RegisterAL::Add16(reg) => format!("ADD HL, {}", reg.disassemble()),
            RegisterAL::Increment(reg) => format!("INC {}", reg.disassemble()),
            RegisterAL::Decrement(reg) => format!("DEC {}", reg.disassemble()),
            RegisterAL::Increment16(reg) => format!("INC {}", reg.disassemble()),
            RegisterAL::Decrement16(reg) => format!("DEC {}", reg.disassemble()),
        }
    }
}

impl Disassemble for instructions::Jump {
    fn disassemble(&self) -> String {
        use instructions::Jump;
        match self {
            Jump::RegisterJump => "JP (HL)".into(),
            Jump::Jump(addr) => format!("JP {}", addr.disassemble()),
            Jump::JumpIf(cond, addr) => {
                format!("JP {}, {}", cond.disassemble(), addr.disassemble())
            }
            Jump::RelativeJump(offset) => format!("JR {}", offset.disassemble()),
            Jump::RelativeJumpIf(cond, offset) => {
                format!("JR {}, {}", cond.disassemble(), offset.disassemble())
            }
            Jump::Call(addr) => format!("CALL {}", addr.disassemble()),
            Jump::CallIf(cond, addr) => {
                format!("CALL {}, {}", cond.disassemble(), addr.disassemble())
            }
            Jump::CallSystem(addr) => format!("RST {}", addr.disassemble()),
            Jump::Return => "RET".into(),
            Jump::ReturnInterrupt => "RETI".into(),
            Jump::ReturnIf(cond) => format!("RET {}", cond.disassemble()),
        }
    }
}

impl Disassemble for instructions::Load {
    fn disassemble(&self) -> String {
        use instructions::Increment::{Decrement, Increment};
        use instructions::Load;
        match self {
            Load::Constant(reg, val) => format!("LD {}, {:X}h", reg.disassemble(), val),
            Load::ConstantMemory(val) => format!("LD (HL), {:X}h", val),
            Load::Constant16(reg, val) => format!("LD {}, {:X}h", reg.disassemble(), val),
            Load::RegisterRegister(dest, src) => {
                format!("LD {}, {}", dest.disassemble(), src.disassemble())
            }
            Load::RegisterMemory(dest, src) => {
                format!("LD {}, ({})", dest.disassemble(), src.disassemble())
            }
            Load::MemoryRegister(dest, src) => {
                format!("LD ({}), {}", dest.disassemble(), src.disassemble())
            }
            Load::AMemoryOffset => "LD A, (C)".into(),
            Load::MemoryOffsetA => "LD (C), A".into(),
            Load::AIndirect(addr) => format!("LD A, {}", addr.disassemble()),
            Load::IndirectA(addr) => format!("LD {}, A", addr.disassemble()),
            Load::AHighOffset(offset) => format!("LD A, {}", offset.disassemble()),
            Load::HighOffsetA(offset) => format!("LD {}, A", offset.disassemble()),
            Load::Increment16A(Increment) => "LD (HL+), A".into(),
            Load::Increment16A(Decrement) => "LD (HL-), A".into(),
            Load::AIncrement16(Increment) => "LD A, (HL+)".into(),
            Load::AIncrement16(Decrement) => "LD A, (HL-)".into(),
        }
    }
}

impl Disassemble for instructions::Extended {
    fn disassemble(&self) -> String {
        use instructions::Carry::{Carry, NoCarry};
        use instructions::Extended as ext;
        use instructions::RotateDirection::{Left, Right};

        match self {
            ext::Rotate(Left, Carry, reg) => format!("RLC {}", reg.disassemble()),
            ext::Rotate(Right, Carry, reg) => format!("RRC {}", reg.disassemble()),
            ext::Rotate(Left, NoCarry, reg) => format!("RL {}", reg.disassemble()),
            ext::Rotate(Right, NoCarry, reg) => format!("RR {}", reg.disassemble()),
            ext::RotateMemory(Left, Carry) => "RLC (HL)".into(),
            ext::RotateMemory(Right, Carry) => "RRC (HL)".into(),
            ext::RotateMemory(Left, NoCarry) => "RL (HL)".into(),
            ext::RotateMemory(Right, NoCarry) => "RR (HL)".into(),
            ext::ShiftHigh(Left, reg) => format!("SLA {}", reg.disassemble()),
            ext::ShiftHigh(Right, reg) => format!("SRA {}", reg.disassemble()),
            ext::ShiftMemoryHigh(Left) => "SLA (HL)".into(),
            ext::ShiftMemoryHigh(Right) => "SRA (HL)".into(),
            ext::ShiftRightZero(reg) => format!("SRL {}", reg.disassemble()),
            ext::ShiftMemoryRightZero => "SRL (HL)".into(),
            ext::Swap(reg) => format!("SWAP {}", reg.disassemble()),
            ext::SwapMemory => "SWAP (HL)".into(),
            ext::TestBit(bit, reg) => format!("BIT {}, {}", bit, reg.disassemble()),
            ext::SetBit(bit, reg) => format!("SET {}, {}", bit, reg.disassemble()),
            ext::ResetBit(bit, reg) => format!("RES {}, {}", bit, reg.disassemble()),
            ext::TestMemoryBit(bit) => format!("BIT {}, (HL)", bit),
            ext::SetMemoryBit(bit) => format!("SET {}, (HL)", bit),
            ext::ResetMemoryBit(bit) => format!("RES {}, (HL)", bit),
        }
    }
}

impl Disassemble for instructions::Instruction {
    fn disassemble(&self) -> String {
        use instructions::Carry::{Carry, NoCarry};
        use instructions::Increment;
        use instructions::Instruction;
        use instructions::RotateDirection::{Left, Right};

        match self {
            Instruction::NOP => "NOP".into(),
            Instruction::Stop => "STOP 0".into(),
            Instruction::Halt => "HALT".into(),
            Instruction::AToBCD => "DAA".into(),
            Instruction::InvertA => "CPL".into(),
            Instruction::SetCarry => "SCF".into(),
            Instruction::InvertCarry => "CCF".into(),
            Instruction::EnableInterrupts => "EI".into(),
            Instruction::DisableInterrupts => "DI".into(),
            Instruction::Literal(val) => format!("DAT {:X}h", val),
            Instruction::Jump(jump) => jump.disassemble(),
            Instruction::RegisterAL(reg_al) => reg_al.disassemble(),
            Instruction::Stack(stack) => stack.disassemble(),
            Instruction::Load(load) => load.disassemble(),
            Instruction::Extended(ext) => ext.disassemble(),
            Instruction::MemoryAL(op) => format!("{} (HL)", op.disassemble()),
            Instruction::ConstantAL(op, val) => format!("{} A, {:X}h", op.disassemble(), val),
            Instruction::MemoryIncrement(Increment::Increment) => "INC (HL)".into(),
            Instruction::MemoryIncrement(Increment::Decrement) => "DEC (HL)".into(),
            Instruction::Rotate(Left, Carry) => "RLCA".into(),
            Instruction::Rotate(Left, NoCarry) => "RLA".into(),
            Instruction::Rotate(Right, Carry) => "RRCA".into(),
            Instruction::Rotate(Right, NoCarry) => "RRA".into(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn assert_dissembly<I: Into<instructions::Instruction>>(instr: I, result: &str) {
        assert_eq!(instr.into().disassemble(), result);
    }

    #[test]
    fn test_basic_instructions() {
        use instructions::Instruction;
        assert_dissembly(Instruction::NOP, "NOP");
        assert_dissembly(Instruction::Stop, "STOP 0");
        assert_dissembly(Instruction::Halt, "HALT");
        assert_dissembly(Instruction::AToBCD, "DAA");
        assert_dissembly(Instruction::InvertA, "CPL");
        assert_dissembly(Instruction::SetCarry, "SCF");
        assert_dissembly(Instruction::InvertCarry, "CCF");
        assert_dissembly(Instruction::EnableInterrupts, "EI");
        assert_dissembly(Instruction::DisableInterrupts, "DI");
        assert_dissembly(Instruction::Literal(0xE3), "DAT E3h");
    }

    #[test]
    fn test_basic_al() {
        use instructions::ALOp;
        use instructions::Increment::{Decrement, Increment};
        use instructions::Instruction;

        assert_dissembly(Instruction::MemoryAL(ALOp::Add), "ADD (HL)");
        assert_dissembly(Instruction::ConstantAL(ALOp::Add, 0x20), "ADD A, 20h");
        assert_dissembly(Instruction::MemoryIncrement(Increment), "INC (HL)");
        assert_dissembly(Instruction::MemoryIncrement(Decrement), "DEC (HL)");
    }

    #[test]
    fn test_rotate_a() {
        use instructions::Carry::{Carry, NoCarry};
        use instructions::Instruction;
        use instructions::RotateDirection::{Left, Right};

        assert_dissembly(Instruction::Rotate(Left, Carry), "RLCA");
        assert_dissembly(Instruction::Rotate(Right, Carry), "RRCA");
        assert_dissembly(Instruction::Rotate(Left, NoCarry), "RLA");
        assert_dissembly(Instruction::Rotate(Right, NoCarry), "RRA");
    }

    #[test]
    fn test_disassemble_stack_push() {
        let instr = instructions::Stack::Push(registers::AccRegister::BC);

        assert_dissembly(instr, "PUSH BC");
    }

    #[test]
    fn test_disassemble_stack_pop() {
        let instr = instructions::Stack::Pop(registers::AccRegister::HL);

        assert_dissembly(instr, "POP HL");
    }

    #[test]
    fn test_disassemble_add_stack_pointer() {
        assert_dissembly(
            instructions::Stack::AddStackPointer(0x12.into()),
            "ADD SP, PC+12h",
        );
    }

    #[test]
    fn test_disassemble_load_stack_offset() {
        let instr = instructions::Stack::LoadStackOffset(0x34.into());

        assert_dissembly(instr, "LD HL, (SP+PC+34h)");
    }

    #[test]
    fn test_disassemble_set_stack_pointer() {
        let instr = instructions::Stack::SetStackPointer;

        assert_dissembly(instr, "LD SP, HL");
    }

    #[test]
    fn test_disassemble_store_stack_pointer_memory() {
        let instr = instructions::Stack::StoreStackPointerMemory(0x2634.into());

        assert_dissembly(instr, "LD $2634h, SP");
    }

    #[test]
    fn test_disassemble_register_al_byte_op() {
        use instructions::ALOp as op;
        use instructions::RegisterAL;
        use registers::ByteRegister as b;

        assert_dissembly(RegisterAL::ByteOp(op::Add, b::A), "ADD A");
        assert_dissembly(RegisterAL::ByteOp(op::AddCarry, b::B), "ADC B");
        assert_dissembly(RegisterAL::ByteOp(op::Sub, b::C), "SUB C");
        assert_dissembly(RegisterAL::ByteOp(op::SubCarry, b::D), "SBC D");
        assert_dissembly(RegisterAL::ByteOp(op::And, b::E), "AND E");
        assert_dissembly(RegisterAL::ByteOp(op::Or, b::F), "OR F");
        assert_dissembly(RegisterAL::ByteOp(op::Xor, b::H), "XOR H");
        assert_dissembly(RegisterAL::ByteOp(op::Compare, b::L), "CP L");
    }

    #[test]
    fn test_disassemble_register_al_inc_byte() {
        use instructions::RegisterAL;
        use registers::ByteRegister as b;

        assert_dissembly(RegisterAL::Increment(b::A), "INC A");
        assert_dissembly(RegisterAL::Decrement(b::B), "DEC B");
    }

    #[test]
    fn test_disassemble_register_al_word_op() {
        use instructions::RegisterAL;
        use registers::StackRegister as w;

        assert_dissembly(RegisterAL::Increment16(w::BC), "INC BC");
        assert_dissembly(RegisterAL::Decrement16(w::DE), "DEC DE");
        assert_dissembly(RegisterAL::Add16(w::SP), "ADD HL, SP");
    }

    #[test]
    fn test_jump_uncond() {
        use instructions::Jump;

        assert_dissembly(Jump::RegisterJump, "JP (HL)");
        assert_dissembly(Jump::Return, "RET");
        assert_dissembly(Jump::ReturnInterrupt, "RETI");
    }

    #[test]
    fn test_jump_uncond_addr() {
        use instructions::Jump;

        assert_dissembly(Jump::Jump(0x12.into()), "JP $12h");
        assert_dissembly(Jump::Call(0x24.into()), "CALL $24h");
        assert_dissembly(Jump::CallSystem(0x28.into()), "RST $28h");
        assert_dissembly(Jump::RelativeJump(0x15.into()), "JR PC+15h");
    }

    #[test]
    fn test_jump_cond() {
        use instructions::Condition as cond;
        use instructions::Jump;

        assert_dissembly(Jump::JumpIf(cond::Zero, 0x12.into()), "JP Z, $12h");
        assert_dissembly(Jump::CallIf(cond::NonZero, 0x24.into()), "CALL NZ, $24h");
        assert_dissembly(Jump::ReturnIf(cond::Carry), "RET C");
        assert_dissembly(
            Jump::RelativeJumpIf(cond::NoCarry, 0x15.into()),
            "JR NC, PC+15h",
        );
    }

    #[test]
    fn test_load_constant() {
        use instructions::Load;
        use registers::ByteRegister as b;
        use registers::StackRegister as sw;

        assert_dissembly(Load::Constant(b::A, 0x23), "LD A, 23h");
        assert_dissembly(Load::ConstantMemory(0x25), "LD (HL), 25h");
        assert_dissembly(Load::Constant16(sw::HL, 0x2345), "LD HL, 2345h");
    }

    #[test]
    fn test_load_move() {
        use instructions::Load;
        use registers::ByteRegister as b;
        use registers::WordRegister as w;

        assert_dissembly(Load::RegisterRegister(b::A, b::F), "LD A, F");
        assert_dissembly(Load::RegisterMemory(b::E, w::HL), "LD E, (HL)");
        assert_dissembly(Load::MemoryRegister(w::DE, b::H), "LD (DE), H");
    }

    #[test]
    fn test_load_indirect() {
        use instructions::Load;

        assert_dissembly(Load::AMemoryOffset, "LD A, (C)");
        assert_dissembly(Load::MemoryOffsetA, "LD (C), A");
        assert_dissembly(Load::AHighOffset(0x23.into()), "LD A, $FF23h");
        let offset: u8 = (-0x10i8).to_le_bytes()[0];
        assert_dissembly(Load::HighOffsetA(offset.into()), "LD $FEF0h, A");
        assert_dissembly(Load::AIndirect(0x23.into()), "LD A, $23h");
        assert_dissembly(Load::IndirectA(0x23.into()), "LD $23h, A");
    }

    #[test]
    fn test_load_increment() {
        use instructions::Increment::{Decrement, Increment};
        use instructions::Load;

        assert_dissembly(Load::Increment16A(Increment), "LD (HL+), A");
        assert_dissembly(Load::Increment16A(Decrement), "LD (HL-), A");
        assert_dissembly(Load::AIncrement16(Increment), "LD A, (HL+)");
        assert_dissembly(Load::AIncrement16(Decrement), "LD A, (HL-)");
    }

    #[test]
    fn test_extended_rotate() {
        use instructions::Carry as c;
        use instructions::Extended as ext;
        use instructions::RotateDirection as r;
        use registers::ByteRegister as b;

        assert_dissembly(ext::Rotate(r::Left, c::Carry, b::C), "RLC C");
        assert_dissembly(ext::Rotate(r::Left, c::NoCarry, b::D), "RL D");
        assert_dissembly(ext::Rotate(r::Right, c::Carry, b::E), "RRC E");
        assert_dissembly(ext::Rotate(r::Right, c::NoCarry, b::H), "RR H");
    }

    #[test]
    fn test_extended_rotate_mem() {
        use instructions::Carry as c;
        use instructions::Extended as ext;
        use instructions::RotateDirection as r;

        assert_dissembly(ext::RotateMemory(r::Left, c::Carry), "RLC (HL)");
        assert_dissembly(ext::RotateMemory(r::Left, c::NoCarry), "RL (HL)");
        assert_dissembly(ext::RotateMemory(r::Right, c::Carry), "RRC (HL)");
        assert_dissembly(ext::RotateMemory(r::Right, c::NoCarry), "RR (HL)");
    }

    #[test]
    fn test_extended_shift() {
        use instructions::Extended as ext;
        use instructions::RotateDirection as r;
        use registers::ByteRegister as b;

        assert_dissembly(ext::ShiftHigh(r::Left, b::L), "SLA L");
        assert_dissembly(ext::ShiftHigh(r::Right, b::A), "SRA A");
        assert_dissembly(ext::ShiftMemoryHigh(r::Left), "SLA (HL)");
        assert_dissembly(ext::ShiftMemoryHigh(r::Right), "SRA (HL)");
        assert_dissembly(ext::ShiftRightZero(b::B), "SRL B");
        assert_dissembly(ext::ShiftMemoryRightZero, "SRL (HL)");
    }

    #[test]
    fn test_extended_swap() {
        use instructions::Extended as ext;
        use registers::ByteRegister as b;

        assert_dissembly(ext::Swap(b::L), "SWAP L");
        assert_dissembly(ext::SwapMemory, "SWAP (HL)");
    }

    #[test]
    fn test_extended_bit_op() {
        use instructions::Extended as ext;
        use registers::ByteRegister as b;

        assert_dissembly(ext::SetBit(0, b::A), "SET 0, A");
        assert_dissembly(ext::ResetBit(1, b::B), "RES 1, B");
        assert_dissembly(ext::TestBit(2, b::C), "BIT 2, C");
        assert_dissembly(ext::SetMemoryBit(0), "SET 0, (HL)");
        assert_dissembly(ext::ResetMemoryBit(1), "RES 1, (HL)");
        assert_dissembly(ext::TestMemoryBit(2), "BIT 2, (HL)");
    }
}
