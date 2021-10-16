use crate::{
    events::{EventHandlerId, ManualStepEvent, Repeat, RomLoadedEvent},
    monitor::{Breakpoint, BreakpointIdentifier},
    remote::{
        commands,
        commands::{
            CommandId, EmulatorCommand, EmulatorResponse, ExecMode, ExecTime, LoadRomError,
            QueryMemoryResponse, QueryRegistersResponse, RemoteEmulatorOutput,
            ToggleBreakpointResponse,
        },
        events::{AdapterEventWrapper, Event as RemoteEvent, RemoteEventListeners},
    },
};

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{
    cell::RefCell,
    convert::{TryFrom, TryInto},
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use hashbrown::HashMap;

use super::commands::{AddBreakpointResponse, RemoveBreakpointRespnse};

pub(crate) struct PendingResponses {
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

/// A command executing in a remote emulator
///
/// This can be `await`-ed for the output of the command
/// if you have an async executor.
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
                    },
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

/// Transfers events/commands to remote emulator
pub trait RemoteEmulatorChannel {
    /// Send a command to the remote emulator
    fn send(&self, cmd: EmulatorCommand) -> CommandId;
    /// Handle output from the remote emulator
    fn handle_output(&mut self, f: Box<dyn Fn(RemoteEmulatorOutput) -> Repeat>);
}

struct InternalEmulatorAdapter {
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
        output: RemoteEmulatorOutput,
    ) {
        match output {
            RemoteEmulatorOutput::Response(id, resp) => {
                let mut pending_responses = pending_responses.borrow_mut();
                pending_responses.responses.insert(id, resp);
                if let Some(waker) = pending_responses.wakers.remove(&id) {
                    waker.wake();
                }
            }
            RemoteEmulatorOutput::Event(event) => {
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

/// An emulator that is executing elsewhere
pub struct RemoteEmulator {
    adapter: InternalEmulatorAdapter,
    cached_registers: RefCell<QueryRegistersResponse>,
}

impl RemoteEmulator {
    pub fn new(
        events: Box<dyn RemoteEventListeners>,
        channel: Box<dyn RemoteEmulatorChannel>,
    ) -> RemoteEmulator {
        let wrapper = AdapterEventWrapper::new(events);
        let adapter = InternalEmulatorAdapter::new(channel, wrapper);
        RemoteEmulator {
            adapter,
            cached_registers: RefCell::new(QueryRegistersResponse::default()),
        }
    }

    /// Listen to events from the remote emulator
    pub fn on<E, F>(&self, f: F) -> EventHandlerId
    where
        E: TryFrom<RemoteEvent> + 'static,
        F: Fn(E) -> Repeat + 'static,
    {
        self.adapter.event_listeners.borrow_mut().on(f)
    }

    /// Listen to events and pass the widget held in a weakref to the callback.
    ///
    /// The Rc provided is downgraded to a weakref. When the event handler is called,
    /// the weakref is attempted to be upgraded to a full reference. If the widget
    /// no longer exists, the event handler is removed, otherwise the callback
    /// is called with the widget reference and event
    pub fn on_widget<E, F, W>(&self, widget: Rc<W>, handler: F) -> EventHandlerId
    where
        W: 'static,
        F: Fn(Rc<W>, E) + 'static,
        E: TryFrom<RemoteEvent> + 'static,
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

    /// Load a given ROM into the remote emulator
    pub async fn load_rom(&self, data: Vec<u8>) -> Result<(), LoadRomError> {
        let result: Result<(), LoadRomError> = self
            .adapter
            .send_command(EmulatorCommand::LoadRom(data))
            .await;

        self.adapter
            .event_listeners
            .borrow_mut()
            .emit(RomLoadedEvent);

        result
    }

    /// Query the data in a given memory range
    pub async fn query_memory(
        &self,
        start_addr: u16,
        end_addr: u16,
    ) -> commands::Result<QueryMemoryResponse> {
        self.adapter
            .send_command(EmulatorCommand::QueryMemory(start_addr, end_addr))
            .await
    }

    /// Query how long the emulator has been running.
    pub async fn exec_time(&self) -> commands::Result<ExecTime> {
        self.adapter
            .send_command(EmulatorCommand::QueryExecTime)
            .await
    }

    /// Query the state of all registers in the system
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

    /// Convenience method to find the last recorded PC
    pub fn cached_pc(&self) -> u16 {
        self.cached_registers.borrow().pc
    }

    /// Run a single CPU instruction in the remote emulator
    pub async fn step(&self) -> commands::Result<()> {
        let result = self.adapter.send_command(EmulatorCommand::Step).await;
        self.adapter
            .event_listeners
            .borrow_mut()
            .emit(ManualStepEvent);
        result
    }

    /// Set the running mode to the given exec mode
    pub async fn set_mode(&self, mode: ExecMode) -> Result<ExecMode, ()> {
        let result: Result<ExecMode, ()> = self
            .adapter
            .send_command(EmulatorCommand::SetMode(mode.clone()))
            .await;
        result
    }

    /// Add a breakpoint to the remote emulator
    pub async fn add_breakpoint(
        &self,
        breakpoint: Breakpoint,
    ) -> Result<AddBreakpointResponse, ()> {
        self.adapter
            .send_command(EmulatorCommand::AddBreakpoint(breakpoint))
            .await
    }

    /// Set a breakpoint to a given active state
    pub async fn set_breakpoint_state(
        &self,
        id: BreakpointIdentifier,
        state: bool,
    ) -> Result<ToggleBreakpointResponse, ()> {
        self.adapter
            .send_command(EmulatorCommand::SetBreakpointActive(id, state))
            .await
    }

    /// Remove a breakpoint from the remote emulator
    pub async fn remove_breakpoint(
        &self,
        id: BreakpointIdentifier,
    ) -> Result<RemoveBreakpointRespnse, ()> {
        self.adapter
            .send_command(EmulatorCommand::RemoveBreakpoint(id))
            .await
    }
}

mod test {
    // Note that engine doesn't contain an implementation of remote emulator
    // See olympia_native::emulator::glib for most of the tests for this module
}
