pub(crate) use crate::emulator::commands::{ExecMode, Repeat};
use core::{
    any::TypeId,
    convert::{TryFrom, TryInto},
};
use derive_more::{Display, Error, From, TryInto};
pub use olympia_engine::events::{
    Event as EngineEvent, EventEmitter, EventHandlerId, HBlankEvent, MemoryWriteEvent,
    RegisterWriteEvent, StepCompleteEvent, VBlankEvent,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModeChangeEvent {
    pub(crate) old_mode: ExecMode,
    pub(crate) new_mode: ExecMode,
}

impl ModeChangeEvent {
    pub(crate) fn new(old_mode: ExecMode, new_mode: ExecMode) -> ModeChangeEvent {
        ModeChangeEvent { old_mode, new_mode }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RomLoadedEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManualStepEvent;

#[derive(Debug, Clone, PartialEq, Eq, From, TryInto)]
pub(crate) enum AdapterEvent {
    ModeChange(ModeChangeEvent),
    VBlank(VBlankEvent),
    HBlank(HBlankEvent),
    ManualStep(ManualStepEvent),
    StepComplete(StepCompleteEvent),
    RegisterWrite(RegisterWriteEvent),
    MemoryWrite(MemoryWriteEvent),
    RomLoaded(RomLoadedEvent),
}

impl AdapterEvent {
    pub(crate) fn event_type_id(&self) -> TypeId {
        use AdapterEvent::*;
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

impl From<EngineEvent> for AdapterEvent {
    fn from(evt: EngineEvent) -> AdapterEvent {
        use AdapterEvent as ae;
        use EngineEvent as ee;
        match evt {
            ee::VBlank(e) => ae::VBlank(e),
            ee::HBlank(e) => ae::HBlank(e),
            ee::RegisterWrite(e) => ae::RegisterWrite(e),
            ee::MemoryWrite(e) => ae::MemoryWrite(e),
            ee::StepComplete(e) => ae::StepComplete(e),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Display, Error)]
pub(crate) enum EventSendError {
    #[display(fmt = "Invalid type of event for channel")]
    TypeError,
    #[display(fmt = "Channel closed")]
    ClosedChannelError,
}

pub(crate) trait Sender<T> {
    fn send(&self, evt: T) -> Result<(), EventSendError>;
}

impl<T> Sender<T> for Box<dyn Sender<T>> {
    fn send(&self, event: T) -> Result<(), EventSendError> {
        Sender::<T>::send(self.as_ref(), event)
    }
}


fn wrapped_handler<E, F>(f: F) -> Box<dyn Fn(AdapterEvent) -> Repeat + 'static>
    where E: TryFrom<AdapterEvent>,
        F: Fn(E) -> Repeat + 'static
{
    Box::new(move |evt| {
        match evt.try_into() {
            Ok(evt) => { 
                f(evt) 
            },
            Err(_) => {
                eprintln!("Invalid event handler found");
                Repeat(false)
            }
        }
    })
}

pub(crate) trait AdapterEventListeners {
    fn on(&mut self, event_type_id: TypeId, f: Box<dyn Fn(AdapterEvent) -> Repeat + 'static>) -> EventHandlerId;
    fn emit(&mut self, evt: AdapterEvent);
}

pub struct AdapterEventWrapper {
    inner: Box<dyn AdapterEventListeners>,
}

impl AdapterEventWrapper {
    pub(crate) fn new(inner: Box<dyn AdapterEventListeners>) -> AdapterEventWrapper {
        AdapterEventWrapper {
            inner
        }
    }

    pub(crate) fn on<T, F>(
        &mut self,
        f: F,
    ) -> EventHandlerId
    where
        T: TryFrom<AdapterEvent> + 'static,
        F: Fn(T) -> Repeat + 'static 
    {
        self.inner.on(TypeId::of::<T>(), wrapped_handler(f))
    }

    pub(crate) fn emit<T>(&mut self, event: T)
    where
        T: Into<AdapterEvent> + 'static,
    {
        self.inner.emit(event.into());
    }
}
