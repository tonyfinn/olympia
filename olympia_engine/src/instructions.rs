use crate::registers;
use crate::types;

#[derive(PartialEq, Eq, Debug)]
#[derive(Copy, Clone)]
pub enum Condition {
    NonZero,
    Zero,
    NoCarry,
    Carry
}

#[derive(PartialEq, Eq, Debug)]
#[derive(Copy, Clone)]
pub enum Carry {
    Carry,
    NoCarry
}

#[derive(PartialEq, Eq, Debug)]
#[derive(Copy, Clone)]
pub enum RotateDirection {
    Left,
    Right
}

#[derive(PartialEq, Eq, Debug)]
#[derive(Copy, Clone)]
pub enum Increment {
    Increment,
    Decrement
}

#[derive(PartialEq, Eq, Debug)]
#[derive(Copy, Clone)]
pub enum ALOp {
    Add,
    AddCarry,
    Sub,
    SubCarry,
    And,
    Xor,
    Or,
    Compare
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Jump {
    RelativeJump(types::PCOffset), // JR r8
    RelativeJumpIf(Condition, types::PCOffset), // JR <condition>, r8
    JumpIf(Condition, types::MemoryAddress), // JP <condition>, a16
    CallIf(Condition, types::MemoryAddress), // CALL <condition>, a16
    ReturnIf(Condition), // RET <condition>
    Jump(types::MemoryAddress), // JP a16
    RegisterJump, // JP (HL)
    Call(types::MemoryAddress), // CALL a16
    Return, // RET
    ReturnInterrupt, // RETI
    CallSystem(types::MemoryAddress) // RST a16
}

impl From<Jump> for Instruction {
    fn from(jump: Jump) -> Self {
        Instruction::Jump(jump)
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum RegisterAL {
    ByteOp(ALOp, registers::ByteRegister), // <op> A, <reg>
    Add16(registers::StackRegister), // ADD HL, <reg>
    Increment(registers::ByteRegister), // INC <reg>
    Decrement(registers::ByteRegister), // DEC <reg>
    Increment16(registers::StackRegister), // INC <reg>
    Decrement16(registers::StackRegister), // DEC <reg>
}

impl From<RegisterAL> for Instruction {
    fn from(register_al: RegisterAL) -> Self {
        Instruction::RegisterAL(register_al)
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Stack {
    Push(registers::AccRegister), // PUSH <reg>
    Pop(registers::AccRegister), // POP <reg>
    AddStackPointer(types::PCOffset), // ADD SP, r8
    LoadStackOffset(types::PCOffset), // LD HL, SP+r8
    LoadStackPointer, // LD SP, HL
    StoreStackPointerMemory(types::MemoryAddress)
}

impl From<Stack> for Instruction {
    fn from(stack: Stack) -> Self {
        Instruction::Stack(stack)
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Load {
    Constant(registers::ByteRegister, u8), // LD <reg>, d8
    ConstantMemory(u8), // LD (HL), d8
    Constant16(registers::StackRegister, u16), // LD <reg>, d16
    RegisterRegister(registers::ByteRegister, registers::ByteRegister), // LD <dest>, <src>
    RegisterMemory(registers::ByteRegister, registers::WordRegister), // LD <dest>, (<src>)
    MemoryRegister(registers::WordRegister, registers::ByteRegister), // LD (<dest>), <src>
    AMemoryOffset, // LD A, (C)
    MemoryOffsetA, // LD (C), A
    AIndirect(types::MemoryAddress), // LD A, (a16)
    IndirectA(types::MemoryAddress), // LD (a16), A
    AHighOffset(types::HighAddress), // LDH A, (a8)
    HighOffsetA(types::HighAddress), // LDH (a8), A
    Increment16A(Increment), // LD (HL+), A / LD (HL-), A
    AIncrement16(Increment), // LD A, (HL+) / LD A, (HL-)
}

impl From<Load> for Instruction {
    fn from(load: Load) -> Self {
        Instruction::Load(load)
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Extended {
    Rotate(RotateDirection, Carry, registers::ByteRegister), // RLC <reg> / RRC <reg> / RL <reg> / RR <reg>
    RotateMemory(RotateDirection, Carry), // RLC (HL) / RRC (HL) / RL (HL) / RR (HL)
    ShiftHigh(RotateDirection, registers::ByteRegister), // SLA <reg> / SRA <reg>
    ShiftMemoryHigh(RotateDirection), // SLA (HL) / SRA (HL)
    Swap(registers::ByteRegister), // SWAP <reg>
    SwapMemory, // SWAP (HL)
    ShiftRightZero(registers::ByteRegister), // SRL <reg>
    ShiftMemoryRightZero, // SRL (HL)
    TestBit(u8, registers::ByteRegister), // BIT <bit>, <reg>
    TestMemoryBit(u8), // BIT <bit>, (HL)
    ResetBit(u8, registers::ByteRegister), // RES <bit>, <reg>
    ResetMemoryBit(u8), // RES <bit>, (HL)
    SetBit(u8, registers::ByteRegister), // SET <bit>, <reg>
    SetMemoryBit(u8) // SET <bit>, (HL)
}

impl From<Extended> for Instruction {
    fn from(extended: Extended) -> Self {
        Instruction::Extended(extended)
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Instruction {
    Jump(Jump),
    RegisterAL(RegisterAL),
    MemoryAL(ALOp), // <op> A, (HL)
    ConstantAL(ALOp, u8), // <op> A, d8
    MemoryIncrement(Increment), // INC (HL) / DEC (HL)
    Stack(Stack),
    Load(Load),
    Extended(Extended),
    Literal(u8), // DAT d8
    NOP, // NOP
    Stop, // STOP 0
    Halt, // HALT
    Rotate(RotateDirection, Carry), // RLCA / RLA / RRCA / RRA
    AToBCD, // DAA
    InvertA, // CPL
    SetCarry, // SCF
    InvertCarry, // CCF
    EnableInterrupts, // EI
    DisableInterrupts, // DI
}