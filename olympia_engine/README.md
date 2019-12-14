# Olympia Engine

Engine is the core gameboy emulation logic. It works in a 
`no_std` environment, allowing its use in a web assembly environment. It does however, require the `core` and `alloc` packages. If you are using it in `std` environment, you can include the `std` feature for helpful addons like `Display` implementations on Error types.

The `decoder` package is used for taking a binary ROM and converting it to an internal representation that can then be executed in the emulated gameboy.

The `gameboy` package contains the logic to implement gameboy features. At the time of writing, this is limited to the CPU.

Various modules at top level, such as `instructions` and `registers` define types that are used throughout the emulator.

## Emulation Details

Instructions are decoded in the `decoder` package. For most instructions, they are lookup up in the table in `decoder.rs`. For more complicated instructions, there can be a decoder registered in the table. This will then be run to decode an instruction. Note that while at decode time it will store the value of the next byte(s) if there is an operand to the instruction there, this will be read again at execution time to allow for changes, so the stored value is primarily for disassembly usage.