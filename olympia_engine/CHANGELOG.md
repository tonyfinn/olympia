# Changelog

## 0.4.0 [Unreleased]

## 0.3.0

### Added features

* Add support for PPU background tiles
* Add support for remote usage
    * This requires a beta toolchain currently, as it relies on async/await which is not coming to
      `no_std` until v1.44.0
* Provide `std::error::Error` implementations for all errors when  the `std` feature is enabled
* Add new event handling system to monitor local events

### Breaking Changes

* Instructions have been totally rewritten. The old `Instruction` type has been replaced with `RuntimeInstruction`
* Many types that are needed for new derives have been moved to the olympia_core crate. In most cases these should
  be re-exported with their old names.
* Renames:
  * `rom::CartridgeEnum` -> `rom::ControllerEnum`
  * `rom::CartridgeType` -> `rom::CartridgeController`

## 0.2.0

### Added features

* Add support for interrupt handling.
* Add remaining stack instructions
* Add DAA and CPL instructions

### Breaking changes

* Removed `types` module (in favour of `olympia_core::address`)