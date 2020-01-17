use std::io;

use olympia_engine::instructionsn::RuntimeDecoder;

struct FormattingIterator<T: Iterator<Item = u8>> {
    verbose: bool,
    next_addr: usize,
    addr: usize,
    source_iterator: T,
    decoder: RuntimeDecoder,
}

impl<T: Iterator<Item = u8>> FormattingIterator<T> {
    fn new(verbose: bool, source_iterator: T) -> Self {
        FormattingIterator {
            verbose,
            source_iterator,
            next_addr: 0,
            addr: 0,
            decoder: RuntimeDecoder::new(),
        }
    }
}

impl<T: Iterator<Item = u8>> Iterator for FormattingIterator<T> {
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
        let bytes = instr.map(|i| i.as_bytes()).unwrap_or(vec![val]);
        let size = bytes.len();
        let mut numeric = String::with_capacity(size * 2);
        for byte in bytes {
            numeric.push_str(&format!("{:02X}", byte))
        }

        let current_addr = self.addr;
        self.addr += size;
        if self.verbose {
            Some(format!(
                "{:>6X}:\t\t{:>6}\t\t{}",
                current_addr, numeric, text
            ))
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

pub(crate) fn do_disassemble<O: io::Write>(
    data: Vec<u8>,
    verbose: bool,
    output: &mut O,
) -> io::Result<()> {
    let formatting_iterator = FormattingIterator::new(verbose, data.into_iter());

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

        do_disassemble(data, false, &mut output).unwrap();

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

        do_disassemble(data, true, &mut output).unwrap();

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
