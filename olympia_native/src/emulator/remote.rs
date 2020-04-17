use crate::emulator::{
    commands,
    commands::{
        CommandId, Continue, EmulatorCommand, EmulatorResponse, EmulatorThreadOutput, ExecMode,
        ExecTime, LoadRomError, QueryMemoryResponse, QueryRegistersResponse, UiBreakpoint,
    },
    emu_thread::EmulatorThread,
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

struct AdapterEventListeners {
    mode_change: HashMap<u32, glib::Sender<ExecMode>>,
    step: HashMap<u32, glib::Sender<()>>,
    next_listener_id: u32,
}

impl AdapterEventListeners {
    fn new() -> AdapterEventListeners {
        AdapterEventListeners {
            mode_change: HashMap::new(),
            step: HashMap::new(),
            next_listener_id: 0,
        }
    }

    fn add_step(&mut self, tx: glib::Sender<()>) {
        self.step.insert(self.next_listener_id, tx);
        self.next_listener_id += 1;
    }

    fn add_mode_change(&mut self, tx: glib::Sender<ExecMode>) {
        self.mode_change.insert(self.next_listener_id, tx);
        self.next_listener_id += 1;
    }

    fn notify_listeners<T: Clone>(listeners: &mut HashMap<u32, glib::Sender<T>>, value: T) {
        let mut listener_ids_to_remove = Vec::new();
        for (id, listener) in listeners.iter_mut() {
            let send_result = listener.send(value.clone());
            if send_result.is_err() {
                listener_ids_to_remove.push(id.clone());
                eprintln!("Removing listener {} due to closed channel", id);
            }
        }
        for id in listener_ids_to_remove {
            listeners.remove(&id);
        }
    }

    fn notify_mode_change(&mut self, mode: ExecMode) {
        AdapterEventListeners::notify_listeners(&mut self.mode_change, mode.clone());
    }

    fn notify_step(&mut self) {
        AdapterEventListeners::notify_listeners(&mut self.step, ());
    }
}

pub(crate) trait RemoteEmulatorChannel {
    fn send(&self, cmd: EmulatorCommand) -> CommandId;
    fn handle_output(&mut self, f: Box<dyn Fn(EmulatorThreadOutput) -> Continue>);
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

    fn handle_output(&mut self, f: Box<dyn Fn(EmulatorThreadOutput) -> Continue>) {
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
            clone!(@strong pending_responses as responses, @strong event_listeners => move |output| {
                let mut pending_responses = responses.borrow_mut();
                match output {
                    EmulatorThreadOutput::Response(id, resp) => {
                        pending_responses.responses.insert(id, resp);
                        if let Some(waker) = pending_responses.wakers.remove(&id) {
                            waker.wake();
                        }
                    }
                    EmulatorThreadOutput::ModeChange(mode) => {
                        event_listeners.borrow_mut().notify_mode_change(mode);
                    },
                    _ => {},
                }
                Continue(true)
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

    pub(crate) fn on_step(&self, tx: glib::Sender<()>) {
        self.adapter.event_listeners.borrow_mut().add_step(tx)
    }

    pub(crate) fn on_mode_change(&self, tx: glib::Sender<ExecMode>) {
        self.adapter
            .event_listeners
            .borrow_mut()
            .add_mode_change(tx)
    }

    fn apply_mode(&self, mode: ExecMode) {
        self.mode.replace(mode.clone());
        self.adapter
            .event_listeners
            .borrow_mut()
            .notify_mode_change(mode);
    }

    pub(crate) async fn load_rom(&self, path: PathBuf) -> Result<(), LoadRomError> {
        let result: Result<(), LoadRomError> = self
            .adapter
            .send_command(EmulatorCommand::LoadRom(path))
            .await;

        if result.is_ok() {
            self.apply_mode(ExecMode::Paused);
        }

        self.adapter.event_listeners.borrow_mut().notify_step();

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
        self.adapter.event_listeners.borrow_mut().notify_step();
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
    use olympia_engine::debug::Breakpoint;
    use olympia_engine::registers::WordRegister;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    fn rom_path() -> PathBuf {
        let mut path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_owned();
        path.push("res");
        path.push("fizzbuzz.gb");
        path
    }

    #[test]
    fn test_load_rom() {
        let context = glib::MainContext::new();
        context.acquire();
        let channel = Box::new(GlibEmulatorChannel::with_context(context.clone()));
        let emu = RemoteEmulator::new(channel);
        let resp = context.block_on(emu.load_rom(rom_path()));
        assert_eq!(resp, Ok(()));
    }

    #[test]
    fn test_load_rom_error() {
        let context = glib::MainContext::new();
        context.acquire();
        let channel = Box::new(GlibEmulatorChannel::with_context(context.clone()));
        let emu = RemoteEmulator::new(channel);
        let mut path = rom_path();
        path.push("doesnotexist");
        let resp = context.block_on(emu.load_rom(path));
        assert!(matches!(resp, Err(LoadRomError::Unreadable(_))));
    }

    fn track_emulator_event<T: 'static>(
        ctx: &glib::MainContext,
    ) -> (glib::Sender<T>, Rc<RefCell<Vec<T>>>) {
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let tracked = Rc::new(RefCell::new(Vec::new()));
        rx.attach(
            Some(ctx),
            clone!(@strong tracked => move |arg| {
                tracked.borrow_mut().push(arg);
                glib::Continue(true)
            }),
        );
        (tx, tracked)
    }

    fn wait_for_task<T>(ctx: &glib::MainContext, t: impl Future<Output = T>) -> T {
        let start_time = Instant::now();
        let result = ctx.block_on(t);
        let timeout = Duration::from_millis(1000);
        while ctx.pending() {
            if start_time.elapsed() > timeout {
                panic!("Timeout of {:?} elapsed", timeout);
            }
            ctx.iteration(true);
        }
        result
    }

    fn digest_events(ctx: &glib::MainContext) {
        let start_time = Instant::now();
        let timeout = Duration::from_millis(1000);
        while ctx.pending() {
            if start_time.elapsed() > timeout {
                panic!("Timeout of {:?} elapsed", timeout);
            }
            ctx.iteration(true);
        }
    }

    #[test]
    fn test_load_starts_paused() {
        let context = glib::MainContext::new();
        context.acquire();
        let channel = Box::new(GlibEmulatorChannel::with_context(context.clone()));
        let emu = RemoteEmulator::new(channel);
        let (tx, events) = track_emulator_event(&context);
        emu.on_mode_change(tx);
        let task = async {
            emu.load_rom(rom_path()).await.unwrap();
        };
        wait_for_task(&context, task);
        assert_eq!(events.borrow().clone(), vec![ExecMode::Paused]);
    }

    #[test]
    fn test_step_notify() {
        let context = glib::MainContext::new();
        context.acquire();
        let channel = Box::new(GlibEmulatorChannel::with_context(context.clone()));
        let emu = RemoteEmulator::new(channel);
        let (tx, events) = track_emulator_event(&context);
        emu.on_step(tx);
        let task = async {
            emu.load_rom(rom_path()).await.unwrap();
            emu.step().await.unwrap();
        };
        wait_for_task(&context, task);
        assert_eq!(events.borrow().clone(), vec![(), ()]);
    }

    #[test]
    fn test_query_memory() {
        let context = glib::MainContext::new();
        context.acquire();
        let channel = Box::new(GlibEmulatorChannel::with_context(context.clone()));
        let emu = RemoteEmulator::new(channel);
        let task = async {
            emu.load_rom(rom_path()).await.unwrap();
            emu.step().await.unwrap();
            emu.query_memory(0x00, 0x04).await
        };
        let memory_result = wait_for_task(&context, task);
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
    fn test_query_register() {
        let context = glib::MainContext::new();
        context.acquire();
        let channel = Box::new(GlibEmulatorChannel::with_context(context.clone()));
        let emu = RemoteEmulator::new(channel);
        let task = async {
            emu.load_rom(rom_path()).await.unwrap();
            emu.step().await.unwrap();
            emu.query_registers().await
        };
        let register_result = wait_for_task(&context, task);
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
    fn test_run_to_breakpoint() {
        let context = glib::MainContext::new();
        context.acquire();
        let channel = Box::new(GlibEmulatorChannel::with_context(context.clone()));
        let emu = RemoteEmulator::new(channel);
        let (tx, events) = track_emulator_event(&context);
        emu.on_mode_change(tx);
        let bp: UiBreakpoint = Breakpoint::new(WordRegister::PC.into(), 0x150).into();
        let task = async {
            emu.load_rom(rom_path()).await.unwrap();
            emu.add_breakpoint(bp.clone()).await.unwrap();
        };
        wait_for_task(&context, task);
        let play_task = async {
            emu.set_mode(ExecMode::Standard).await.unwrap();
        };
        wait_for_task(&context, play_task);
        std::thread::sleep(Duration::from_millis(200));
        digest_events(&context);
        let emulation_time = wait_for_task(&context, emu.exec_time()).unwrap();
        // 1 cycle for NOP, 4 for JUMP
        let actual_gb_time =
            Duration::from_secs_f64(5.0 / f64::from(olympia_engine::gameboy::CYCLE_FREQ));
        assert!(dbg!(Duration::from(emulation_time)) >= dbg!(actual_gb_time));
        assert_eq!(
            events.borrow().clone(),
            vec![
                ExecMode::Paused,
                ExecMode::Standard,
                ExecMode::HitBreakpoint(bp)
            ]
        );
    }

    #[test]
    fn test_ff_to_breakpoint() {
        let context = glib::MainContext::new();
        context.acquire();
        let channel = Box::new(GlibEmulatorChannel::with_context(context.clone()));
        let emu = RemoteEmulator::new(channel);
        let (tx, events) = track_emulator_event(&context);
        emu.on_mode_change(tx);
        let bp: UiBreakpoint = Breakpoint::new(WordRegister::PC.into(), 0x150).into();
        let task = async {
            emu.load_rom(rom_path()).await.unwrap();
            emu.add_breakpoint(bp.clone()).await.unwrap();
        };
        wait_for_task(&context, task);
        let play_task = async {
            emu.set_mode(ExecMode::Uncapped).await.unwrap();
        };
        wait_for_task(&context, play_task);
        std::thread::sleep(Duration::from_millis(200));
        digest_events(&context);
        assert_eq!(
            events.borrow().clone(),
            vec![
                ExecMode::Paused,
                ExecMode::Uncapped,
                ExecMode::HitBreakpoint(bp)
            ]
        );
        // TODO: Test in release mode only, debug builds too slow
        // let emulation_time: ExecTime = wait_for_task(&context, emu.exec_time()).unwrap();
        // // 1 cycle for NOP, 4 for JUMP
        // let actual_gb_time = Duration::from_secs_f64(5.0 / f64::from(olympia_engine::gameboy::CYCLE_FREQ));
        // assert!(dbg!(Duration::from(emulation_time)) <= dbg!(actual_gb_time));
    }
}
