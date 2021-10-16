use std::cmp::Ordering;
use std::io;
use std::ops;

use derive_more::{Display, Error, From};
use olympia_engine::{
    gameboy,
    monitor::{parse_number, Breakpoint, BreakpointCondition, Comparison, RWTarget},
    registers::{ByteRegister as br, WordRegister as wr},
};
use structopt::StructOpt;

const PROMPT: &str = "> ";

type ByteRange = (ops::Bound<u16>, ops::Bound<u16>);

#[derive(Debug, Display, From, Error)]
pub enum RangeParseError {
    #[display(fmt = "Lower Bound Invalid")]
    LowerBoundInvalid,
    #[display(fmt = "Upper Bound Invalid")]
    UpperBoundInvalid,
    #[display(fmt = "Failed to parse range: {}", "_0")]
    ParseFailed(std::num::ParseIntError),
    #[display(fmt = "Unknown named range or missing seperator ':' for numbered range")]
    NoSeperator,
    #[display(fmt = "Invalid numbered range. Format: <start>:<end>")]
    ExtraSeperator,
}

fn parse_bound(src: &str) -> Result<ops::Bound<u16>, RangeParseError> {
    if src.is_empty() {
        Ok(ops::Bound::Unbounded)
    } else {
        let num = parse_number(src);
        match num {
            Ok(num) => Ok(ops::Bound::Included(num)),
            Err(e) => Err(RangeParseError::ParseFailed(e)),
        }
    }
}

const fn addr_bound(name: &'static str, start: u16, end: u16) -> (&str, ByteRange) {
    (
        name,
        (ops::Bound::Included(start), ops::Bound::Included(end)),
    )
}

const NAMED_BOUNDS: [(&str, ByteRange); 7] = [
    addr_bound("header", 0x0000, 0x014f),
    addr_bound("staticrom", 0x0000, 0x3fff),
    addr_bound("switchrom", 0x4000, 0x7fff),
    addr_bound("vram", 0x8000, 0x9fff),
    addr_bound("cartram", 0xa000, 0xbfff),
    addr_bound("sysram", 0xc000, 0xdfff),
    addr_bound("cpuram", 0xfe00, 0xffff),
];

fn parse_range(src: &str) -> Result<ByteRange, RangeParseError> {
    for (name, range) in NAMED_BOUNDS.iter() {
        if src == *name {
            let (start, end) = *range;
            return Ok((start, end));
        }
    }
    let bounds: Vec<_> = src.split(':').collect();
    match bounds.len().cmp(&2) {
        Ordering::Less => Err(RangeParseError::NoSeperator),
        Ordering::Greater => Err(RangeParseError::ExtraSeperator),
        Ordering::Equal => {
            let lower_bound = parse_bound(bounds.get(0).unwrap().trim())
                .map_err(|_| RangeParseError::LowerBoundInvalid)?;
            let upper_bound = parse_bound(bounds.get(1).unwrap().trim())
                .map_err(|_| RangeParseError::UpperBoundInvalid)?;
            Ok((lower_bound, upper_bound))
        }
    }
}

struct CliDebugger<'a> {
    breakpoints: Vec<Breakpoint>,
    gb: gameboy::GameBoy,
    inb: &'a mut dyn io::BufRead,
    out: &'a mut dyn io::Write,
    err: &'a mut dyn io::Write,
}

impl<'a> CliDebugger<'a> {
    fn new(
        gb: gameboy::GameBoy,
        inb: &'a mut dyn io::BufRead,
        out: &'a mut dyn io::Write,
        err: &'a mut dyn io::Write,
    ) -> CliDebugger<'a> {
        CliDebugger {
            breakpoints: Vec::new(),
            gb,
            inb,
            out,
            err,
        }
    }

    fn print_bytes(&mut self, range: ByteRange) -> io::Result<()> {
        let (min, max) = range;

        let min_address = match min {
            ops::Bound::Unbounded => 0,
            ops::Bound::Included(x) => x,
            ops::Bound::Excluded(x) => x + 1,
        };

        let max_address = match max {
            ops::Bound::Unbounded => std::u16::MAX,
            ops::Bound::Included(x) => x,
            ops::Bound::Excluded(x) => x - 1,
        };

        let mut addr = min_address;
        let mut printed_first = false;

        while addr != max_address.wrapping_add(1) || !printed_first {
            printed_first = true;
            let addr_difference = addr.wrapping_sub(min_address);
            if addr_difference % 16 == 0 {
                if addr != min_address {
                    writeln!(self.out)?;
                }
                write!(self.out, "{:04X}: ", addr)?;
            }
            let val = self
                .gb
                .get_memory_u8(addr)
                .map(|val| format!("{:02X}", val))
                .unwrap_or_else(|_| "--".to_string());
            write!(self.out, "{} ", val)?;
            addr = addr.wrapping_add(1);
        }

        writeln!(self.out)
    }

    fn print_registers(&mut self) -> io::Result<()> {
        writeln!(
            self.out,
            "A: {:02X}, F: {:02x}, AF: {:04X}",
            self.gb.read_register_u8(br::A),
            self.gb.read_register_u8(br::F),
            self.gb.read_register_u16(wr::AF)
        )?;
        writeln!(
            self.out,
            "B: {:02X}, C: {:02X}, BC: {:04X}",
            self.gb.read_register_u8(br::B),
            self.gb.read_register_u8(br::C),
            self.gb.read_register_u16(wr::BC)
        )?;
        writeln!(
            self.out,
            "D: {:02X}, E: {:02X}, DE: {:04X}",
            self.gb.read_register_u8(br::D),
            self.gb.read_register_u8(br::E),
            self.gb.read_register_u16(wr::DE)
        )?;
        writeln!(
            self.out,
            "H: {:02X}, L: {:02X}, HL: {:04X}",
            self.gb.read_register_u8(br::H),
            self.gb.read_register_u8(br::L),
            self.gb.read_register_u16(wr::HL)
        )?;
        writeln!(
            self.out,
            "SP: {:04X}, PC: {:04X}",
            self.gb.read_register_u16(wr::SP),
            self.gb.read_register_u16(wr::PC)
        )?;
        let flags_register = self.gb.read_register_u8(br::F);
        writeln!(
            self.out,
            "Flags - Zero: {}, AddSubtract: {}, HalfCarry: {}, Carry: {}",
            flags_register & 0x80 == 0,
            flags_register & 0x40 == 0,
            flags_register & 0x20 == 0,
            flags_register & 0x10 == 0
        )?;
        Ok(())
    }

    fn step(&mut self, steps: u16) -> io::Result<()> {
        for _ in 0..steps {
            match self.gb.step() {
                Ok(_) => (),
                Err(e) => writeln!(self.err, "{:?}", e)?,
            }
        }
        Ok(())
    }

    fn cycle_count(&mut self) -> io::Result<()> {
        let cycles = self.gb.clocks_elapsed();
        writeln!(self.out, "Cycles: {} / M-Cycles: {}", cycles, cycles / 4)?;
        Ok(())
    }

    fn read(&mut self, target: RWTarget) -> io::Result<()> {
        match target.read(&self.gb) {
            Ok(val) => writeln!(self.out, "{:X}", val)?,
            Err(e) => writeln!(self.err, "{}", e)?,
        };
        Ok(())
    }

    fn write(&mut self, target: RWTarget, value: u16) -> io::Result<()> {
        match target.write(&mut self.gb, value) {
            Ok(old) => writeln!(self.out, "Wrote {:X} (was {:X})", value, old)?,
            Err(e) => writeln!(self.err, "{}", e)?,
        };
        Ok(())
    }

    fn print_current(&mut self) -> io::Result<()> {
        let ci = self.gb.current_instruction();
        let disassembly = match ci {
            Ok(instr) => instr.disassemble(),
            Err(gameboy::StepError::InvalidOpcode(i)) => format!("DAT {:X}h", i),
            Err(gameboy::StepError::Memory(_)) => String::from("--"),
        };
        writeln!(self.out, "{}", disassembly)?;
        Ok(())
    }

    fn add_breakpoint(&mut self, target: RWTarget, value: u16) -> io::Result<()> {
        self.breakpoints.push(Breakpoint::new(
            target,
            BreakpointCondition::Test(Comparison::Equal, value.into()),
        ));
        writeln!(self.out, "Added breakpoint for {} == {:X}", target, value)?;
        Ok(())
    }

    fn fast_forward(&mut self) -> io::Result<()> {
        'ff: loop {
            match self.gb.step() {
                Ok(_) => (),
                Err(e) => {
                    writeln!(self.err, "Broke due to error {:?}", e)?;
                    break;
                }
            };
            for breakpoint in &self.breakpoints {
                if breakpoint.should_break(&self.gb) {
                    writeln!(self.out, "Broke on {}", breakpoint)?;
                    break 'ff;
                }
            }
        }
        Ok(())
    }

    fn debug(&mut self) -> io::Result<()> {
        loop {
            write!(self.err, "{}", PROMPT)?;
            self.err.flush()?;
            let mut input = String::new();
            let read_result = self.inb.read_line(&mut input);

            match read_result {
                Ok(0) => return Ok(()),
                Ok(_) => (),
                Err(e) => return Err(e),
            }

            let trimmed_input = input.trim();

            let parsed_command = if trimmed_input.is_empty() {
                let empty_iter: std::slice::Iter<&str> = [].iter();
                DebugCommand::from_iter_safe(empty_iter)
            } else {
                DebugCommand::from_iter_safe(trimmed_input.split(' '))
            };

            match parsed_command {
                Ok(DebugCommand::Exit) => {
                    writeln!(self.out, "Exiting")?;
                    break;
                }
                Ok(DebugCommand::PrintBytes { range }) => self.print_bytes(range)?,
                Ok(DebugCommand::PrintRegisters) => self.print_registers()?,
                Ok(DebugCommand::Step { steps }) => self.step(steps)?,
                Ok(DebugCommand::CycleCount) => self.cycle_count()?,
                Ok(DebugCommand::Read { target }) => self.read(target)?,
                Ok(DebugCommand::Write { target, value }) => self.write(target, value)?,
                Ok(DebugCommand::Breakpoint { target, value }) => {
                    self.add_breakpoint(target, value)?
                }
                Ok(DebugCommand::FastForward) => self.fast_forward()?,
                Ok(DebugCommand::Current) => self.print_current()?,
                Err(clap::Error {
                    kind: clap::ErrorKind::HelpDisplayed,
                    message,
                    ..
                }) => {
                    writeln!(self.out, "{}", message)?;
                }
                Err(clap::Error {
                    kind: clap::ErrorKind::VersionDisplayed,
                    message,
                    ..
                }) => {
                    writeln!(self.out, "{}", message)?;
                }
                Err(
                    ref
                    e
                    @
                    clap::Error {
                        kind: clap::ErrorKind::UnknownArgument,
                        ..
                    },
                ) => {
                    let command = e
                        .info
                        .as_ref()
                        .and_then(|args| args.get(0).cloned())
                        .unwrap_or_else(|| String::from(""));
                    writeln!(
                        self.err,
                        "Unknown command: {:?}. List commands with \"help\"",
                        command
                    )?;
                }
                Err(clap::Error { message, .. }) => {
                    writeln!(self.err, "{}", message)?;
                }
            }
            self.out.flush()?;
            self.err.flush()?;
        }
        Ok(())
    }
}

#[derive(StructOpt)]
#[structopt(no_version,
    global_settings=&[
        clap::AppSettings::DisableVersion,
        clap::AppSettings::DisableHelpFlags,
        clap::AppSettings::NoBinaryName
    ],
    settings = &[
        clap::AppSettings::SubcommandRequiredElseHelp
    ],
    usage="<SUBCOMMAND> [OPTIONS]"
)]
enum DebugCommand {
    /// Print out the given bytes that are mapped in the CPU's memory map. (alias: pb)
    ///
    /// You may provide a range using the syntax such as START-END, such as 2:5
    /// to print bytes 2 to 5 inclusive. You may omit either of START, END or both. For example,
    /// 200: prints all bytes from location 200, while : dumps the entire memory space.
    /// Numbers are assumed decimal by default, but you can provide hex numbers as
    /// 0x2a3 or 2a3h
    ///
    /// Bytes that cannot be read (such as addresses mapped to RAM or ROM not present)
    /// in the current cartridge, are printed as "--"
    ///
    /// Alternatively, the following named ranges can be used:
    ///
    /// header:    0x0000:0x014f
    ///
    /// staticrom: 0x0000:0x3fff
    ///
    /// switchrom: 0x4000:0x7fff
    ///
    /// vram:      0x8000:0x9fff
    ///
    /// cartram:   0xa000:0xbfff
    ///
    /// sysram:    0xc000:0xdfff
    ///
    /// cpuram:    0xfe00:0xffff
    #[structopt(no_version, alias = "pb")]
    PrintBytes {
        #[structopt(parse(try_from_str = parse_range))]
        range: ByteRange,
    },
    /// Print cycles since emulator startup (alias: cc)
    #[structopt(no_version, alias = "cc")]
    CycleCount,
    /// Prints out all registers (alias: pr)
    #[structopt(no_version, alias = "pr")]
    PrintRegisters,
    /// Run emulation as quickly as possible until a breakpoint is triggered (alias: ff)
    #[structopt(no_version, alias = "ff")]
    FastForward,
    /// Adds a breakpoint at the given location (alias: br)
    #[structopt(no_version, alias = "br")]
    Breakpoint {
        /// Can be a register such as PC or B, or a memory location such as 0x8000
        target: RWTarget,
        /// Break when the target has this value. For 8-bit registers and memory locations, must be in the range 0-FF
        #[structopt(parse(try_from_str = parse_number))]
        value: u16,
    },
    /// Steps the CPU by a specified number of cycles (alias: s)
    #[structopt(no_version, alias = "s")]
    Step {
        #[structopt(default_value = "1")]
        steps: u16,
    },
    /// Reads the given register or memory location (alias: r)
    #[structopt(no_version, alias = "r")]
    Read {
        /// Can be a register such as PC or B, or a memory location such as 0x8000
        target: RWTarget,
    },
    /// Writes the given register or memory location (alias: w)
    #[structopt(no_version, alias = "w")]
    Write {
        /// Can be a register such as PC or B, or a memory location such as 0x8000
        target: RWTarget,
        /// The value to write. For 8-bit registers and memory locations, must be in the range 0-FF
        #[structopt(parse(try_from_str = parse_number))]
        value: u16,
    },
    /// Print current instruction disassembly (alias: ci)
    #[structopt(no_version, alias = "ci")]
    Current,
    /// Exit out of this debugging session.
    #[structopt(no_version)]
    Exit,
}

pub(crate) fn debug(
    gb: gameboy::GameBoy,
    in_: &mut dyn io::Read,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> io::Result<()> {
    let mut inb = io::BufReader::new(in_);
    let mut debugger = CliDebugger::new(gb, &mut inb, out, err);
    debugger.debug()?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use olympia_engine::registers::WordRegister;
    use olympia_engine::rom;

    fn get_test_gbcpu() -> gameboy::GameBoy {
        let cartridge = rom::Cartridge {
            data: vec![0xF1u8; 0x8000],
            controller: rom::MBC2::new(5).into(),
            target: rom::TargetConsole::GameBoyOnly,
        };
        gameboy::GameBoy::new(cartridge, gameboy::GameBoyModel::GameBoy)
    }

    struct TestResult {
        output: Vec<String>,
        errors: Vec<String>,
        gb: gameboy::GameBoy,
    }

    fn assert_debug_output(gb: gameboy::GameBoy, input: &str, expected: &str) {
        let mut captured_output = Vec::new();
        let mut captured_error = Vec::new();

        debug(
            gb,
            &mut io::BufReader::new(input.as_bytes()),
            &mut captured_output,
            &mut captured_error,
        )
        .unwrap();

        let actual_output = String::from_utf8_lossy(&captured_output);
        let expected_output = String::from(expected);

        assert_eq!(actual_output, expected_output);
    }

    fn assert_debug_error_contains(gb: gameboy::GameBoy, input: &str, expected: &str) {
        let mut captured_output = Vec::new();
        let mut captured_error = Vec::new();

        debug(
            gb,
            &mut io::BufReader::new(input.as_bytes()),
            &mut captured_output,
            &mut captured_error,
        )
        .unwrap();

        let actual_error = String::from_utf8_lossy(&captured_error);
        assert!(
            actual_error.contains(expected),
            "Expected error missing. Expected error:\n\t{}\nActual error:\n\t{}\n",
            expected,
            actual_error
        );
    }

    fn run_debug_script(gb: gameboy::GameBoy, input: &[&str]) -> io::Result<TestResult> {
        let joined = input.join("\n");
        let inb = &mut io::BufReader::new(joined.as_bytes());
        let mut captured_output = Vec::new();
        let mut captured_error = Vec::new();
        let mut debugger = CliDebugger::new(gb, inb, &mut captured_output, &mut captured_error);

        debugger.debug()?;

        Ok(TestResult {
            gb: debugger.gb,
            output: String::from_utf8_lossy(&captured_output)
                .lines()
                .map(|s| s.into())
                .collect(),
            errors: String::from_utf8_lossy(&captured_error)
                .lines()
                .map(|s| s.into())
                .collect(),
        })
    }

    #[test]
    fn test_print_registers() {
        let mut gb = get_test_gbcpu();

        gb.write_register_u16(wr::AF, 0x1234);
        gb.write_register_u16(wr::BC, 0x2244);
        gb.write_register_u16(wr::DE, 0x3254);
        gb.write_register_u16(wr::HL, 0x4264);
        gb.write_register_u16(wr::PC, 0x5264);
        gb.write_register_u16(wr::SP, 0x6274);

        let expected_output = [
            // F register lower 4 bytes are not writable
            "A: 12, F: 30, AF: 1230",
            "B: 22, C: 44, BC: 2244",
            "D: 32, E: 54, DE: 3254",
            "H: 42, L: 64, HL: 4264",
            "SP: 6274, PC: 5264",
            "Flags - Zero: true, AddSubtract: true, HalfCarry: false, Carry: false\n",
        ]
        .join("\n");

        assert_debug_output(gb, "pr\n", &expected_output);
    }

    #[test]
    fn test_print_bytes() {
        let mut gb = get_test_gbcpu();

        for x in 0..=0x1fu8 {
            let addr = 0xc000 + u16::from(x);
            gb.set_memory_u8(addr, x).unwrap()
        }

        let expected_output = [
            "C000: 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F ",
            "C010: 10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F \n",
        ]
        .join("\n");

        assert_debug_output(gb, "pb 0xC000:0xC01F\n\n", &expected_output);
    }

    #[test]
    fn test_print_bytes_unmapped() {
        let gb = get_test_gbcpu();

        let expected_output = [
            "A000: -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- ",
            "A010: -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- \n",
        ]
        .join("\n");

        assert_debug_output(gb, "pb 40960:0xA01F\n", &expected_output);
    }

    #[test]
    fn test_print_bytes_unbound_min() {
        let gb = get_test_gbcpu();

        let expected_output = [
            "0000: F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 ",
            "0010: F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 \n",
        ]
        .join("\n");

        assert_debug_output(gb, "pb :0x001F\n", &expected_output);
    }

    #[test]
    fn test_print_bytes_unbound_max() {
        let mut gb = get_test_gbcpu();

        for x in 0..=0x1fu8 {
            let addr = 0xffe0 + u16::from(x);
            gb.set_memory_u8(addr, x).unwrap()
        }

        let expected_output = [
            "FFE0: 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F ",
            "FFF0: 10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F \n",
        ]
        .join("\n");

        assert_debug_output(gb, "pb 0xFFE0:\n", &expected_output);
    }

    #[test]
    fn test_print_bytes_wraparound() {
        let mut gb = get_test_gbcpu();

        for x in 0..=0x1fu8 {
            let addr = 0xffe0 + u16::from(x);
            gb.set_memory_u8(addr, x).unwrap()
        }

        let expected_output = [
            "FFF0: 10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F ",
            "0000: F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 \n",
        ]
        .join("\n");

        assert_debug_output(gb, "pb 0xFFF0:Fh\n", &expected_output);
    }

    #[test]
    fn test_print_invalid_range_extra_colon() {
        let gb = get_test_gbcpu();

        assert_debug_error_contains(
            gb,
            "pb 0:1:2\n",
            "Invalid numbered range. Format: <start>:<end>",
        );
    }

    #[test]
    fn test_print_invalid_range_no_colon() {
        let gb = get_test_gbcpu();

        assert_debug_error_contains(
            gb,
            "pb abc\n",
            "Unknown named range or missing seperator ':' for numbered range",
        );
    }

    #[test]
    fn test_unknown_command() {
        let gb = get_test_gbcpu();

        assert_debug_error_contains(
            gb,
            "unknown\n",
            "Unknown command: \"unknown\". List commands with \"help\"",
        );
    }

    #[test]
    fn test_current_instruction() {
        let mut gb = get_test_gbcpu();

        let addr = 0x8000;

        gb.write_register_u16(WordRegister::PC, addr);
        gb.set_memory_u8(addr, 0x70).unwrap(); // LD (HL), B

        assert_debug_output(gb, "ci\n", "LD (HL), B\n");
    }

    #[test]
    fn test_current_instruction_extended() {
        let mut gb = get_test_gbcpu();

        let addr = 0x8000;

        gb.write_register_u16(olympia_engine::registers::WordRegister::PC, addr);

        // RES 0, (HL)
        gb.set_memory_u8(addr, 0xCB).unwrap();
        gb.set_memory_u8(addr + 1, 0x86).unwrap();

        assert_debug_output(gb, "ci\n", "RES 0h, (HL)\n");
    }

    #[test]
    fn test_current_instruction_multibyte() {
        let mut gb = get_test_gbcpu();

        let addr = 0x8000;

        gb.write_register_u16(olympia_engine::registers::WordRegister::PC, addr);

        // LD HL, SP + -2
        gb.set_memory_u8(addr, 0xF8).unwrap();
        gb.set_memory_u8(addr + 1, 0xFE).unwrap();

        assert_debug_output(gb, "ci\n", "LD HL, SP + -2h\n");
    }

    #[test]
    fn test_current_instruction_literal() {
        let mut gb = get_test_gbcpu();

        let addr = 0x8000;

        gb.write_register_u16(olympia_engine::registers::WordRegister::PC, addr);
        gb.set_memory_u8(addr, 0xD3).unwrap();

        assert_debug_output(gb, "ci\n", "DAT D3h\n");
    }

    #[test]
    fn test_current_instruction_invalid() {
        let mut gb = get_test_gbcpu();

        let addr = 0xFF51;

        gb.write_register_u16(olympia_engine::registers::WordRegister::PC, addr);

        assert_debug_output(gb, "ci\n", "--\n");
    }

    #[test]
    fn write_reg16() {
        let mut gb = get_test_gbcpu();

        let addr = 0xFEFE;

        gb.write_register_u16(olympia_engine::registers::WordRegister::PC, addr);

        let result = run_debug_script(gb, &["write BC 0x0145", "read B", "r C"]).unwrap();

        assert_eq!(result.output, vec!["Wrote 145 (was 13)", "1", "45"]);
        assert_eq!(result.gb.read_register_u16(wr::BC), 0x0145);
    }

    #[test]
    fn read_reg16() {
        let mut gb = get_test_gbcpu();

        let addr = 0xFEFE;

        gb.write_register_u16(olympia_engine::registers::WordRegister::PC, addr);

        let result = run_debug_script(gb, &["write E 0x52", "w D 0x22", "read DE"]).unwrap();

        assert_eq!(
            result.output,
            vec!["Wrote 52 (was D8)", "Wrote 22 (was 0)", "2252"]
        );
        assert_eq!(result.gb.read_register_u16(wr::DE), 0x2252);
    }

    #[test]
    fn write_made_up_reg() {
        let mut gb = get_test_gbcpu();

        let addr = 0xFEFE;

        gb.write_register_u16(olympia_engine::registers::WordRegister::PC, addr);

        let result = run_debug_script(gb, &["write XY 0x0145"]).unwrap();

        assert!(result.errors[0].contains("XY is not a valid register or memory location"));
    }

    #[test]
    fn rw_mem() {
        let mut gb = get_test_gbcpu();

        let addr = 0xFEFE;

        gb.write_register_u16(olympia_engine::registers::WordRegister::PC, addr);

        let result = run_debug_script(gb, &["write 0x8000 0x52", "read 0x8000"]).unwrap();

        assert_eq!(result.output, vec!["Wrote 52 (was 0)", "52"]);
        assert_eq!(result.gb.get_memory_u8(0x8000).unwrap(), 0x52);
    }

    #[test]
    fn breakpoint_fast_forward() {
        let mut gb = get_test_gbcpu();

        gb.set_memory_u8(0x8000, 0x33).unwrap(); // INC SP
        gb.set_memory_u8(0x8001, 0x18).unwrap(); // JR -3
        gb.set_memory_u8(0x8002, 0xFD).unwrap();

        gb.write_register_u16(wr::PC, 0x8000);
        gb.write_register_u16(wr::SP, 0x8000);

        let result = run_debug_script(gb, &["br SP 0x8024", "ff"]).unwrap();

        assert_eq!(
            result.output,
            vec![
                "Added breakpoint for register SP == 8024",
                "Broke on Breakpoint: register SP == 8024"
            ]
        );
        assert_eq!(result.gb.read_register_u16(wr::SP), 0x8024);
    }
}
