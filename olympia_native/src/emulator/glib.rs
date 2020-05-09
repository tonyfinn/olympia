use crate::emulator::{
    commands::{CommandId, EmulatorCommand, EmulatorThreadOutput},
    emu_thread::EmulatorThread,
    events::{AdapterEvent, AdapterEventWrapper, AdapterEventListeners, EventHandlerId, EventSendError, Repeat, Sender},
    remote::{InternalEmulatorAdapter, RemoteEmulator, RemoteEmulatorChannel},
};

use std::{
    any::TypeId,
    collections::HashMap,
    convert::{TryFrom, TryInto},
    rc::Rc,
    sync::{atomic::{AtomicU64, Ordering}, mpsc},
};

pub(crate) struct GlibEmulatorChannel {
    tx: mpsc::Sender<(CommandId, EmulatorCommand)>,
    rx: Option<glib::Receiver<EmulatorThreadOutput>>,
    ctx: glib::MainContext,
    next_id: AtomicU64,
}

impl GlibEmulatorChannel {
    pub(crate) fn new(ctx: glib::MainContext) -> GlibEmulatorChannel {
        let (_thread_handle, tx, rx) = EmulatorThread::start();
        GlibEmulatorChannel {
            tx,
            ctx,
            rx: Some(rx),
            next_id: AtomicU64::new(0),
        }
    }
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

impl RemoteEmulatorChannel for GlibEmulatorChannel {
    fn send(&self, cmd: EmulatorCommand) -> CommandId {
        let next_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let cmd_id = CommandId(next_id);
        self.tx
            .send((cmd_id, cmd))
            .expect("Could not send command to emulator");
        cmd_id
    }

    fn handle_output(&mut self, f: Box<dyn Fn(EmulatorThreadOutput) -> Repeat>) {
        self.rx
            .take()
            .expect("Attempted to register two output sources")
            .attach(Some(&self.ctx), move |output| f(output).into());
    }
}



pub(crate) struct GlibAdapterEventListeners {
    listeners: HashMap<TypeId, HashMap<EventHandlerId, Box<dyn Sender<AdapterEvent>>>>,
    context: glib::MainContext,
    next_listener_id: u64,
}

impl GlibAdapterEventListeners {
    pub(crate) fn new(context: glib::MainContext) -> GlibAdapterEventListeners {
        GlibAdapterEventListeners {
            context,
            listeners: HashMap::new(),
            next_listener_id: 0,
        }
    }
}

impl AdapterEventListeners for GlibAdapterEventListeners {
    fn on(&mut self, event_type_id: TypeId, f: Box<dyn Fn(AdapterEvent) -> Repeat + 'static>) -> EventHandlerId {
        let event_handler_id = EventHandlerId(self.next_listener_id);
        let (tx, rx) = glib::MainContext::channel::<AdapterEvent>(glib::PRIORITY_DEFAULT);
        let wrapped = Box::new(tx);
        let map = self
            .listeners
            .entry(event_type_id)
            .or_insert_with(|| HashMap::new());
        map.insert(event_handler_id, wrapped);
        self.next_listener_id += 1;

        rx.attach(Some(&self.context), move |evt| f(evt).into());

        event_handler_id
    }

    fn emit(&mut self, evt: AdapterEvent) {
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

pub(crate) fn glib_remote_emulator(context: glib::MainContext) -> Rc<RemoteEmulator> {
    let channel = GlibEmulatorChannel::new(context.clone());
    let glib_listeners = GlibAdapterEventListeners::new(context.clone());
    let adapter = InternalEmulatorAdapter::new(Box::new(channel), AdapterEventWrapper::new(Box::new(glib_listeners)));
    Rc::new(RemoteEmulator::new(adapter))
}