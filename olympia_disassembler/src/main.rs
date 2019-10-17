use std::fs;
use std::io;
use std::path::PathBuf;

use structopt::StructOpt;

use olympia_engine::decoder;

#[derive(Debug, StructOpt)]
#[structopt(name="olympia-disasm", about="Disassemble a GB rom")]
struct Opt {
    #[structopt(short="v", long="verbose")]
    verbose: bool,
    #[structopt(parse(from_os_str))]
    input: PathBuf,
}


struct FormattingIterator<T: Iterator<Item=decoder::DisassembledInstruction>> {
    verbose: bool,
    next_addr: usize,
    addr: usize,
    source_iterator: T
}

impl<T: Iterator<Item=decoder::DisassembledInstruction>> FormattingIterator<T> {
    fn new(verbose: bool, source_iterator: T) -> Self {
        FormattingIterator {
            verbose,
            source_iterator,
            next_addr: 0,
            addr: 0
        }
    }
}

impl<T: Iterator<Item=decoder::DisassembledInstruction>> Iterator for FormattingIterator<T> {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        use olympia_engine::decoder::DisassembledInstruction as dis;
        let (numeric, text, size) = match self.source_iterator.next()? {
            dis::OneByte(binary, text) => (format!("{:0>2X}", binary), text, 1),
            dis::TwoByte(binary, text) => (format!("{:0>4X}", binary), text, 2),
            dis::ThreeByte(binary, text) => (format!("{:0>6X}", binary), text, 3),
        };

        let current_addr = self.addr;
        self.addr += size;
        if self.verbose {
            Some(format!("{:>6X}:\t\t{:>6}\t\t{}", current_addr, numeric, text))
        } else {
            let addr_to_print = if current_addr >= self.next_addr {
                self.next_addr += 0x10;
                format!("{:>6X}:", current_addr)
            } else {
                format!("{:>7}", &"")
            };
            Some(format!("{}\t\t{}", addr_to_print, text))
        }
    }
}

fn do_disassemble<O: io::Write>(args: Opt, data: Vec<u8>, output: &mut O) -> io::Result<()> {
    let disassembled = decoder::disassemble(&data)
        .expect("Failed decoding ROM");
    let formatting_iterator = FormattingIterator::new(args.verbose, disassembled.into_iter());

    for disassembled_instruction in formatting_iterator {
        writeln!(output, "{}", disassembled_instruction)?;
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let args = Opt::from_args();

    let data = fs::read(args.input.clone())
        .expect("Failed reading ROM");

    do_disassemble(args, data, &mut io::stdout())
}

#[cfg(test)]
pub mod test {

    use super::*;

    #[test]
    fn test_disassembly_non_verbose() {
        let args = Opt {
            verbose: false,
            input: PathBuf::new()
        };

        let data = vec![
            0x26, 0x20,
            0x0E, 0x44,
            0x11, 0x23, 0x25,
            0xC3, 0x22, 0x11,
            0xF3, 0x00, 0xFB,
            0xF3, 0x00, 0xFB,
            0xF3, 0x00, 0xFB,
        ];

        let mut output: Vec<u8> = Vec::new();

        do_disassemble(args, data, &mut output).unwrap();

        let expected_result = concat!(
            "     0:\t\tLD H, 20h\n",
            "       \t\tLD C, 44h\n",
            "       \t\tLD DE, 2523h\n",
            "       \t\tJP $1122h\n",
            "       \t\tDI\n",
            "       \t\tNOP\n",
            "       \t\tEI\n",
            "       \t\tDI\n",
            "       \t\tNOP\n",
            "       \t\tEI\n",
            "    10:\t\tDI\n",
            "       \t\tNOP\n",
            "       \t\tEI\n",
        );
        assert_eq!(String::from_utf8_lossy(&output), String::from(expected_result));
    }

    #[test]
    fn test_disassembly_verbose() {
        let args = Opt {
            verbose: true,
            input: PathBuf::new()
        };

        let data = vec![
            0x26, 0x20,
            0x0E, 0x44,
            0x11, 0x23, 0x25,
            0xC3, 0x22, 0x11,
            0xF3, 0x00, 0xFB,
            0xF3, 0x00, 0xFB,
            0xF3, 0x00, 0xFB,
        ];

        let mut output: Vec<u8> = Vec::new();

        do_disassemble(args, data, &mut output).unwrap();

        let expected_result = concat!(
            "     0:\t\t  2620\t\tLD H, 20h\n",
            "     2:\t\t  0E44\t\tLD C, 44h\n",
            "     4:\t\t112325\t\tLD DE, 2523h\n",
            "     7:\t\tC32211\t\tJP $1122h\n",
            "     A:\t\t    F3\t\tDI\n",
            "     B:\t\t    00\t\tNOP\n",
            "     C:\t\t    FB\t\tEI\n",
            "     D:\t\t    F3\t\tDI\n",
            "     E:\t\t    00\t\tNOP\n",
            "     F:\t\t    FB\t\tEI\n",
            "    10:\t\t    F3\t\tDI\n",
            "    11:\t\t    00\t\tNOP\n",
            "    12:\t\t    FB\t\tEI\n",
        );
        assert_eq!(String::from_utf8_lossy(&output), String::from(expected_result));
    }
}