use crate::rom::TargetConsole;

pub mod cpu;

pub enum GameBoyModel {
    GameBoy,          // DMG
    GameBoyPocket,    // MGB
    SuperGameBoy,     // SGB
    GameBoyColor,     // GBC
    GameBoyAdvance,   // AGB
    GameBoyAdvanceSP, // AGS
}

impl GameBoyModel {
    pub(crate) fn default_af(&self) -> u16 {
        match self {
            GameBoyModel::GameBoy => 0x01B0,
            GameBoyModel::GameBoyPocket => 0xFFB0,
            GameBoyModel::SuperGameBoy => 0x0100,
            GameBoyModel::GameBoyColor => 0x1180,
            GameBoyModel::GameBoyAdvance => 0x1100,
            GameBoyModel::GameBoyAdvanceSP => 0x1100,
        }
    }

    pub(crate) fn default_bc(&self) -> u16 {
        match self {
            GameBoyModel::GameBoy => 0x0013,
            GameBoyModel::GameBoyPocket => 0x0013,
            GameBoyModel::SuperGameBoy => 0x0014,
            GameBoyModel::GameBoyColor => 0x0000,
            GameBoyModel::GameBoyAdvance => 0x0100,
            GameBoyModel::GameBoyAdvanceSP => 0x0100,
        }
    }

    pub(crate) fn default_de(&self, target: TargetConsole) -> u16 {
        let gbc_mode = target != TargetConsole::GameBoyOnly;
        match self {
            GameBoyModel::GameBoy => 0x00D8,
            GameBoyModel::GameBoyPocket => 0x00D8,
            GameBoyModel::SuperGameBoy => 0x0000,
            GameBoyModel::GameBoyColor if gbc_mode => 0xFF56,
            GameBoyModel::GameBoyColor => 0x0008,
            GameBoyModel::GameBoyAdvance if gbc_mode => 0xFF56,
            GameBoyModel::GameBoyAdvance => 0x0008,
            GameBoyModel::GameBoyAdvanceSP if gbc_mode => 0xFF56,
            GameBoyModel::GameBoyAdvanceSP => 0x0008,
        }
    }

    pub(crate) fn default_hl(&self, target: TargetConsole) -> u16 {
        let gbc_mode = target != TargetConsole::GameBoyOnly;
        match self {
            GameBoyModel::GameBoy => 0x014D,
            GameBoyModel::GameBoyPocket => 0x014D,
            GameBoyModel::SuperGameBoy => 0xC060,
            GameBoyModel::GameBoyColor if gbc_mode => 0x000D,
            GameBoyModel::GameBoyColor => 0x007C,
            GameBoyModel::GameBoyAdvance if gbc_mode => 0x000D,
            GameBoyModel::GameBoyAdvance => 0x007C,
            GameBoyModel::GameBoyAdvanceSP if gbc_mode => 0x000D,
            GameBoyModel::GameBoyAdvanceSP => 0x007C,
        }
    }
}
