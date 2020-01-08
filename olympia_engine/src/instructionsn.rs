mod alu;
mod extended;
mod jump;
mod load;
pub(crate) mod misc;
mod stack;

use crate::gameboy::StepResult;

use alloc::boxed::Box;
use alloc::vec::Vec;

use olympia_core::instructions::{Instruction, InstructionOpcode};

pub trait ExecutableInstruction: Instruction {
    fn execute(&self, gb: &mut crate::gameboy::GameBoy) -> StepResult<()>;
}

pub trait ExecutableOpcode {
    fn to_executable(&self, data: &mut dyn Iterator<Item = u8>) -> Box<dyn ExecutableInstruction>;
    fn all() -> Vec<(u8, Box<dyn ExecutableOpcode>)>
    where
        Self: Sized;
}

impl<T, F> ExecutableOpcode for T
where
    T: InstructionOpcode<FullInstruction = F> + 'static,
    F: ExecutableInstruction + 'static,
{
    fn to_executable(&self, data: &mut dyn Iterator<Item = u8>) -> Box<dyn ExecutableInstruction> {
        Box::new(self.build_instruction(data))
    }

    fn all() -> Vec<(u8, Box<dyn ExecutableOpcode>)> {
        let mut output = vec![];
        for opcode in Self::definition().opcodes {
            let exe_opcode: Box<dyn ExecutableOpcode> = Box::new(Self::from_opcode(*opcode));
            output.push((*opcode, exe_opcode));
        }
        output
    }
}

pub(crate) struct RuntimeDecoder {
    opcodes: Vec<Option<Box<dyn ExecutableOpcode>>>,
    extended_opcodes: Vec<Option<Box<dyn ExecutableOpcode>>>,
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

    pub fn decode(&self, value: u8) -> Option<&dyn ExecutableOpcode> {
        self.opcodes[value as usize].as_deref()
    }

    pub fn decode_extended(&self, value: u8) -> &dyn ExecutableOpcode {
        self.extended_opcodes[value as usize].as_deref().unwrap()
    }
}
