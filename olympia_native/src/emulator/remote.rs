use crate::{
    emulator::{
        commands,
        commands::{
            CommandId, EmulatorCommand, EmulatorResponse, EmulatorThreadOutput, ExecMode, ExecTime,
            LoadRomError, QueryMemoryResponse, QueryRegistersResponse, Repeat, UiBreakpoint,
        },
        emu_thread::EmulatorThread,
        events::{
            AdapterEvent, AdapterEventListeners, EventHandlerId, ManualStepEvent, ModeChangeEvent,
            RomLoadedEvent,
        },
    },
    utils::HasGlibContext,
};

use glib::clone;

use std::{
    cell::RefCell,
    collections::HashMap,
    convert::{TryFrom, TryInto},
    future::Future,
    marker::PhantomData,
    path::PathBuf,
    pin::Pin,
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    task::{Context, Poll, Waker},
};

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

pub(crate) struct EmulatorCommandExecution<T> {
    id: CommandId,
    command: EmulatorCommand,
    pending_responses: Rc<RefCell<PendingResponses>>,
    response_type: PhantomData<T>,
}

impl<T> Future for EmulatorCommandExecution<T>
where
    T: TryFrom<EmulatorResponse> + std::fmt::Debug,
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

pub(crate) trait RemoteEmulatorChannel {
    fn send(&self, cmd: EmulatorCommand) -> CommandId;
    fn handle_output(&mut self, f: Box<dyn Fn(EmulatorThreadOutput) -> Repeat>);
}

pub(crate) struct GlibEmulatorChannel {
    tx: mpsc::Sender<(CommandId, EmulatorCommand)>,
    rx: Option<glib::Receiver<EmulatorThreadOutput>>,
    ctx: glib::MainContext,
    next_id: AtomicU64,
}

impl GlibEmulatorChannel {
    pub(crate) fn new() -> GlibEmulatorChannel {
        GlibEmulatorChannel::with_context(glib::MainContext::default())
    }

    pub(crate) fn with_context(ctx: glib::MainContext) -> GlibEmulatorChannel {
        let (_thread_handle, tx, rx) = EmulatorThread::start();
        GlibEmulatorChannel {
            tx,
            ctx,
            rx: Some(rx),
            next_id: AtomicU64::new(0),
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

pub struct InternalEmulatorAdapter {
    channel: Box<dyn RemoteEmulatorChannel>,
    pending_responses: Rc<RefCell<PendingResponses>>,
    event_listeners: Rc<RefCell<AdapterEventListeners>>,
}

impl InternalEmulatorAdapter {
    fn new(mut channel: Box<dyn RemoteEmulatorChannel>) -> InternalEmulatorAdapter {
        let pending_responses = Rc::new(RefCell::new(PendingResponses::default()));
        let event_listeners = Rc::new(RefCell::new(AdapterEventListeners::new()));
        channel.handle_output(Box::new(
            clone!(@weak pending_responses as responses, @weak event_listeners => @default-return Repeat(false), move |output| {
                let mut pending_responses = responses.borrow_mut();
                match output {
                    EmulatorThreadOutput::Response(id, resp) => {
                        pending_responses.responses.insert(id, resp);
                        if let Some(waker) = pending_responses.wakers.remove(&id) {
                            waker.wake();
                        }
                    }
                    EmulatorThreadOutput::ModeChange(change_event) => {
                        event_listeners.borrow_mut().emit(change_event);
                    },
                    EmulatorThreadOutput::Event(event) => {
                        event_listeners.borrow_mut().emit(event);
                    },
                    _ => {}
                }
                Repeat(true)
            }),
        ));
        InternalEmulatorAdapter {
            channel,
            pending_responses,
            event_listeners,
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

pub(crate) struct RemoteEmulator {
    adapter: InternalEmulatorAdapter,
    mode: RefCell<ExecMode>,
    cached_registers: RefCell<QueryRegistersResponse>,
}

impl RemoteEmulator {
    pub(crate) fn new(remote_channel: Box<dyn RemoteEmulatorChannel>) -> RemoteEmulator {
        RemoteEmulator {
            adapter: InternalEmulatorAdapter::new(remote_channel),
            mode: RefCell::new(ExecMode::Unloaded),
            cached_registers: RefCell::new(QueryRegistersResponse::default()),
        }
    }

    pub(crate) fn on<E, F>(&self, context: &glib::MainContext, f: F) -> EventHandlerId
    where
        E: TryFrom<AdapterEvent> + 'static,
        F: Fn(E) -> Repeat + 'static,
    {
        self.adapter
            .event_listeners
            .borrow_mut()
            .on(context, f)
    }

    pub(crate) fn add_listener<E, F, W>(&self, widget: Rc<W>, handler: F) -> EventHandlerId
    where
        W: HasGlibContext + 'static,
        F: Fn(Rc<W>, E) -> () + 'static,
        E: TryFrom<AdapterEvent> + 'static,
    {
        self.on(
            widget.get_context(),
            clone!(@weak widget => @default-return Repeat(false), move |evt| {
                handler(widget, evt);
                Repeat(true)
            }),
        )
    }

    fn apply_mode(&self, new_mode: ExecMode) {
        let old_mode = self.mode.replace(new_mode.clone());
        self.adapter
            .event_listeners
            .borrow_mut()
            .emit(ModeChangeEvent { old_mode, new_mode });
    }

    pub(crate) async fn load_rom(&self, path: PathBuf) -> Result<(), LoadRomError> {
        let result: Result<(), LoadRomError> = self
            .adapter
            .send_command(EmulatorCommand::LoadRom(path))
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

    pub(crate) async fn query_memory(
        &self,
        start_addr: u16,
        end_addr: u16,
    ) -> commands::Result<QueryMemoryResponse> {
        self.adapter
            .send_command(EmulatorCommand::QueryMemory(start_addr, end_addr))
            .await
    }

    #[allow(dead_code)]
    pub(crate) async fn exec_time(&self) -> commands::Result<ExecTime> {
        self.adapter
            .send_command(EmulatorCommand::QueryExecTime)
            .await
    }

    pub(crate) async fn query_registers(&self) -> commands::Result<QueryRegistersResponse> {
        let result: Result<QueryRegistersResponse, commands::Error> = self
            .adapter
            .send_command(EmulatorCommand::QueryRegisters)
            .await;
        if let Ok(ref registers) = result {
            self.cached_registers.replace(registers.clone());
        }
        result
    }

    pub(crate) fn pc(&self) -> u16 {
        self.cached_registers.borrow().pc
    }

    pub(crate) async fn step(&self) -> commands::Result<()> {
        let result = self.adapter.send_command(EmulatorCommand::Step).await;
        self.adapter
            .event_listeners
            .borrow_mut()
            .emit(ManualStepEvent);
        result
    }

    pub(crate) async fn set_mode(&self, mode: ExecMode) -> Result<ExecMode, ()> {
        let result: Result<ExecMode, ()> = self
            .adapter
            .send_command(EmulatorCommand::SetMode(mode.clone()))
            .await;

        if result.is_ok() {
            self.apply_mode(mode);
        }

        result
    }

    pub(crate) async fn add_breakpoint(&self, breakpoint: UiBreakpoint) -> Result<(), ()> {
        self.adapter
            .send_command(EmulatorCommand::AddBreakpoint(breakpoint))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils;
    use olympia_engine::debug::Breakpoint;
    use olympia_engine::registers::WordRegister;
    use std::time::Duration;

    #[test]
    fn test_load_rom() {
        let context = test_utils::setup_context();
        let emu = test_utils::get_unloaded_remote_emu(context.clone());
        let (f, events) = track_event();
        emu.on::<RomLoadedEvent, _>(&context, f);
        let task = async {
            emu.load_rom(test_utils::fizzbuzz_path()).await
        };
        let resp = test_utils::wait_for_task(&context, task);
        assert_eq!(resp, Ok(()));
        assert_eq!(events.borrow().clone(), vec![RomLoadedEvent]);
    }

    #[test]
    fn test_load_rom_error() {
        let context = test_utils::setup_context();
        let emu = test_utils::get_unloaded_remote_emu(context.clone());
        let mut path = test_utils::fizzbuzz_path();
        path.push("doesnotexist");
        let resp = context.block_on(emu.load_rom(path));
        assert!(matches!(resp, Err(LoadRomError::Unreadable(_))));
    }

    fn track_event<T: 'static>() -> (impl Fn(T) -> Repeat + 'static, Rc<RefCell<Vec<T>>>) {
        let tracked = Rc::new(RefCell::new(Vec::new()));
        let other_ref = tracked.clone();
        let f = move |arg| {
            other_ref.borrow_mut().push(arg);
            Repeat(true)
        };
        (f, tracked)
    }

    #[test]
    fn test_load_starts_paused() {
        let context = test_utils::setup_context();
        let emu = test_utils::get_unloaded_remote_emu(context.clone());
        let (f, events) = track_event();
        emu.on::<ModeChangeEvent, _>(&context, f);
        let task = async {
            emu.load_rom(test_utils::fizzbuzz_path()).await.unwrap();
        };
        test_utils::wait_for_task(&context, task);
        assert_eq!(
            events.borrow().clone(),
            vec![ModeChangeEvent::new(ExecMode::Unloaded, ExecMode::Paused)]
        );
    }

    #[test]
    fn test_step() {
        let context = test_utils::setup_context();
        let emu = test_utils::get_unloaded_remote_emu(context.clone());
        let (f, events) = track_event();
        emu.on::<ManualStepEvent, _>(&context, f);
        let task = async {
            emu.load_rom(test_utils::fizzbuzz_path()).await.unwrap();
            emu.step().await
        };
        let step_result = test_utils::wait_for_task(&context, task);
        assert_eq!(
            events.borrow().clone(),
            vec![ManualStepEvent]
        );
        assert_eq!(step_result, Ok(()))
    }

    #[test]
    fn test_step_unloaded() {
        let context = test_utils::setup_context();
        let emu = test_utils::get_unloaded_remote_emu(context.clone());
        let task = async { emu.step().await };
        let step_result = test_utils::wait_for_task(&context, task);
        assert_eq!(step_result, Err(commands::Error::NoRomLoaded))
    }

    #[test]
    fn test_query_memory() {
        let context = test_utils::setup_context();
        let emu = test_utils::get_loaded_remote_emu(context.clone());
        let task = async {
            emu.step().await.unwrap();
            emu.query_memory(0x00, 0x04).await
        };
        let memory_result = test_utils::wait_for_task(&context, task);
        let expected_data = vec![201, 0, 0, 0, 0].into_iter().map(|x| Some(x)).collect();
        assert_eq!(
            memory_result,
            Ok(QueryMemoryResponse {
                start_addr: 0x00,
                data: expected_data
            })
        )
    }

    #[test]
    fn test_query_memory_unloaded() {
        let context = test_utils::setup_context();
        let emu = test_utils::get_unloaded_remote_emu(context.clone());
        let task = async { emu.query_memory(0x00, 0x04).await };
        let memory_result = test_utils::wait_for_task(&context, task);
        assert_eq!(memory_result, Err(commands::Error::NoRomLoaded))
    }

    #[test]
    fn test_query_register() {
        let context = test_utils::setup_context();
        let emu = test_utils::get_loaded_remote_emu(context.clone());
        let task = async {
            emu.step().await.unwrap();
            emu.query_registers().await
        };
        let register_result = test_utils::wait_for_task(&context, task);
        assert_eq!(
            register_result,
            Ok(QueryRegistersResponse {
                af: 0x01b0,
                bc: 0x0013,
                de: 0x00d8,
                hl: 0x014d,
                sp: 0xfffe,
                pc: 0x0101,
            })
        )
    }

    #[test]
    fn test_query_register_unloaded() {
        let context = test_utils::setup_context();
        let emu = test_utils::get_unloaded_remote_emu(context.clone());
        let task = async { emu.query_registers().await };
        let register_result = test_utils::wait_for_task(&context, task);
        assert_eq!(register_result, Err(commands::Error::NoRomLoaded))
    }

    #[test]
    fn test_run_to_breakpoint() {
        let context = test_utils::setup_context();
        let emu = test_utils::get_unloaded_remote_emu(context.clone());
        let (f, events) = track_event();
        emu.on::<ModeChangeEvent, _>(&context, f);
        let bp: UiBreakpoint = Breakpoint::new(WordRegister::PC.into(), 0x150).into();
        let task = async {
            emu.load_rom(test_utils::fizzbuzz_path()).await.unwrap();
            emu.add_breakpoint(bp.clone()).await.unwrap();
        };
        test_utils::wait_for_task(&context, task);
        let play_task = async {
            emu.set_mode(ExecMode::Standard).await.unwrap();
        };
        test_utils::wait_for_task(&context, play_task);
        std::thread::sleep(Duration::from_millis(200));
        test_utils::digest_events(&context);
        let emulation_time = test_utils::wait_for_task(&context, emu.exec_time()).unwrap();
        // 1 cycle for NOP, 4 for JUMP
        let actual_gb_time =
            Duration::from_secs_f64(5.0 / f64::from(olympia_engine::gameboy::CYCLE_FREQ));
        assert!(dbg!(Duration::from(emulation_time)) >= dbg!(actual_gb_time));
        assert_eq!(
            events.borrow().clone(),
            vec![
                ModeChangeEvent::new(ExecMode::Unloaded, ExecMode::Paused),
                ModeChangeEvent::new(ExecMode::Paused, ExecMode::Standard),
                ModeChangeEvent::new(ExecMode::Standard, ExecMode::HitBreakpoint(bp)),
            ]
        );
    }

    #[test]
    fn test_ff_to_breakpoint() {
        let context = test_utils::setup_context();
        let emu = test_utils::get_unloaded_remote_emu(context.clone());
        let (f, events) = track_event();
        emu.on::<ModeChangeEvent, _>(&context, f);
        let bp: UiBreakpoint = Breakpoint::new(WordRegister::PC.into(), 0x150).into();
        let task = async {
            emu.load_rom(test_utils::fizzbuzz_path()).await.unwrap();
            emu.add_breakpoint(bp.clone()).await.unwrap();
        };
        test_utils::wait_for_task(&context, task);
        let play_task = async {
            emu.set_mode(ExecMode::Uncapped).await.unwrap();
        };
        test_utils::wait_for_task(&context, play_task);
        std::thread::sleep(Duration::from_millis(200));
        test_utils::digest_events(&context);
        assert_eq!(
            events.borrow().clone(),
            vec![
                ModeChangeEvent::new(ExecMode::Unloaded, ExecMode::Paused),
                ModeChangeEvent::new(ExecMode::Paused, ExecMode::Uncapped),
                ModeChangeEvent::new(ExecMode::Uncapped, ExecMode::HitBreakpoint(bp)),
            ]
        );
        // TODO: Test in release mode only, debug builds too slow
        // let emulation_time: ExecTime = wait_for_task(&context, emu.exec_time()).unwrap();
        // // 1 cycle for NOP, 4 for JUMP
        // let actual_gb_time = Duration::from_secs_f64(5.0 / f64::from(olympia_engine::gameboy::CYCLE_FREQ));
        // assert!(dbg!(Duration::from(emulation_time)) <= dbg!(actual_gb_time));
    }
}
