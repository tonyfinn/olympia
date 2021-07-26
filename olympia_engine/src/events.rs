//! Event handling system for local emulators
//!
//! Note that event handling for remote emulators is slightly
//! different, see [`crate::remote`] for details on those
//!
//! [`crate::remote`]: ../remote/index.html

use crate::address;
use crate::gameboy::GBPixel;
use crate::registers;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::borrow::Borrow;
use core::cell::RefCell;
use hashbrown::HashMap;

use derive_more::{Constructor, From, TryInto};

use crate::remote::ExecMode;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Represents a change in running mode of an emulator
pub struct ModeChangeEvent {
    /// Previous execution mode
    pub old_mode: ExecMode,
    /// Execution mode after the mode change
    pub new_mode: ExecMode,
}

impl ModeChangeEvent {
    pub fn new(old_mode: ExecMode, new_mode: ExecMode) -> ModeChangeEvent {
        ModeChangeEvent { old_mode, new_mode }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A ROM has been loaded into a remote emulator
pub struct RomLoadedEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
/// A step has happened by a manual user request
pub struct ManualStepEvent;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
/// Memory has been accessed
pub enum MemoryEvent {
    /// Memory has been read from
    Read {
        /// Location read from
        address: address::LiteralAddress,
        /// Value read from that location
        value: u8,
    },
    /// Memory has been written to
    Write {
        /// Location written to
        address: address::LiteralAddress,
        /// Value written to that location
        value: u8,
        /// The actual new value after the write
        new_value: u8,
    },
}

impl MemoryEvent {
    pub(crate) fn read(address: address::LiteralAddress, value: u8) -> MemoryEvent {
        MemoryEvent::Read { address, value }
    }
    pub(crate) fn write(address: address::LiteralAddress, value: u8, new_value: u8) -> MemoryEvent {
        MemoryEvent::Write {
            address,
            value,
            new_value,
        }
    }
}

/// A register has been written to
///
/// Note this event is only in terms of 16-bit registers,
/// when an 8-bit write occurs, the event will cover
/// the 16-bit register the 8-bit register is part of
#[derive(Debug, PartialEq, Eq, Clone, Copy, Constructor)]
pub struct RegisterWriteEvent {
    pub reg: registers::WordRegister,
    pub value: u16,
}

#[derive(Debug, PartialEq, Eq, Clone, Constructor)]
/// PPU has entered the HBlank phase
pub struct HBlankEvent {
    pub current_line: u8,
    pub pixels: Vec<GBPixel>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
/// PPU has entered the VBlank phase
pub struct VBlankEvent;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
/// A single instruction has completed
pub struct StepCompleteEvent;

#[derive(Debug, PartialEq, Eq, Clone, From)]
/// Events from the PPU
pub enum PPUEvent {
    VBlank(VBlankEvent),
    HBlank(HBlankEvent),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
/// Indicates whether an event handler should repeat
pub struct Repeat(pub bool);

#[derive(Debug, PartialEq, Eq, Clone, From, TryInto)]
/// Gameboy events that frontends might be interested in
pub enum Event {
    /// A write occured to a memory mapped location
    Memory(MemoryEvent),
    /// A write occured to a named register
    RegisterWrite(RegisterWriteEvent),
    /// The PPU reached its hblank cycle
    HBlank(HBlankEvent),
    /// The PPU reached its vblank cycle
    VBlank(VBlankEvent),
    /// An instruction cycle completed
    StepComplete(StepCompleteEvent),
}

impl From<PPUEvent> for Event {
    fn from(ppue: PPUEvent) -> Event {
        match ppue {
            PPUEvent::VBlank(e) => Event::VBlank(e),
            PPUEvent::HBlank(e) => Event::HBlank(e),
        }
    }
}

/// A method to handle a local event
pub type EventHandler<T> = Box<dyn Fn(&T) -> () + 'static>;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
/// An identifer for a event handler
pub struct EventHandlerId(pub u64);

/// A producer of events that can be listened to
pub struct EventEmitter<T> {
    event_handlers: RefCell<HashMap<EventHandlerId, EventHandler<T>>>,
    next_event_handler_id: RefCell<u64>,
    is_emitting: RefCell<bool>,
    queued_handlers: RefCell<Vec<(EventHandlerId, EventHandler<T>)>>,
    queued_removals: RefCell<Vec<EventHandlerId>>,
}

impl<'a, T> EventEmitter<T> {
    pub fn new() -> EventEmitter<T> {
        EventEmitter {
            event_handlers: RefCell::new(HashMap::new()),
            next_event_handler_id: RefCell::new(0),
            is_emitting: RefCell::new(false),
            queued_handlers: RefCell::new(Vec::new()),
            queued_removals: RefCell::new(Vec::new()),
        }
    }

    /// Listen to events of a given type
    pub fn on(&self, f: EventHandler<T>) -> EventHandlerId {
        let event_handler_id = self.next_handler_id();
        if self.is_emitting.borrow().clone() {
            self.queue_handler(event_handler_id, f)
        } else {
            self.register_handler(event_handler_id, f)
        }
        event_handler_id
    }

    /// Stop listening to a given event
    pub fn off(&self, id: EventHandlerId) {
        if self.is_emitting.borrow().clone() {
            self.queued_removals.borrow_mut().push(id);
        } else {
            self.event_handlers.borrow_mut().remove(&id);
        }
    }

    fn next_handler_id(&self) -> EventHandlerId {
        EventHandlerId(self.next_event_handler_id.replace_with(|old| *old + 1))
    }

    fn register_handler(&self, event_handler_id: EventHandlerId, f: EventHandler<T>) {
        self.event_handlers.borrow_mut().insert(event_handler_id, f);
    }

    fn queue_handler(&self, event_handler_id: EventHandlerId, f: EventHandler<T>) {
        self.queued_handlers
            .borrow_mut()
            .push((event_handler_id, f));
    }

    /// Notify all listeners of a given event
    pub fn emit(&self, evt: T) {
        self.is_emitting.replace(true);
        for handler in self.event_handlers.borrow().values() {
            handler(&evt);
        }
        self.is_emitting.replace(false);

        let mut event_handlers = self.event_handlers.borrow_mut();
        for (id, f) in self.queued_handlers.borrow_mut().drain(..) {
            event_handlers.insert(id, f);
        }

        for id in self.queued_removals.borrow_mut().drain(..) {
            event_handlers.remove(&id);
        }
    }
}

/// Propagate events from one event emitter to another
pub fn propagate_events<I, O, E>(inner_events: &EventEmitter<I>, outer_events: E) -> EventHandlerId
where
    I: Into<O> + Clone,
    E: 'static + Borrow<EventEmitter<O>>,
{
    inner_events.on(Box::new(move |inner_item| {
        let cloned: I = inner_item.clone();
        outer_events.borrow().emit(cloned.into())
    }))
}
