mod alu;
mod extended;
mod jump;
mod load;
pub(crate) mod misc;
mod stack;

use crate::disasm::Disassemble;
use crate::gameboy::StepResult;

use alloc::boxed::Box;
use alloc::vec::Vec;

use olympia_core::instructions::{Instruction, InstructionOpcode, SerializableInstruction};

pub trait ExecutableInstruction: Instruction {
    fn execute(&self, gb: &mut crate::gameboy::GameBoy) -> StepResult<()>;
}

pub trait RuntimeOpcode {
    fn to_instruction(&self, data: &mut dyn Iterator<Item = u8>) -> Box<dyn RuntimeInstruction>;
    fn all() -> Vec<(u8, Box<dyn RuntimeOpcode>)>
    where
        Self: Sized;
}

impl<T, F> RuntimeOpcode for T
where
    T: InstructionOpcode<FullInstruction = F> + 'static,
    F: RuntimeInstruction + 'static,
{
    fn to_instruction(&self, data: &mut dyn Iterator<Item = u8>) -> Box<dyn RuntimeInstruction> {
        Box::new(self.build_instruction(data))
    }

    fn all() -> Vec<(u8, Box<dyn RuntimeOpcode>)> {
        let mut output = vec![];
        for opcode in Self::definition().opcodes {
            let exe_opcode: Box<dyn RuntimeOpcode> = Box::new(Self::from_opcode(*opcode));
            output.push((*opcode, exe_opcode));
        }
        output
    }
}

pub trait RuntimeInstruction
where
    Self: ExecutableInstruction,
    Self: SerializableInstruction,
    Self: Disassemble,
{
}

impl<T> RuntimeInstruction for T
where
    T: ExecutableInstruction,
    T: SerializableInstruction,
    T: Disassemble,
{
}

pub struct RuntimeDecoder {
    opcodes: Vec<Option<Box<dyn RuntimeOpcode>>>,
    extended_opcodes: Vec<Option<Box<dyn RuntimeOpcode>>>,
}

impl RuntimeDecoder {
    pub fn new() -> RuntimeDecoder {
        let mut opcodes = Vec::with_capacity(256);
        let mut extended_opcodes = Vec::with_capacity(256);
        for _ in 0..256 {
            opcodes.push(None);
            extended_opcodes.push(None);
        }
        let input_codes = stack::opcodes()
            .into_iter()
            .chain(alu::opcodes())
            .chain(jump::opcodes())
            .chain(misc::opcodes())
            .chain(load::opcodes());

        for (value, executable) in input_codes {
            opcodes[value as usize] = Some(executable);
        }

        for (value, executable) in extended::opcodes() {
            extended_opcodes[value as usize] = Some(executable);
        }

        RuntimeDecoder {
            opcodes,
            extended_opcodes,
        }
    }

    pub fn is_extended(&self, value: u8) -> bool {
        value == 0xCB
    }

    pub fn decode(&self, value: u8) -> Option<&dyn RuntimeOpcode> {
        self.opcodes[value as usize].as_deref()
    }

    pub fn decode_from_iter(
        &self,
        val: u8,
        iter: &mut dyn Iterator<Item = u8>,
    ) -> Option<Box<dyn RuntimeInstruction>> {
        if self.is_extended(val) {
            iter.next()
                .map(|ex| self.decode_extended(ex).to_instruction(iter))
        } else if let Some(opcode) = self.decode(val) {
            Some(opcode.to_instruction(iter))
        } else {
            None
        }
    }

    pub fn decode_extended(&self, value: u8) -> &dyn RuntimeOpcode {
        self.extended_opcodes[value as usize].as_deref().unwrap()
    }
}

#[cfg(test)]
mod alu_tests;

#[cfg(test)]
mod disassembly_tests;

#[cfg(test)]
mod extended_opcode_tests;

#[cfg(test)]
mod jump_tests;

#[cfg(test)]
mod interrupt_tests;

#[cfg(test)]
mod load_tests;

#[cfg(test)]
mod stack_tests;

#[cfg(test)]
mod misc_tests;
