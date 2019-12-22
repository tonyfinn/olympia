# Olympia CLI

A terminal UI for interaction with the Olympia emulator.

## CLI Commands

### debug

Usage:

`olympia_cli debug <rom>`

Open an interactive debugging session for the given ROM. For a list of commands available in the debugger, type `help` at the prompt it produces, or scroll down to `Debugger Commands`.


### rom-info

Usage:

`olympia_cli rom-info <rom>`

Prints out the known information about a given ROM. Currently limited to controller type, RAM size and ROM size.

### disassemble

Usage:

`olympia_cli disassemble [-v] <rom>`

Prints out a disassembly of the given ROM. 

If the verbose (`-v`) flag is specified, every line will contain an address, the numeric value of the
opcode, and the textual value of the opcode. If the flag is not specified, then every 10th
line will contain a label for the address, and every line will contain the textual value
of the operation.


## Common Debugger Commands

### step

Usage:

`step <n>` / `s <n>`

Step forward by the given number of instructions. 


### exit

Usage:

`exit`

Quits this debugging session and the CLI.


### help

Usage:

`help`

Prints out usage instructions for the debug prompt.


### print-bytes

Usage:

`print-bytes 1:100` / `pb 1:100`

Prints out the bytes from decimal 1 to decimal 100

`print-bytes :0x100`

Print out the bytes from 0 to hex 100

`print-bytes 0xFFE0:`

Print out the bytes from 0xFFE0 to 0xFFFF

`print-bytes header`

Print out bytes in the named region `header`. Other options include `staticrom`, `switchrom`, `vram`, `cartram`, `sysram`, `cpuram`.

Note that with carts with less than 0x2000 bytes of RAM or 0x8000 bytes of ROM, not all of the address space will be populated. In the debugger this will be displayed as `--` for the affected addresses. Games attempting to read this area on actual gameboy hardware will have undefined behaviour.


### print-registers

Usage:

`print-registers` / `pr`

Print out the values of all registers. The F (flags) register is broken out the show the individual flags.


## Other Debugger Commands

### current

Usage:

`current` / `ci`

Prints out the disassembly of the current instruction.


### cycle-count

Usage:

`cycle-count` / `cc`

Print out the total number of clock cycles elapsed since emulator startup. This is mostly useful for performance measurement or emulator debugging.

