use crate::{
    address,
    gameboy::cpu::InterruptState,
    gameboy::{GameBoy, StepResult},
    instructions::Condition,
    instructionsn::{ExecutableInstruction, ExecutableOpcode},
    registers,
};

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::convert::TryFrom;

use olympia_derive::OlympiaInstruction;

fn should_jump(gb: &GameBoy, cond: Condition) -> bool {
    match cond {
        Condition::Zero => gb.read_flag(registers::Flag::Zero),
        Condition::NonZero => !gb.read_flag(registers::Flag::Zero),
        Condition::Carry => gb.read_flag(registers::Flag::Carry),
        Condition::NoCarry => !gb.read_flag(registers::Flag::Carry),
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1100_0011, label = "JP")]
struct Jump {
    #[olympia(single)]
    dest: address::LiteralAddress,
}

impl ExecutableInstruction for Jump {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        gb.set_pc(self.dest);
        gb.cycle();
        Ok(())
    }
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x110A_A010, label = "JP")]
struct JumpIf {
    #[olympia(dest, mask = 0xA)]
    cond: Condition,
    #[olympia(src)]
    dest: address::LiteralAddress,
}

impl ExecutableInstruction for JumpIf {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        if should_jump(gb, self.cond) {
            gb.set_pc(self.dest);
            gb.cycle();
        }
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1110_1001, label = "JP")]
struct JumpRegister {
    #[olympia(single, constant(registers::WordRegister::HL))]
    dest: registers::WordRegister,
}

impl ExecutableInstruction for JumpRegister {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        gb.set_pc(gb.read_register_u16(self.dest));
        Ok(())
    }
}

fn relative_jump(gb: &mut GameBoy, offset: i8) {
    let pc = gb.read_register_u16(registers::WordRegister::PC);
    let new_pc = if offset > 0 {
        pc.wrapping_add(u16::try_from(offset).unwrap())
    } else {
        pc.wrapping_sub(u16::try_from(offset.abs()).unwrap())
    };
    gb.cycle();
    gb.set_pc(new_pc);
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x0001_1000, label = "JR")]
struct RelativeJump {
    #[olympia(single)]
    offset: i8,
}

impl ExecutableInstruction for RelativeJump {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        relative_jump(gb, self.offset);
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x001A_A000, label = "JR")]
struct RelativeJumpIf {
    #[olympia(dest, mask = 0xA)]
    cond: Condition,
    #[olympia(src)]
    offset: i8,
}

impl ExecutableInstruction for RelativeJumpIf {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        if should_jump(gb, self.cond) {
            relative_jump(gb, self.offset);
        }
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1100_1101, label = "CALL")]
struct Call {
    #[olympia(single)]
    dest: address::LiteralAddress,
}

impl ExecutableInstruction for Call {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        gb.exec_push(gb.read_pc())?;
        gb.set_pc(self.dest);
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x110A_A100, label = "CALL")]
struct CallIf {
    #[olympia(dest, mask = 0xA)]
    cond: Condition,
    #[olympia(src)]
    dest: address::LiteralAddress,
}

impl ExecutableInstruction for CallIf {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        if should_jump(gb, self.cond) {
            gb.exec_push(gb.read_pc())?;
            gb.set_pc(self.dest);
        }
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x11AA_A111, label = "RST")]
struct CallSystem {
    #[olympia(single, mask = 0xA)]
    dest: u8,
}

impl ExecutableInstruction for CallSystem {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        gb.exec_push(gb.read_pc())?;
        gb.set_pc(u16::from(self.dest) << 3);
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1100_1001, label = "RET")]
struct Return {}

impl ExecutableInstruction for Return {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let return_addr: address::LiteralAddress = gb.exec_pop()?;
        gb.set_pc(return_addr);
        gb.cycle();
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1101_1001, label = "RETI")]
struct ReturnInterrupt {}

impl ExecutableInstruction for ReturnInterrupt {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let return_addr: address::LiteralAddress = gb.exec_pop()?;
        gb.set_pc(return_addr);
        gb.set_interrupt_state(InterruptState::Enabled);
        gb.cycle();
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x110A_A000, label = "RET")]
struct ReturnIf {
    #[olympia(dest, mask = 0xA)]
    cond: Condition,
}

impl ExecutableInstruction for ReturnIf {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        if should_jump(gb, self.cond) {
            let return_addr: address::LiteralAddress = gb.exec_pop()?;
            gb.set_pc(return_addr);
            gb.cycle();
        }
        gb.cycle();
        Ok(())
    }
}

pub(crate) fn opcodes() -> Vec<(u8, Box<dyn ExecutableOpcode>)> {
    vec![
        JumpOpcode::all(),
        JumpIfOpcode::all(),
        JumpRegisterOpcode::all(),
        RelativeJumpOpcode::all(),
        RelativeJumpIfOpcode::all(),
        CallOpcode::all(),
        CallIfOpcode::all(),
        CallSystemOpcode::all(),
        ReturnOpcode::all(),
        ReturnInterruptOpcode::all(),
        ReturnIfOpcode::all(),
    ]
    .into_iter()
    .flatten()
    .collect()
}
