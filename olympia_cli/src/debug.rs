use std::fmt;
use std::io;
use std::io::prelude::*;
use std::ops;

use olympia_engine::disassembler::Disassemble;
use olympia_engine::gameboy;
use olympia_engine::registers::{ByteRegister as br, WordRegister as wr};
use structopt::StructOpt;

const PROMPT: &str = "> ";

type ByteRange = (ops::Bound<u16>, ops::Bound<u16>);

#[derive(Debug)]
pub enum RangeParseError {
    LowerBoundInvalid,
    UpperBoundInvalid,
    ParseFailed(std::num::ParseIntError),
    NoSeperator,
    ExtraSeperator,
}

impl std::error::Error for RangeParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let RangeParseError::ParseFailed(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

impl fmt::Display for RangeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use RangeParseError::*;
        match self {
            LowerBoundInvalid => write!(f, "Lower bound invalid"),
            UpperBoundInvalid => write!(f, "Upper bound invalid"),
            ParseFailed(e) => write!(f, "Failed to parse range: {}", e),
            NoSeperator => write!(f, "No seperator ':' found"),
            ExtraSeperator => write!(f, "Too many seperators ':' found"),
        }
    }
}

fn parse_number(src: &str) -> Result<u16, std::num::ParseIntError> {
    let lowered = src.to_lowercase();
    if lowered.starts_with("0x") {
        u16::from_str_radix(&src[2..], 16)
    } else if lowered.ends_with('h') {
        u16::from_str_radix(&src[..src.len() - 1], 16)
    } else {
        src.parse()
    }
}

fn parse_bound(src: &str) -> Result<ops::Bound<u16>, RangeParseError> {
    if src == "" {
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
    if bounds.len() > 2 {
        Err(RangeParseError::ExtraSeperator)
    } else if bounds.len() < 2 {
        Err(RangeParseError::NoSeperator)
    } else {
        let lower_bound = parse_bound(bounds.get(0).unwrap().trim())
            .map_err(|_| RangeParseError::LowerBoundInvalid)?;
        let upper_bound = parse_bound(bounds.get(1).unwrap().trim())
            .map_err(|_| RangeParseError::UpperBoundInvalid)?;
        Ok((lower_bound, upper_bound))
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
    /// Print cycles since emulator startup (alias cc)
    #[structopt(no_version, alias = "cc")]
    CycleCount,
    /// Prints out all registers (alias: pr)
    #[structopt(no_version, alias = "pr")]
    PrintRegisters,
    /// Steps the CPU by a specified number of cycles (alias: s)
    #[structopt(no_version, alias = "s")]
    Step {
        #[structopt(default_value = "1")]
        steps: u16,
    },
    /// Print current instruction disassembly (alias ci)
    #[structopt(no_version, alias = "ci")]
    Current,
    /// Exit out of this debugging session.
    #[structopt(no_version)]
    Exit,
}

fn print_bytes(gb: &gameboy::GameBoy, range: ByteRange, out: &mut dyn io::Write) -> io::Result<()> {
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
                writeln!(out)?;
            }
            write!(out, "{:04X}: ", addr)?;
        }
        let val = gb
            .read_memory_u8(addr)
            .map(|val| format!("{:02X}", val))
            .unwrap_or_else(|_| "--".to_string());
        write!(out, "{} ", val)?;
        addr = addr.wrapping_add(1);
    }

    writeln!(out)
}

fn print_registers(gb: &gameboy::GameBoy, out: &mut dyn io::Write) -> io::Result<()> {
    writeln!(
        out,
        "A: {:02X}, F: {:02x}, AF: {:04X}",
        gb.read_register_u8(br::A),
        gb.read_register_u8(br::F),
        gb.read_register_u16(wr::AF)
    )?;
    writeln!(
        out,
        "B: {:02X}, C: {:02X}, BC: {:04X}",
        gb.read_register_u8(br::B),
        gb.read_register_u8(br::C),
        gb.read_register_u16(wr::BC)
    )?;
    writeln!(
        out,
        "D: {:02X}, E: {:02X}, DE: {:04X}",
        gb.read_register_u8(br::D),
        gb.read_register_u8(br::E),
        gb.read_register_u16(wr::DE)
    )?;
    writeln!(
        out,
        "H: {:02X}, L: {:02X}, HL: {:04X}",
        gb.read_register_u8(br::H),
        gb.read_register_u8(br::L),
        gb.read_register_u16(wr::HL)
    )?;
    writeln!(
        out,
        "SP: {:04X}, PC: {:04X}",
        gb.read_register_u16(wr::SP),
        gb.read_register_u16(wr::PC)
    )?;
    let flags_register = gb.read_register_u8(br::F);
    writeln!(
        out,
        "Flags - Zero: {}, AddSubtract: {}, HalfCarry: {}, Carry: {}",
        flags_register & 0x80 == 0,
        flags_register & 0x40 == 0,
        flags_register & 0x20 == 0,
        flags_register & 0x10 == 0
    )?;
    Ok(())
}

pub(crate) fn debug(
    mut gb: gameboy::GameBoy,
    in_: &mut dyn io::Read,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> io::Result<()> {
    let mut inb = io::BufReader::new(in_);
    loop {
        write!(err, "{}", PROMPT)?;
        err.flush()?;
        let mut input = String::new();
        let read_result = inb.read_line(&mut input);

        match read_result {
            Ok(0) => return Ok(()),
            Ok(_) => (),
            Err(e) => return Err(e),
        }

        let trimmed_input = input.trim();

        let parsed_command = if trimmed_input == "" {
            let empty_iter: std::slice::Iter<&str> = [].iter();
            DebugCommand::from_iter_safe(empty_iter)
        } else {
            DebugCommand::from_iter_safe(trimmed_input.split(' '))
        };

        match parsed_command {
            Ok(DebugCommand::Exit) => {
                writeln!(out, "Exiting")?;
                break;
            }
            Ok(DebugCommand::PrintBytes { range }) => {
                let result = print_bytes(&gb, range, out);
                match result {
                    Ok(_) => (),
                    Err(e) => writeln!(err, "{}", e)?,
                };
            }
            Ok(DebugCommand::PrintRegisters) => {
                let result = print_registers(&gb, out);
                match result {
                    Ok(_) => (),
                    Err(e) => writeln!(err, "{}", e)?,
                };
            }
            Ok(DebugCommand::Step { steps }) => {
                for _ in 0..steps {
                    match gb.step() {
                        Ok(_) => (),
                        Err(e) => writeln!(err, "{:?}", e)?,
                    }
                }
            }
            Ok(DebugCommand::CycleCount) => {
                let cycles = gb.clocks_elapsed();
                writeln!(out, "Cycles: {} / M-Cycles: {}", cycles, cycles / 4)?;
            }
            Ok(DebugCommand::Current) => {
                let disassembly = gb.current_instruction().unwrap().disassemble();
                writeln!(out, "{}", disassembly)?;
            }
            Err(clap::Error {
                kind: clap::ErrorKind::HelpDisplayed,
                message,
                ..
            }) => {
                writeln!(out, "{}", message)?;
            }
            Err(clap::Error {
                kind: clap::ErrorKind::VersionDisplayed,
                message,
                ..
            }) => {
                writeln!(out, "{}", message)?;
            }
            Err(
                ref e @ clap::Error {
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
                    err,
                    "Unknown command: {:?}. List commands with \"help\"",
                    command
                )?;
            }
            Err(clap::Error { message, .. }) => {
                writeln!(err, "{}", message)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use olympia_engine::rom;

    fn get_test_gbcpu() -> gameboy::GameBoy {
        let cartridge = rom::Cartridge {
            data: vec![0xF1u8; 0x8000],
            controller: rom::MBC2::default().into(),
            target: rom::TargetConsole::GameBoyOnly,
        };
        gameboy::GameBoy::new(cartridge, gameboy::GameBoyModel::GameBoy)
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

        let input = "pr\n";
        let mut captured_output = Vec::new();

        debug(
            gb,
            &mut io::BufReader::new(input.as_bytes()),
            &mut captured_output,
            &mut io::sink(),
        )
        .unwrap();

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

        let actual_output = String::from_utf8_lossy(&captured_output);

        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_print_bytes() {
        let mut gb = get_test_gbcpu();

        for x in 0..=0x1fu8 {
            let addr = 0xc000 + u16::from(x);
            gb.write_memory_u8(addr, x).unwrap()
        }

        let input = "pb 0xC000:0xC01F\n\n";
        let mut captured_output = Vec::new();

        debug(
            gb,
            &mut io::BufReader::new(input.as_bytes()),
            &mut captured_output,
            &mut io::sink(),
        )
        .unwrap();

        let expected_output = [
            "C000: 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F ",
            "C010: 10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F \n",
        ]
        .join("\n");

        let actual_output = String::from_utf8_lossy(&captured_output);

        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_print_bytes_unmapped() {
        let gb = get_test_gbcpu();

        let input = "pb 40960:0xA01F\n";
        let mut captured_output = Vec::new();

        debug(
            gb,
            &mut io::BufReader::new(input.as_bytes()),
            &mut captured_output,
            &mut io::sink(),
        )
        .unwrap();

        let expected_output = [
            "A000: -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- ",
            "A010: -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- \n",
        ]
        .join("\n");

        let actual_output = String::from_utf8_lossy(&captured_output);

        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_print_bytes_unbound_min() {
        let gb = get_test_gbcpu();

        let input = "pb :0x001F\n";
        let mut captured_output = Vec::new();

        debug(
            gb,
            &mut io::BufReader::new(input.as_bytes()),
            &mut captured_output,
            &mut io::sink(),
        )
        .unwrap();

        let expected_output = [
            "0000: F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 ",
            "0010: F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 \n",
        ]
        .join("\n");

        let actual_output = String::from_utf8_lossy(&captured_output);

        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_print_bytes_unbound_max() {
        let mut gb = get_test_gbcpu();

        for x in 0..=0x1fu8 {
            let addr = 0xffe0 + u16::from(x);
            gb.write_memory_u8(addr, x).unwrap()
        }

        let input = "pb 0xFFE0:\n";
        let mut captured_output = Vec::new();

        debug(
            gb,
            &mut io::BufReader::new(input.as_bytes()),
            &mut captured_output,
            &mut io::sink(),
        )
        .unwrap();

        let expected_output = [
            "FFE0: 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F ",
            "FFF0: 10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F \n",
        ]
        .join("\n");

        let actual_output = String::from_utf8_lossy(&captured_output);

        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_print_bytes_wraparound() {
        let mut gb = get_test_gbcpu();

        for x in 0..=0x1fu8 {
            let addr = 0xffe0 + u16::from(x);
            gb.write_memory_u8(addr, x).unwrap()
        }

        let input = "pb 0xFFF0:Fh\n";
        let mut captured_output = Vec::new();

        debug(
            gb,
            &mut io::BufReader::new(input.as_bytes()),
            &mut captured_output,
            &mut io::sink(),
        )
        .unwrap();

        let expected_output = [
            "FFF0: 10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F ",
            "0000: F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 F1 \n",
        ]
        .join("\n");

        let actual_output = String::from_utf8_lossy(&captured_output);

        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_unknown_command() {
        let mut gb = get_test_gbcpu();

        for x in 0..=0x1fu8 {
            let addr = 0xffe0 + u16::from(x);
            gb.write_memory_u8(addr, x).unwrap()
        }

        let input = "unknown\n";
        let mut captured_output = Vec::new();
        let mut captured_error = Vec::new();

        debug(
            gb,
            &mut io::BufReader::new(input.as_bytes()),
            &mut captured_output,
            &mut captured_error,
        )
        .unwrap();

        let expected_error =
            String::from("> Unknown command: \"unknown\". List commands with \"help\"\n> ");

        let actual_error = String::from_utf8_lossy(&captured_error);

        assert_eq!(actual_error, expected_error);
    }
}