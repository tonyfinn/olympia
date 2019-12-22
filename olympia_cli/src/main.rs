mod debug;
mod disassemble;

use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use olympia_engine::gameboy;
use olympia_engine::rom;
use structopt::StructOpt;

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
    Disassemble {
        #[structopt(short = "v", long)]
        verbose: bool,
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

#[cfg(unix)]
fn is_tty() -> bool {
    use std::os::unix::fs::FileTypeExt;
    let pid = std::process::id();
    let fd_path = format!("/proc/{}/fd/0", pid);
    let metadata_result = std::fs::metadata(fd_path);
    if let Ok(metadata) = metadata_result {
        if metadata.file_type().is_char_device() {
            return true;
        }
    }
    false
}

#[cfg(windows)]
fn is_tty() -> bool {
    false
}

fn find_err_out(args: &OlympiaArgs) -> Box<dyn io::Write> {
    if args.quiet || !is_tty() {
        Box::new(io::sink())
    } else {
        Box::new(io::stderr())
    }
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
        OlympiaCommand::Debug { rom } => debug::debug(
            gameboy::GameBoy::new(parse_cartridge(&rom)?, gameboy::GameBoyModel::GameBoy),
            &mut io::stdin(),
            &mut io::stdout(),
            err.as_mut(),
        )?,
        OlympiaCommand::Disassemble { verbose, rom } => {
            let data = std::fs::read(rom)?;
            disassemble::do_disassemble(data, verbose, &mut io::stdout())?
        }
    }
    Ok(())
}

#[cfg(test)]
pub mod test {
    use super::*;

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
