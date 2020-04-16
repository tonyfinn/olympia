# Olympia

[![pipeline status](https://gitlab.com/tonyfinn/olympia/badges/master/pipeline.svg)](https://gitlab.com/tonyfinn/olympia/-/commits/master)[![Coverage Status](https://coveralls.io/repos/gitlab/tonyfinn/olympia/badge.svg?branch=master)](https://coveralls.io/gitlab/tonyfinn/olympia?branch=master)

Olympia is a gameboy emulator and toolkit, intended to run as a native or web assembly application targeting a cycle count accurate emulation.

Currently it is in a very early stage, with mostly basic CPU instruction set emulation and a CLI debugger and disassembler completed.

## Components

`olympia_engine` - This is the core engine for Olympia, inteded for use across various frontends. Because it needs to run in both a native application and a WebAssembly module, it must work in a `no_std` environment - `alloc` and `core` are allowed. The `std` feature is allowed to use libraries from `std`, but should not be used for any essential functionality.

`olympia_cli` - This provides a CLI that currently allows you to print ROM metadata or interactively debug execution.

`olympia_native` - This provides a native UI to run the emulator

## License

Olympia is licensed under the GPL v3+, available at LICENSE.txt. (c) Tony Finn 2019

Some documentation (in /docs) and test ROMs (in /res) in  this repo are under different licenses. For test roms, if the ROM itself was not created as part of Olympia, the original author and license will be listed in a .txt file alongside the .gb file. 