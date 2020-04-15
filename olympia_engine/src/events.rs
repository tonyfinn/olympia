use crate::address;
use core::borrow::Borrow;
use core::cell::RefCell;
use alloc::collections::BTreeMap;
use crate::registers;
use crate::gameboy::GBPixel;

use derive_more::{Constructor, From, TryInto};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Constructor)]
pub struct MemoryWriteEvent {
    /// Location written to
    address: address::LiteralAddress,
    /// Value written to that location
    value: u8,
    /// The actual new value after the write
    new_value: u8,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Constructor)]
pub struct RegisterWriteEvent {
    reg: registers::WordRegister,
    value: u16,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct VBlankEvent;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct StepCompleteEvent;

#[derive(Debug, PartialEq, Eq, Clone, Constructor)]
pub struct HBlankEvent {
    pixels: Vec<GBPixel>,
}

#[derive(Debug, PartialEq, Eq, Clone, From, TryInto)]
/// Gameboy events that frontends might be interested in
pub enum Event {
    /// A write occured to a memory mapped location
    MemoryWrite(MemoryWriteEvent),
    /// A write occured to a named register
    RegisterWrite(RegisterWriteEvent),
    /// The PPU reached its hblank cycle
    HBlank(HBlankEvent),
    /// The PPU reached its vblank cycle
    VBlank(VBlankEvent),
    /// An instruction cycle completed
    StepComplete(StepCompleteEvent),
}

pub type EventHandler<T> = Box<dyn Fn(&T) -> () + 'static>;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub struct EventHandlerId(u64);

pub struct EventEmitter<T> {
    event_handlers: RefCell<BTreeMap<EventHandlerId, EventHandler<T>>>,
    next_event_handler_id: RefCell<u64>,
    is_emitting: RefCell<bool>,
    queued_handlers: RefCell<Vec<(EventHandlerId, EventHandler<T>)>>,
    queued_removals: RefCell<Vec<EventHandlerId>>,
}

impl<'a, T> EventEmitter<T> {
    pub fn new() -> EventEmitter<T> {
        EventEmitter {
            event_handlers: RefCell::new(BTreeMap::new()),
            next_event_handler_id: RefCell::new(0),
            is_emitting: RefCell::new(false),
            queued_handlers: RefCell::new(Vec::new()),
            queued_removals: RefCell::new(Vec::new()),
        }
    }
    
    pub fn on(&self, f: EventHandler<T>) -> EventHandlerId {
        let event_handler_id = self.next_handler_id();
        if self.is_emitting.borrow().clone() {
            self.queue_handler(event_handler_id, f)
        } else {
            self.register_handler(event_handler_id, f)
        }
        event_handler_id
    }

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
        self.queued_handlers.borrow_mut().push((event_handler_id, f));
    }
    
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

pub fn propagate_events<I, O, E>(inner_events: &EventEmitter<I>, outer_events: E) -> EventHandlerId
    where I: Into<O> + Clone,
        E: 'static + Borrow<EventEmitter<O>>,
{
    inner_events.on(Box::new(move |inner_item| {
        let cloned: I = inner_item.clone();
        outer_events.borrow().emit(cloned.into())
    }))
}