use super::{
    cpu::{Interrupt, CLOCKS_PER_CYCLE},
    memory::Memory,
    CYCLE_FREQ,
};

pub const TIMER_FREQ: u64 = 16384;
pub const GB_TICKS_PER_TIMER_TICK: u64 = (CYCLE_FREQ * CLOCKS_PER_CYCLE) as u64 / TIMER_FREQ;

pub const TIMER_ENABLE_MASK: u8 = 0b100;
pub const TIMER_PERIOD_MASK: u8 = 0b11;

pub const TIMER_DIVISORS: [u64; 4] = [1024, 16, 64, 256];

#[derive(Default)]
pub struct Timer {
    gb_ticks: u64,
    timer_ticks: u64,
    timer_enabled: bool,
    timer_enabled_at: u64,
    timer_reset_at: u64,
    timer_divisor_selected: usize,
    last_seen_div: u8,
}

impl Timer {
    /// Ticks the gameboy's internal timer
    ///
    /// Sets an interrupt if the timer counter overflows
    pub fn tick(&mut self, mem: &mut Memory, gb_ticks: u64) {
        let old_ticks = self.gb_ticks;
        self.gb_ticks = self.gb_ticks.wrapping_add(gb_ticks);

        let div = mem.registers().div;
        if div == 0 && self.last_seen_div != 0 {
            self.timer_reset_at = self.gb_ticks;
        }
        self.last_seen_div = div;

        self.timer_ticks = (self.gb_ticks - self.timer_reset_at) / GB_TICKS_PER_TIMER_TICK;
        mem.registers_mut().div = (self.timer_ticks & 0xFF) as u8;

        let timer_register_enabled = (mem.registers().tac & TIMER_ENABLE_MASK) != 0;

        if self.timer_enabled && timer_register_enabled {
            self.update_counter(mem, old_ticks, self.gb_ticks)
        } else if self.timer_enabled {
            self.timer_enabled = false;
        } else if timer_register_enabled {
            self.timer_enabled = true;
            self.timer_enabled_at = self.gb_ticks;
        }
    }

    fn update_counter(&mut self, mem: &mut Memory, starting_ticks: u64, finishing_ticks: u64) {
        let timer_divisor = TIMER_DIVISORS[self.timer_divisor_selected];
        let old_remainder = (starting_ticks - self.timer_enabled_at) % timer_divisor;
        let elapsed = finishing_ticks - starting_ticks;
        let gb_ticks_since_last_increment = old_remainder + elapsed;

        let amount_to_increment = (gb_ticks_since_last_increment / timer_divisor) as u8;

        let registers = mem.registers_mut();

        self.timer_divisor_selected = usize::from(registers.tac & TIMER_PERIOD_MASK);
        let (new_value, did_overflow) = registers.tima.overflowing_add(amount_to_increment);

        if did_overflow {
            registers.tima = registers.tma;
            Interrupt::Timer.set(&mut registers.iflag);
        } else {
            registers.tima = new_value;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::rom::Cartridge;

    fn memory() -> Memory {
        Memory::new(Cartridge::from_data(vec![0u8; 0x8000]).unwrap())
    }

    #[test]
    fn test_timer_sets_div() {
        let mut memory = memory();
        let mut timer = Timer::default();

        memory.registers_mut().div = 0;

        for _ in 0..65 {
            timer.tick(&mut memory, 4);
        }

        assert_eq!(memory.registers().div, 1);
    }

    #[test]
    fn test_timer_increments_counter() {
        let mut memory = memory();
        let mut timer = Timer::default();
        let timer_index = 1u8;

        timer.last_seen_div = 0;
        timer.timer_divisor_selected = usize::from(timer_index);
        memory.registers_mut().div = 0;
        memory.registers_mut().tma = 0xE0;
        memory.registers_mut().tima = 0xF0;

        memory.registers_mut().tac |= timer_index | TIMER_ENABLE_MASK;

        for _ in 0..5 {
            timer.tick(&mut memory, 4);
        }

        assert_eq!(memory.registers().tima, 0xF1);
    }

    #[test]
    fn test_timer_counter_overflow() {
        let mut memory = memory();
        let mut timer = Timer::default();
        let timer_index = 1u8;

        timer.last_seen_div = 0;
        timer.timer_divisor_selected = usize::from(timer_index);
        Interrupt::Timer.set(&mut memory.registers_mut().ie);
        memory.registers_mut().div = 0;
        memory.registers_mut().tma = 0xE0;
        memory.registers_mut().tima = 0xFF;

        memory.registers_mut().tac |= timer_index | TIMER_ENABLE_MASK;

        for _ in 0..5 {
            timer.tick(&mut memory, 4);
        }

        assert_eq!(memory.registers().tima, 0xE0);
        assert_eq!(
            Interrupt::test(memory.registers().ie, memory.registers().iflag),
            Some(Interrupt::Timer)
        );
    }
}
