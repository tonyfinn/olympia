use crate::registers;
use olympia_core::address;

pub use olympia_core::instructions::{ALOp, Carry, Condition, Increment, RotateDirection};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Jump {
    RegisterJump,                                      // JP (HL)
    Jump(address::LiteralAddress),                     // JP a16
    JumpIf(Condition, address::LiteralAddress),        // JP <condition>, a16
    RelativeJump(address::AddressOffset),              // JR r8
    RelativeJumpIf(Condition, address::AddressOffset), // JR <condition>, r8
    Call(address::LiteralAddress),                     // CALL a16
    CallIf(Condition, address::LiteralAddress),        // CALL <condition>, a16
    CallSystem(address::LiteralAddress),               // RST a16
    Return,                                            // RET
    ReturnIf(Condition),                               // RET <condition>
    ReturnInterrupt,                                   // RETI
}

impl From<Jump> for Instruction {
    fn from(jump: Jump) -> Self {
        Instruction::Jump(jump)
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum RegisterAL {
    ByteOp(ALOp, registers::ByteRegister), // <op> A, <reg>
    Add16(registers::StackRegister),       // ADD HL, <reg>
    Increment(registers::ByteRegister),    // INC <reg>
    Decrement(registers::ByteRegister),    // DEC <reg>
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
    Push(registers::AccRegister),                     // PUSH <reg>
    Pop(registers::AccRegister),                      // POP <reg>
    AddStackPointer(address::AddressOffset),          // ADD SP, r8
    LoadStackOffset(address::AddressOffset),          // LD HL, SP+r8
    SetStackPointer,                                  // LD SP, HL
    StoreStackPointerMemory(address::LiteralAddress), // LD (a16), SP
}

impl From<Stack> for Instruction {
    fn from(stack: Stack) -> Self {
        Instruction::Stack(stack)
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Load {
    Constant(registers::ByteRegister, u8),     // LD <reg>, d8
    ConstantMemory(u8),                        // LD (HL), d8
    Constant16(registers::StackRegister, u16), // LD <reg>, d16
    RegisterRegister(registers::ByteRegister, registers::ByteRegister), // LD <dest>, <src>
    RegisterMemory(registers::ByteRegister, registers::WordRegister), // LD <dest>, (<src>)
    MemoryRegister(registers::WordRegister, registers::ByteRegister), // LD (<dest>), <src>
    AMemoryOffset,                             // LD A, (C)
    MemoryOffsetA,                             // LD (C), A
    AIndirect(address::LiteralAddress),        // LD A, (a16)
    IndirectA(address::LiteralAddress),        // LD (a16), A
    AHighOffset(address::HighAddress),         // LDH A, (a8)
    HighOffsetA(address::HighAddress),         // LDH (a8), A
    Increment16A(Increment),                   // LD (HL+), A / LD (HL-), A
    AIncrement16(Increment),                   // LD A, (HL+) / LD A, (HL-)
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
    ShiftZero(RotateDirection, registers::ByteRegister), // SLA <reg> / SRA <reg>
    ShiftMemoryZero(RotateDirection),     // SLA (HL) / SRL (HL)
    Swap(registers::ByteRegister),        // SWAP <reg>
    SwapMemory,                           // SWAP (HL)
    ShiftRightExtend(registers::ByteRegister), // SRA <reg>
    ShiftMemoryRightExtend,               // SRA (HL)
    TestBit(u8, registers::ByteRegister), // BIT <bit>, <reg>
    TestMemoryBit(u8),                    // BIT <bit>, (HL)
    ResetBit(u8, registers::ByteRegister), // RES <bit>, <reg>
    ResetMemoryBit(u8),                   // RES <bit>, (HL)
    SetBit(u8, registers::ByteRegister),  // SET <bit>, <reg>
    SetMemoryBit(u8),                     // SET <bit>, (HL)
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
    MemoryAL(ALOp),             // <op> A, (HL)
    ConstantAL(ALOp, u8),       // <op> A, d8
    MemoryIncrement(Increment), // INC (HL) / DEC (HL)
    Stack(Stack),
    Load(Load),
    Extended(Extended),
    Literal(u8),                    // DAT d8
    NOP,                            // NOP
    Stop,                           // STOP 0
    Halt,                           // HALT
    Rotate(RotateDirection, Carry), // RLCA / RLA / RRCA / RRA
    AToBCD,                         // DAA
    InvertA,                        // CPL
    SetCarry,                       // SCF
    InvertCarry,                    // CCF
    EnableInterrupts,               // EI
    DisableInterrupts,              // DI
}
