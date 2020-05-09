pub(crate) use crate::emulator::commands::{ExecMode, Repeat};
pub use olympia_engine::events::{
    Event as EngineEvent, EventEmitter, EventHandlerId, HBlankEvent, MemoryWriteEvent,
    RegisterWriteEvent, StepCompleteEvent, VBlankEvent,
};
use std::{
    any::TypeId,
    collections::HashMap,
    convert::{TryFrom, TryInto},
};

use derive_more::{Display, Error, From, TryInto};

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
    fn event_type_id(&self) -> TypeId {
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

impl<T, R> Sender<T> for glib::Sender<R>
where
    R: TryFrom<T>,
{
    fn send(&self, event: T) -> Result<(), EventSendError> {
        match event.try_into() {
            Ok(evt) => self
                .send(evt)
                .map_err(|_| EventSendError::ClosedChannelError),
            Err(_) => Err(EventSendError::TypeError),
        }
    }
}

impl<T> Sender<T> for Box<dyn Sender<T>> {
    fn send(&self, event: T) -> Result<(), EventSendError> {
        Sender::<T>::send(self.as_ref(), event)
    }
}

pub(crate) struct AdapterEventListeners {
    listeners: HashMap<TypeId, HashMap<EventHandlerId, Box<dyn Sender<AdapterEvent>>>>,
    next_listener_id: u64,
}

impl AdapterEventListeners {
    pub(crate) fn new() -> AdapterEventListeners {
        AdapterEventListeners {
            listeners: HashMap::new(),
            next_listener_id: 0,
        }
    }

    pub(crate) fn on<T, F>(
        &mut self,
        context: &glib::MainContext,
        f: F,
    ) -> EventHandlerId
    where
        T: TryFrom<AdapterEvent> + 'static,
        F: Fn(T) -> Repeat + 'static,
    {
        let event_handler_id = EventHandlerId(self.next_listener_id);
        let (tx, rx) = glib::MainContext::channel::<T>(glib::PRIORITY_DEFAULT);
        let wrapped = Box::new(tx);
        let type_id = TypeId::of::<T>();
        let map = self
            .listeners
            .entry(type_id)
            .or_insert_with(|| HashMap::new());
        map.insert(event_handler_id, wrapped);
        self.next_listener_id += 1;

        rx.attach(Some(context), move |evt| f(evt).into());

        event_handler_id
    }

    pub(crate) fn emit<T>(&mut self, event: T)
    where
        T: Into<AdapterEvent> + 'static,
    {
        let evt = event.into();
        let event_type_id = evt.event_type_id();
        if let Some(listeners) = self.listeners.get_mut(&event_type_id) {
            let mut listener_ids_to_remove = Vec::new();
            for (id, listener) in listeners.iter_mut() {
                let send_result = listener.send(evt.clone());
                if send_result.is_err() {
                    listener_ids_to_remove.push(id.clone());
                    eprintln!("Removing listener {:?} due to closed channel", id);
                }
            }
            for id in listener_ids_to_remove {
                listeners.remove(&id);
            }
        }
    }
}