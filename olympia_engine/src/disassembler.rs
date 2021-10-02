use std::io;

use crate::instructionsn::RuntimeDecoder;

/// Format to print disassembly in
#[derive(Debug, PartialEq, Eq)]
pub enum DisassemblyFormat {
    /// Address every 10 bytes + decoded instruction
    Normal,
    /// Address every byte + raw bytes + decoded instruction
    Verbose,
    /// Verbose with space aligned columns
    Columnar,
}

impl Default for DisassemblyFormat {
    fn default() -> DisassemblyFormat {
        DisassemblyFormat::Normal
    }
}

/// Iterates over a sequence of bytes and emits disassembled instructions
pub struct DisassemblyIterator<T: Iterator<Item = u8>> {
    format: DisassemblyFormat,
    next_addr: usize,
    addr: usize,
    source_iterator: T,
    decoder: RuntimeDecoder,
}

impl<T: Iterator<Item = u8>> DisassemblyIterator<T> {
    /// Create a new disassembling iterator
    ///
    /// `verbose` includes hex values of instructions as well as disassembly
    ///
    /// `initial_offset` indicates the starting address of this program fragment
    pub fn new(source_iterator: T, format: DisassemblyFormat, initial_offset: usize) -> Self {
        DisassemblyIterator {
            format,
            source_iterator,
            next_addr: initial_offset,
            addr: initial_offset,
            decoder: RuntimeDecoder::new(),
        }
    }
}

impl<T: Iterator<Item = u8>> Iterator for DisassemblyIterator<T> {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        let val = self.source_iterator.next()?;

        let instr = self
            .decoder
            .decode_from_iter(val, &mut self.source_iterator);
        let text = instr
            .as_ref()
            .map(|i| i.disassemble())
            .unwrap_or_else(|| format!("DAT {:X}h", val));
        let bytes = instr.map(|i| i.as_bytes()).unwrap_or_else(|| vec![val]);
        let size = bytes.len();
        let mut numeric = String::with_capacity(size * 2);
        for byte in bytes {
            numeric.push_str(&format!("{:02X}", byte))
        }

        let current_addr = self.addr;
        self.addr += size;
        if self.format == DisassemblyFormat::Verbose {
            Some(format!(
                "{:>6X}:\t\t{:>6}\t\t{}",
                current_addr, numeric, text
            ))
        } else if self.format == DisassemblyFormat::Columnar {
            let addr_text = format!("{:04X}:", current_addr);
            Some(format!("{:<7}{:>10}    {}", addr_text, numeric, text))
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

/// Disassembles a complete program
///
/// `verbose` includes hex values of instructions as well as disassembly
///
/// See [`FormattingIterator`] for more customisable options
pub fn disassemble(
    data: Vec<u8>,
    format: DisassemblyFormat,
    output: &mut dyn std::io::Write,
) -> io::Result<()> {
    let formatting_iterator = DisassemblyIterator::new(data.into_iter(), format, 0);

    for disassembled_instruction in formatting_iterator {
        writeln!(output, "{}", disassembled_instruction)?;
    }
    Ok(())
}

#[cfg(test)]
pub mod test {

    use super::*;

    #[test]
    fn test_disassembly_non_verbose() {
        let data = vec![
            0x26, 0x20, // LD H, 20h
            0x0E, 0x44, // LD C, 44h
            0x11, 0x23, 0x25, // LD DE 2523h
            0xC3, 0x22, 0x11, // JP $1122h
            0xF3, 0x00, 0xFB, // DI, NOP, EI
            0xF3, 0x00, 0xFB, // DI, NOP, EI
            0xF3, 0x00, 0xFB, // DI, NOP, EI
        ];

        let mut output: Vec<u8> = Vec::new();

        disassemble(data, DisassemblyFormat::Normal, &mut output).unwrap();

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
        assert_eq!(
            String::from_utf8_lossy(&output),
            String::from(expected_result)
        );
    }

    #[test]
    fn test_disassembly_verbose() {
        let data = vec![
            0x26, 0x20, // LD H, 20h
            0x0E, 0x44, // LD C, 44h
            0x11, 0x23, 0x25, // LD DE, 2523h
            0xC3, 0x22, 0x11, // JP $1122h
            0xF3, 0x00, 0xFB, // DI, NOP, EI
            0xF3, 0x00, 0xFB, // DI, NOP, EI
            0xF3, 0x00, 0xFB, // DI, NOP, EI
        ];

        let mut output: Vec<u8> = Vec::new();

        disassemble(data, DisassemblyFormat::Verbose, &mut output).unwrap();

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
        assert_eq!(
            String::from_utf8_lossy(&output),
            String::from(expected_result)
        );
    }
}
