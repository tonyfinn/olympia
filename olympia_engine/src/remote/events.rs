use crate::events::{
    Event as EngineEvent, EventHandlerId, HBlankEvent, ManualStepEvent, MemoryWriteEvent,
    ModeChangeEvent, RegisterWriteEvent, Repeat, RomLoadedEvent, StepCompleteEvent, VBlankEvent,
};
use core::{
    any::TypeId,
    convert::{TryFrom, TryInto},
};
#[cfg(feature = "std")]
use derive_more::Error;
use derive_more::{Display, From, TryInto};

#[derive(Debug, Clone, PartialEq, Eq, From, TryInto)]
/// Events from a remote emulator
pub enum Event {
    ModeChange(ModeChangeEvent),
    VBlank(VBlankEvent),
    HBlank(HBlankEvent),
    ManualStep(ManualStepEvent),
    StepComplete(StepCompleteEvent),
    RegisterWrite(RegisterWriteEvent),
    MemoryWrite(MemoryWriteEvent),
    RomLoaded(RomLoadedEvent),
}

impl Event {
    /// The type of the underlying event
    pub fn event_type_id(&self) -> TypeId {
        use Event::*;
        match self {
            ModeChange(_) => TypeId::of::<ModeChangeEvent>(),
            VBlank(_) => TypeId::of::<VBlankEvent>(),
            HBlank(_) => TypeId::of::<HBlankEvent>(),
            ManualStep(_) => TypeId::of::<ManualStepEvent>(),
            StepComplete(_) => TypeId::of::<StepCompleteEvent>(),
            RegisterWrite(_) => TypeId::of::<RegisterWriteEvent>(),
            MemoryWrite(_) => TypeId::of::<MemoryWriteEvent>(),
            RomLoaded(_) => TypeId::of::<RomLoadedEvent>(),
        }
    }
}

impl From<EngineEvent> for Event {
    fn from(evt: EngineEvent) -> Event {
        use EngineEvent as ee;
        use Event as re;
        match evt {
            ee::VBlank(e) => re::VBlank(e),
            ee::HBlank(e) => re::HBlank(e),
            ee::RegisterWrite(e) => re::RegisterWrite(e),
            ee::MemoryWrite(e) => re::MemoryWrite(e),
            ee::StepComplete(e) => re::StepComplete(e),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Display)]
#[cfg_attr(feature = "std", derive(Error))]
/// Failures to send an event
pub enum EventSendError {
    #[display(fmt = "Invalid type of event for channel")]
    /// The sender is expecting events of a different type.
    ///
    /// This is usually indicative of a mistake in a frontend
    /// as senders should be grouped by type
    TypeError,
    #[display(fmt = "Channel closed")]
    /// The recieving of the channel has gone away.
    ///
    /// This may indicate the remote emulator has been shutdown
    /// or encountered a fatal error
    ClosedChannelError,
}

/// An item that can have events sent to it
pub trait Sender<T> {
    fn send(&self, evt: T) -> Result<(), EventSendError>;
}

impl<T> Sender<T> for Box<dyn Sender<T>> {
    fn send(&self, event: T) -> Result<(), EventSendError> {
        Sender::<T>::send(self.as_ref(), event)
    }
}

fn wrapped_handler<E, F>(f: F) -> Box<dyn Fn(Event) -> Repeat + 'static>
where
    E: TryFrom<Event>,
    F: Fn(E) -> Repeat + 'static,
{
    Box::new(move |evt| match evt.try_into() {
        Ok(evt) => f(evt),
        Err(_) => {
            eprintln!("Invalid event handler found");
            Repeat(false)
        }
    })
}

/// A mechanism for listening to remote events
pub trait RemoteEventListeners {
    /// Listen to events of type `event_type_id`
    fn on(
        &mut self,
        event_type_id: TypeId,
        f: Box<dyn Fn(Event) -> Repeat + 'static>,
    ) -> EventHandlerId;
    /// Notify listeners of a given event
    fn emit(&mut self, evt: Event);
}

/// Type-safe wrapper around dynamic event listeners
pub struct AdapterEventWrapper {
    inner: Box<dyn RemoteEventListeners>,
}

impl AdapterEventWrapper {
    pub fn new(inner: Box<dyn RemoteEventListeners>) -> AdapterEventWrapper {
        AdapterEventWrapper { inner }
    }

    /// Register an event handler
    pub fn on<T, F>(&mut self, f: F) -> EventHandlerId
    where
        T: TryFrom<Event> + 'static,
        F: Fn(T) -> Repeat + 'static,
    {
        self.inner.on(TypeId::of::<T>(), wrapped_handler(f))
    }

    /// Notify listeners of a given event
    pub fn emit<T>(&mut self, event: T)
    where
        T: Into<Event> + 'static,
    {
        self.inner.emit(event.into());
    }
}
