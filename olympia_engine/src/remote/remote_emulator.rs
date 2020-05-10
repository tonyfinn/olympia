use crate::{
    events::{EventHandlerId, ManualStepEvent, ModeChangeEvent, Repeat, RomLoadedEvent},
    remote::{
        commands,
        commands::{
            CommandId, EmulatorCommand, EmulatorResponse, EmulatorThreadOutput, ExecMode, ExecTime,
            LoadRomError, QueryMemoryResponse, QueryRegistersResponse, UiBreakpoint,
        },
        events::{AdapterEvent, AdapterEventWrapper},
    },
};

use alloc::rc::Rc;
use core::{
    cell::RefCell,
    convert::{TryFrom, TryInto},
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use hashbrown::HashMap;

pub struct PendingResponses {
    responses: HashMap<CommandId, EmulatorResponse>,
    wakers: HashMap<CommandId, Waker>,
}

impl Default for PendingResponses {
    fn default() -> PendingResponses {
        PendingResponses {
            responses: HashMap::new(),
            wakers: HashMap::new(),
        }
    }
}

pub struct EmulatorCommandExecution<T> {
    id: CommandId,
    command: EmulatorCommand,
    pending_responses: Rc<RefCell<PendingResponses>>,
    response_type: PhantomData<T>,
}

impl<T> Future for EmulatorCommandExecution<T>
where
    T: TryFrom<EmulatorResponse> + core::fmt::Debug,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut pending_responses = self.pending_responses.borrow_mut();
        match pending_responses.responses.remove(&self.id) {
            Some(t) => {
                let formatted_response = format!("{:?}", t);
                Poll::Ready(match t.try_into() {
                    Ok(t) => {
                        cx.waker().wake_by_ref();
                        t
                    }
                    Err(_) => panic!(
                        "Invalid response recieved for command {:?}.\n\tCommand: {:?}\n\tResponse: {:?}", 
                        self.id,
                        self.command,
                        formatted_response
                    ),
                })
            }
            None => {
                pending_responses.wakers.insert(self.id, cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

pub trait RemoteEmulatorChannel {
    fn send(&self, cmd: EmulatorCommand) -> CommandId;
    fn handle_output(&mut self, f: Box<dyn Fn(EmulatorThreadOutput) -> Repeat>);
}

pub struct InternalEmulatorAdapter {
    channel: Box<dyn RemoteEmulatorChannel>,
    pending_responses: Rc<RefCell<PendingResponses>>,
    event_listeners: Rc<RefCell<AdapterEventWrapper>>,
}

impl InternalEmulatorAdapter {
    pub fn new(
        channel: Box<dyn RemoteEmulatorChannel>,
        listeners: AdapterEventWrapper,
    ) -> InternalEmulatorAdapter {
        let pending_responses = Rc::new(RefCell::new(PendingResponses::default()));
        let event_listeners = Rc::new(RefCell::new(listeners));
        let mut adapter = InternalEmulatorAdapter {
            channel,
            pending_responses,
            event_listeners,
        };
        adapter.connect_output_channel();
        adapter
    }

    fn connect_output_channel(&mut self) {
        let pending_responses = Rc::downgrade(&self.pending_responses);
        let event_listeners = Rc::downgrade(&self.event_listeners);
        self.channel.handle_output(Box::new(move |output| {
            let pending_responses = pending_responses.upgrade();
            let event_listeners = event_listeners.upgrade();

            match (pending_responses, event_listeners) {
                (Some(p), Some(e)) => {
                    InternalEmulatorAdapter::handle_output(&p, &e, output);
                    Repeat(true)
                }
                _ => Repeat(false),
            }
        }));
    }

    fn handle_output(
        pending_responses: &RefCell<PendingResponses>,
        event_listeners: &RefCell<AdapterEventWrapper>,
        output: EmulatorThreadOutput,
    ) {
        match output {
            EmulatorThreadOutput::Response(id, resp) => {
                let mut pending_responses = pending_responses.borrow_mut();
                pending_responses.responses.insert(id, resp);
                if let Some(waker) = pending_responses.wakers.remove(&id) {
                    waker.wake();
                }
            }
            EmulatorThreadOutput::ModeChange(change_event) => {
                event_listeners.borrow_mut().emit(change_event);
            }
            EmulatorThreadOutput::Event(event) => {
                event_listeners.borrow_mut().emit(event);
            }
            _ => {}
        }
    }

    fn send_command<T>(&self, cmd: EmulatorCommand) -> EmulatorCommandExecution<T> {
        let id = self.channel.send(cmd.clone());
        EmulatorCommandExecution {
            id,
            command: cmd,
            pending_responses: self.pending_responses.clone(),
            response_type: PhantomData,
        }
    }
}

pub struct RemoteEmulator {
    adapter: InternalEmulatorAdapter,
    mode: RefCell<ExecMode>,
    cached_registers: RefCell<QueryRegistersResponse>,
}

impl RemoteEmulator {
    pub fn new(adapter: InternalEmulatorAdapter) -> RemoteEmulator {
        RemoteEmulator {
            adapter: adapter,
            mode: RefCell::new(ExecMode::Unloaded),
            cached_registers: RefCell::new(QueryRegistersResponse::default()),
        }
    }

    pub fn on<E, F>(&self, f: F) -> EventHandlerId
    where
        E: TryFrom<AdapterEvent> + 'static,
        F: Fn(E) -> Repeat + 'static,
    {
        self.adapter.event_listeners.borrow_mut().on(f)
    }

    pub fn on_widget<E, F, W>(&self, widget: Rc<W>, handler: F) -> EventHandlerId
    where
        W: 'static,
        F: Fn(Rc<W>, E) -> () + 'static,
        E: TryFrom<AdapterEvent> + 'static,
    {
        let weak = Rc::downgrade(&widget);
        self.on(move |evt| match weak.upgrade() {
            Some(w) => {
                handler(w, evt);
                Repeat(true)
            }
            None => Repeat(false),
        })
    }

    fn apply_mode(&self, new_mode: ExecMode) {
        let old_mode = self.mode.replace(new_mode.clone());
        self.adapter
            .event_listeners
            .borrow_mut()
            .emit(ModeChangeEvent { old_mode, new_mode });
    }

    pub async fn load_rom(&self, data: Vec<u8>) -> Result<(), LoadRomError> {
        let result: Result<(), LoadRomError> = self
            .adapter
            .send_command(EmulatorCommand::LoadRom(data))
            .await;

        if result.is_ok() {
            self.apply_mode(ExecMode::Paused);
        }

        self.adapter
            .event_listeners
            .borrow_mut()
            .emit(RomLoadedEvent);

        result
    }

    pub async fn query_memory(
        &self,
        start_addr: u16,
        end_addr: u16,
    ) -> commands::Result<QueryMemoryResponse> {
        self.adapter
            .send_command(EmulatorCommand::QueryMemory(start_addr, end_addr))
            .await
    }

    #[allow(dead_code)]
    pub async fn exec_time(&self) -> commands::Result<ExecTime> {
        self.adapter
            .send_command(EmulatorCommand::QueryExecTime)
            .await
    }

    pub async fn query_registers(&self) -> commands::Result<QueryRegistersResponse> {
        let result: Result<QueryRegistersResponse, commands::Error> = self
            .adapter
            .send_command(EmulatorCommand::QueryRegisters)
            .await;
        if let Ok(ref registers) = result {
            self.cached_registers.replace(registers.clone());
        }
        result
    }

    pub fn pc(&self) -> u16 {
        self.cached_registers.borrow().pc
    }

    pub async fn step(&self) -> commands::Result<()> {
        let result = self.adapter.send_command(EmulatorCommand::Step).await;
        self.adapter
            .event_listeners
            .borrow_mut()
            .emit(ManualStepEvent);
        result
    }

    pub async fn set_mode(&self, mode: ExecMode) -> Result<ExecMode, ()> {
        let result: Result<ExecMode, ()> = self
            .adapter
            .send_command(EmulatorCommand::SetMode(mode.clone()))
            .await;

        if result.is_ok() {
            self.apply_mode(mode);
        }

        result
    }

    pub async fn add_breakpoint(&self, breakpoint: UiBreakpoint) -> Result<(), ()> {
        self.adapter
            .send_command(EmulatorCommand::AddBreakpoint(breakpoint))
            .await
    }
}

mod test {
    // Note that engine doesn't contain an implementation of remote emulator
    // See olympia_native::emulator::glib for most of the tests for this module
}
