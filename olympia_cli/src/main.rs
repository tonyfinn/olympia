use std::error;
use std::fmt;
use std::io;
use std::io::prelude::*;
use std::ops;
use std::path::PathBuf;

use olympia_engine::gameboy::cpu;
use olympia_engine::rom;
use structopt::StructOpt;

const PROMPT: &str = "> ";

#[derive(Debug)]
enum OlympiaError {
    Io(std::io::Error),
    Cartridge(rom::CartridgeError),
}

impl From<std::io::Error> for OlympiaError {
    fn from(err: std::io::Error) -> Self {
        OlympiaError::Io(err)
    }
}

impl From<rom::CartridgeError> for OlympiaError {
    fn from(err: rom::CartridgeError) -> Self {
        OlympiaError::Cartridge(err)
    }
}

impl fmt::Display for OlympiaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use OlympiaError::*;
        match self {
            Cartridge(e) => write!(f, "Cartridge error: {}", e),
            Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl error::Error for OlympiaError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use OlympiaError::*;
        match self {
            Cartridge(e) => Some(e),
            Io(e) => Some(e),
        }
    }
}

type OlympiaResult<T> = Result<T, OlympiaError>;
type ByteRange = (ops::Bound<u16>, ops::Bound<u16>);

#[derive(Debug, StructOpt)]
enum OlympiaCommand {
    RomInfo {
        #[structopt(parse(from_os_str))]
        rom: PathBuf,
    },
    Debug {
        #[structopt(parse(from_os_str))]
        rom: PathBuf,
    },
}

#[derive(Debug, StructOpt)]
#[structopt(name = "olympia-cli", about = "Load and debug a GB ROM")]
struct OlympiaArgs {
    #[structopt(short = "q", long)]
    /// Do not produce user facing input (e.q. for scripted use)
    quiet: bool,
    #[structopt(subcommand)]
    cmd: OlympiaCommand,
}

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
    /// Prints out all registers (alias: pr)
    #[structopt(no_version, alias = "pr")]
    PrintRegisters,
    /// Exit out of this debugging session.
    #[structopt(no_version)]
    Exit,
}

fn print_rom_info(cartridge: rom::Cartridge, out: &mut dyn io::Write) -> OlympiaResult<()> {
    write!(out, "Cartridge Type: ")?;
    match cartridge.controller {
        rom::CartridgeEnum::StaticRom(_srom) => writeln!(out, "Static ROM")?,
        rom::CartridgeEnum::Type1(mbc1) => {
            writeln!(out, "MBC1")?;
            writeln!(
                out,
                "RAM Size: {}KiB",
                rom::CartridgeType::ram_size(&mbc1) / 1024
            )?
        }
        rom::CartridgeEnum::Type2(_mbc2) => {
            writeln!(out, "MBC2")?;
            writeln!(out, "RAM Size: 512 x 4 bits")?
        }
    }

    write!(out, "ROM Size: {}KiB", cartridge.data.len() / 1024)?;
    Ok(())
}

fn print_bytes(cpu: &cpu::GameBoy, range: ByteRange, out: &mut dyn io::Write) -> io::Result<()> {
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
        let val = cpu
            .read_memory_u8(addr)
            .map(|val| format!("{:02X}", val))
            .unwrap_or_else(|_| "--".to_string());
        write!(out, "{} ", val)?;
        addr = addr.wrapping_add(1);
    }

    writeln!(out)
}

fn print_registers(cpu: &cpu::GameBoy, out: &mut dyn io::Write) -> io::Result<()> {
    writeln!(
        out,
        "A: {:02X}, F: {:02X}, AF: {:04X}",
        cpu.read_register_u8(cpu::ByteRegister::A),
        cpu.read_register_u8(cpu::ByteRegister::F),
        cpu.read_register_u16(cpu::WordRegister::AF)
    )?;
    writeln!(
        out,
        "B: {:02X}, C: {:02X}, BC: {:04X}",
        cpu.read_register_u8(cpu::ByteRegister::B),
        cpu.read_register_u8(cpu::ByteRegister::C),
        cpu.read_register_u16(cpu::WordRegister::BC)
    )?;
    writeln!(
        out,
        "D: {:02X}, E: {:02X}, DE: {:04X}",
        cpu.read_register_u8(cpu::ByteRegister::D),
        cpu.read_register_u8(cpu::ByteRegister::E),
        cpu.read_register_u16(cpu::WordRegister::DE)
    )?;
    writeln!(
        out,
        "H: {:02X}, L: {:02X}, HL: {:04X}",
        cpu.read_register_u8(cpu::ByteRegister::H),
        cpu.read_register_u8(cpu::ByteRegister::L),
        cpu.read_register_u16(cpu::WordRegister::HL)
    )?;
    writeln!(
        out,
        "SP: {:04X}, PC: {:04X}",
        cpu.read_register_u16(cpu::WordRegister::SP),
        cpu.read_register_u16(cpu::WordRegister::PC)
    )?;
    Ok(())
}

fn debug(
    gb: cpu::GameBoy,
    in_: &mut dyn io::Read,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> OlympiaResult<()> {
    let mut inb = io::BufReader::new(in_);
    loop {
        write!(err, "{}", PROMPT)?;
        err.flush()?;
        let mut input = String::new();
        let read_result = inb.read_line(&mut input);

        match read_result {
            Ok(0) => return Ok(()),
            Ok(_) => (),
            Err(e) => return Err(OlympiaError::Io(e)),
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

fn find_err_out(args: &OlympiaArgs) -> Box<dyn io::Write> {
    if cfg!(unix) {
        use std::os::unix::fs::FileTypeExt;
        let pid = std::process::id();
        let fd_path = format!("/proc/{}/fd/0", pid);
        let metadata_result = std::fs::metadata(fd_path);
        if let Ok(metadata) = metadata_result {
            if !metadata.file_type().is_char_device() {
                return Box::new(io::sink());
            }
        }
    }
    let err: Box<dyn io::Write> = if args.quiet {
        Box::new(io::sink())
    } else {
        Box::new(io::stderr())
    };
    err
}

fn parse_cartridge(rom_path: &PathBuf) -> OlympiaResult<rom::Cartridge> {
    let data = std::fs::read(rom_path)?;
    let cartridge = rom::Cartridge::from_data(data)?;
    Ok(cartridge)
}

fn main() -> OlympiaResult<()> {
    let args = OlympiaArgs::from_args();
    let mut err = find_err_out(&args);
    match args.cmd {
        OlympiaCommand::RomInfo { rom } => {
            print_rom_info(parse_cartridge(&rom)?, &mut io::stdout())?
        }
        OlympiaCommand::Debug { rom } => debug(
            cpu::GameBoy::new(parse_cartridge(&rom)?),
            &mut io::stdin(),
            &mut io::stdout(),
            err.as_mut(),
        )?,
    }
    Ok(())
}

#[cfg(test)]
pub mod test {
    use super::*;

    fn get_test_gbcpu() -> cpu::GameBoy {
        let cartridge = rom::Cartridge {
            data: vec![0xF1u8; 0x8000],
            controller: rom::MBC2::default().into(),
        };
        cpu::GameBoy::new(cartridge)
    }

    #[test]
    fn test_print_registers() {
        let mut cpu = get_test_gbcpu();

        cpu.write_register_u16(cpu::WordRegister::AF, 0x1234);
        cpu.write_register_u16(cpu::WordRegister::BC, 0x2244);
        cpu.write_register_u16(cpu::WordRegister::DE, 0x3254);
        cpu.write_register_u16(cpu::WordRegister::HL, 0x4264);
        cpu.write_register_u16(cpu::WordRegister::PC, 0x5264);
        cpu.write_register_u16(cpu::WordRegister::SP, 0x6274);

        let input = "pr\n";
        let mut captured_output = Vec::new();

        debug(
            cpu,
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
            "SP: 6274, PC: 5264\n",
        ]
        .join("\n");

        let actual_output = String::from_utf8_lossy(&captured_output);

        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_print_bytes() {
        let mut cpu = get_test_gbcpu();

        for x in 0..=0x1fu8 {
            let addr = 0xc000 + u16::from(x);
            cpu.write_memory_u8(addr, x).unwrap()
        }

        let input = "pb 0xC000:0xC01F\n\n";
        let mut captured_output = Vec::new();

        debug(
            cpu,
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
        let cpu = get_test_gbcpu();

        let input = "pb 40960:0xA01F\n";
        let mut captured_output = Vec::new();

        debug(
            cpu,
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
        let cpu = get_test_gbcpu();

        let input = "pb :0x001F\n";
        let mut captured_output = Vec::new();

        debug(
            cpu,
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
        let mut cpu = get_test_gbcpu();

        for x in 0..=0x1fu8 {
            let addr = 0xffe0 + u16::from(x);
            cpu.write_memory_u8(addr, x).unwrap()
        }

        let input = "pb 0xFFE0:\n";
        let mut captured_output = Vec::new();

        debug(
            cpu,
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
        let mut cpu = get_test_gbcpu();

        for x in 0..=0x1fu8 {
            let addr = 0xffe0 + u16::from(x);
            cpu.write_memory_u8(addr, x).unwrap()
        }

        let input = "pb 0xFFF0:Fh\n";
        let mut captured_output = Vec::new();

        debug(
            cpu,
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
        let mut cpu = get_test_gbcpu();

        for x in 0..=0x1fu8 {
            let addr = 0xffe0 + u16::from(x);
            cpu.write_memory_u8(addr, x).unwrap()
        }

        let input = "unknown\n";
        let mut captured_output = Vec::new();
        let mut captured_error = Vec::new();

        debug(
            cpu,
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

    #[test]
    fn test_rom_info_srom() {
        let cartridge = rom::Cartridge::from_data(vec![0; 0x2000]).unwrap();
        let mut captured_output = Vec::new();

        print_rom_info(cartridge, &mut captured_output).unwrap();

        let actual_output = String::from_utf8_lossy(&captured_output);
        let expected_output = ["Cartridge Type: Static ROM", "ROM Size: 8KiB"].join("\n");
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_rom_info_mbc1() {
        let mut data = vec![0; 0x2000];
        data[0x147] = 1;
        let cartridge = rom::Cartridge::from_data(data).unwrap();
        let mut captured_output = Vec::new();

        print_rom_info(cartridge, &mut captured_output).unwrap();

        let actual_output = String::from_utf8_lossy(&captured_output);
        let expected_output =
            ["Cartridge Type: MBC1", "RAM Size: 0KiB", "ROM Size: 8KiB"].join("\n");
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_rom_info_mbc1_ram() {
        let mut data = vec![0; 0x2000];
        data[0x147] = 2;
        data[0x149] = 2;
        let cartridge = rom::Cartridge::from_data(data).unwrap();
        let mut captured_output = Vec::new();

        print_rom_info(cartridge, &mut captured_output).unwrap();

        let actual_output = String::from_utf8_lossy(&captured_output);
        let expected_output =
            ["Cartridge Type: MBC1", "RAM Size: 8KiB", "ROM Size: 8KiB"].join("\n");
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_rom_info_mbc2() {
        let mut data = vec![0; 0x2000];
        data[0x147] = 5;
        let cartridge = rom::Cartridge::from_data(data).unwrap();
        let mut captured_output = Vec::new();

        print_rom_info(cartridge, &mut captured_output).unwrap();

        let actual_output = String::from_utf8_lossy(&captured_output);
        let expected_output = [
            "Cartridge Type: MBC2",
            "RAM Size: 512 x 4 bits",
            "ROM Size: 8KiB",
        ]
        .join("\n");
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_cartridge_error_display() {
        assert_eq!(
            format!(
                "{}",
                OlympiaError::Cartridge(rom::CartridgeError::NoDataInRom(0x1234))
            ),
            "Cartridge error: Address 0x1234 exceeds ROM"
        );
        assert_eq!(
            format!(
                "{}",
                OlympiaError::Cartridge(rom::CartridgeError::NonCartAddress(0x2345))
            ),
            "Cartridge error: Cannot read non-cart address 0x2345 from cartridge"
        );
        assert_eq!(
            format!(
                "{}",
                OlympiaError::Cartridge(rom::CartridgeError::NoCartridgeRam)
            ),
            "Cartridge error: RAM not supported by current cartridge"
        );
        assert_eq!(
            format!(
                "{}",
                OlympiaError::Cartridge(rom::CartridgeError::CartridgeRamDisabled)
            ),
            "Cartridge error: RAM disabled on current cartridge"
        );
        assert_eq!(
            format!(
                "{}",
                OlympiaError::Cartridge(rom::CartridgeError::ExceedsCartridgeRam(0x3456))
            ),
            "Cartridge error: Address 0x3456 outside of available cart ram"
        );
        assert_eq!(
            format!(
                "{}",
                OlympiaError::Cartridge(rom::CartridgeError::UnsupportedCartridgeType(0x56))
            ),
            "Cartridge error: Unsupported cartridge type: 0x56"
        );
        assert_eq!(
            format!(
                "{}",
                OlympiaError::Cartridge(rom::CartridgeError::UnsupportedRamSize(0x6))
            ),
            "Cartridge error: Unsupported cartridge RAM size: 0x6"
        );
    }

    #[test]
    fn test_io_error_display() {
        assert_eq!(
            format!(
                "{}",
                OlympiaError::Io(io::Error::new(io::ErrorKind::Other, "Blah"))
            ),
            "IO error: Blah"
        );
    }
}
