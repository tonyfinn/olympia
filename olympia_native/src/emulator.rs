use derive_more::{From, TryInto};
use glib::clone;
use olympia_engine::{
    debug::Breakpoint,
    events::Event as EngineEvent,
    gameboy::{GameBoy, GameBoyModel, StepError},
    registers::WordRegister,
    rom::{Cartridge, CartridgeError},
};
use std::{
    cell::RefCell,
    collections::HashMap,
    convert::{TryFrom, TryInto},
    future::Future,
    marker::PhantomData,
    path::{Path, PathBuf},
    pin::Pin,
    rc::Rc,
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc,
    },
    task::{Context, Poll, Waker},
    thread,
    time::Duration,
    vec::Vec,
};
use thiserror::Error;

#[derive(Debug, Clone)]
pub(crate) struct UiBreakpoint {
    pub(crate) active: bool,
    pub(crate) breakpoint: Breakpoint,
}

impl From<Breakpoint> for UiBreakpoint {
    fn from(breakpoint: Breakpoint) -> UiBreakpoint {
        UiBreakpoint {
            active: true,
            breakpoint,
        }
    }
}

#[derive(Error, Debug)]
pub(crate) enum LoadRomError {
    #[error("Could not parse ROM: {0}")]
    InvalidRom(#[from] CartridgeError),
    #[error("Could not load ROM: {0}")]
    Unreadable(#[from] std::io::Error),
}

impl PartialEq for LoadRomError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LoadRomError::InvalidRom(cart_error), LoadRomError::InvalidRom(other_cart_error)) => {
                cart_error == other_cart_error
            }
            (LoadRomError::Unreadable(err), LoadRomError::Unreadable(other_err)) => {
                err.kind() == other_err.kind()
            }
            _ => false,
        }
    }
}

impl Eq for LoadRomError {}

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub(crate) struct QueryRegistersResponse {
    af: u16,
    bc: u16,
    de: u16,
    hl: u16,
    sp: u16,
    pc: u16,
}

impl QueryRegistersResponse {
    pub(crate) fn read_u16(&self, register: WordRegister) -> u16 {
        match register {
            WordRegister::AF => self.af,
            WordRegister::BC => self.bc,
            WordRegister::DE => self.de,
            WordRegister::HL => self.hl,
            WordRegister::SP => self.sp,
            WordRegister::PC => self.pc,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct QueryMemoryResponse {
    pub start_addr: u16,
    pub data: Vec<Option<u8>>,
}

#[derive(Debug)]
pub(crate) enum EmulatorCommand {
    /// Load a rom from a given file path
    LoadRom(PathBuf),
    /// Query all registers
    QueryRegisters,
    /// Query memory from the start address (inclusive)
    /// to end address (inclusive)
    QueryMemory(u16, u16),
    /// Run a single step
    Step,
    /// Set the exec mode - paused, 1x speed or fast forward
    SetMode(ExecMode),
    /// Add a breakpoint
    AddBreakpoint(UiBreakpoint),
}

#[derive(Debug, From, TryInto, PartialEq, Eq)]
pub(crate) enum EmulatorResponse {
    LoadRom(Result<(), LoadRomError>),
    QueryRegisters(Result<QueryRegistersResponse, Error>),
    QueryMemory(Result<QueryMemoryResponse, Error>),
    #[from(ignore)]
    Step(Result<(), Error>),
    #[from(ignore)]
    SetMode(Result<(), Error>),
    AddBreakpoint(Result<(), ()>),
}

#[derive(Error, Debug, PartialEq, Eq)]
pub(crate) enum Error {
    #[error("Error during emulation: {0}")]
    Exec(#[from] StepError),
    #[error("Failed loading ROM: {0}")]
    Load(#[from] LoadRomError),
    #[error("Action cannot be performed without a ROM loaded")]
    NoRomLoaded,
}

#[derive(From, TryInto)]
pub(crate) enum EmulatorThreadOutput {
    Event(EngineEvent),
    Error(Error),
    Response(u32, EmulatorResponse),
}

pub struct PendingResponses {
    responses: HashMap<u32, EmulatorResponse>,
    wakers: HashMap<u32, Waker>,
}

impl Default for PendingResponses {
    fn default() -> PendingResponses {
        PendingResponses {
            responses: HashMap::new(),
            wakers: HashMap::new(),
        }
    }
}

struct SenderClosed {}

pub(crate) struct EmulatorCommandExecution<T> {
    id: u32,
    pending_responses: Rc<RefCell<PendingResponses>>,
    response_type: PhantomData<T>,
}

impl<T> Future for EmulatorCommandExecution<T>
where
    T: TryFrom<EmulatorResponse>,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut pending_responses = self.pending_responses.borrow_mut();
        match pending_responses.responses.remove(&self.id) {
            Some(t) => Poll::Ready(match t.try_into() {
                Ok(t) => {
                    cx.waker().wake_by_ref();
                    t
                }
                Err(_) => panic!("Invalid response recieved for command {}", self.id),
            }),
            None => {
                pending_responses.wakers.insert(self.id, cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

struct EmulatorThread {
    state: EmulatorState,
    rx: mpsc::Receiver<(u32, EmulatorCommand)>,
    tx: glib::Sender<EmulatorThreadOutput>,
    breakpoints: Vec<UiBreakpoint>,
    exec_mode: ExecMode,
}

impl EmulatorThread {
    fn start() -> (
        thread::JoinHandle<()>,
        mpsc::Sender<(u32, EmulatorCommand)>,
        glib::Receiver<EmulatorThreadOutput>,
    ) {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = glib::MainContext::channel(glib::source::PRIORITY_DEFAULT);

        let thread = thread::spawn(move || {
            let emu_thread = EmulatorThread {
                state: EmulatorState::new(),
                rx: command_rx,
                tx: event_tx,
                breakpoints: Vec::new(),
                exec_mode: ExecMode::Unloaded,
            };
            emu_thread.run();
        });

        (thread, command_tx, event_rx)
    }

    fn handle_commands(&mut self) -> Result<(), SenderClosed> {
        let mut iter = self.rx.try_iter();
        while let Some((id, cmd)) = iter.next() {
            let resp: EmulatorResponse = match cmd {
                EmulatorCommand::LoadRom(path) => {
                    EmulatorResponse::LoadRom(self.state.load_rom(&path))
                }
                EmulatorCommand::QueryMemory(start_index, end_index) => {
                    EmulatorResponse::QueryMemory(self.state.query_memory(start_index, end_index))
                }
                EmulatorCommand::QueryRegisters => {
                    EmulatorResponse::QueryRegisters(self.state.query_registers())
                }
                EmulatorCommand::Step => EmulatorResponse::Step(self.state.step()),
                EmulatorCommand::SetMode(mode) => {
                    self.exec_mode = mode;
                    EmulatorResponse::SetMode(Ok(()))
                },
                EmulatorCommand::AddBreakpoint(bp) => {
                    self.breakpoints.push(bp);
                    EmulatorResponse::AddBreakpoint(Ok(()))
                },
            };
            self.tx
                .send(EmulatorThreadOutput::Response(id, resp))
                .map_err(|_| SenderClosed {})?;
        }
        Ok(())
    }

    fn step(breakpoints: &Vec<UiBreakpoint>, gb: &mut GameBoy, inital_mode: ExecMode) -> Result<ExecMode, StepError> {
        gb.step()?;
        for bp in breakpoints.iter() {
            if bp.breakpoint.should_break(gb) {
                return Ok(ExecMode::Paused)
            }
        }
        Ok(inital_mode)
    }

    fn run(mut self) {
        loop {
            if let Err(_) = self.handle_commands() {
                break;
            }
            if let Some(gb) = self.state.gameboy.as_mut() {
                let result = match self.exec_mode {
                    ExecMode::Paused | ExecMode::Unloaded => {
                        thread::sleep(Duration::from_micros(10000));
                        Ok(self.exec_mode)
                    }
                    ExecMode::Standard => {
                        thread::sleep(Duration::from_micros(1024 * 1024));
                        EmulatorThread::step(&self.breakpoints, gb, self.exec_mode)
                    }
                    ExecMode::Uncapped => EmulatorThread::step(&self.breakpoints, gb, self.exec_mode),
                };
                match result {
                    Err(e) => {
                        self.tx
                            .send(Error::Exec(e).into())
                            .expect("Emulator thread response channel closed");
                    },
                    Ok(mode) => {
                        self.exec_mode = mode
                    }
                }
            } else {
                thread::sleep(Duration::from_micros(10000))
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum ExecMode {
    Unloaded,
    Paused,
    Standard,
    Uncapped,
}

pub(crate) struct EmulatorState {
    pub gameboy: Option<GameBoy>,
    pub breakpoints: Vec<UiBreakpoint>,
}

impl EmulatorState {
    pub(crate) fn new() -> EmulatorState {
        EmulatorState {
            gameboy: None,
            breakpoints: vec![],
        }
    }

    pub(crate) fn step(&mut self) -> Result<(), Error> {
        if let Some(gb) = self.gameboy.as_mut() {
            gb.step().map_err(|e| Error::Exec(e))
        } else {
            Err(Error::NoRomLoaded)
        }
    }

    pub(crate) fn load_rom(&mut self, path: &Path) -> Result<(), LoadRomError> {
        let rom = std::fs::read(path)?;
        self.gameboy = Some(GameBoy::new(
            Cartridge::from_data(rom)?,
            GameBoyModel::GameBoy,
        ));
        println!("Loading ROM {}", path.to_string_lossy());
        Ok(())
    }

    fn query_registers(&mut self) -> Result<QueryRegistersResponse, Error> {
        if let Some(gb) = self.gameboy.as_ref() {
            Ok(QueryRegistersResponse {
                af: gb.read_register_u16(WordRegister::AF),
                bc: gb.read_register_u16(WordRegister::BC),
                de: gb.read_register_u16(WordRegister::DE),
                hl: gb.read_register_u16(WordRegister::HL),
                sp: gb.read_register_u16(WordRegister::SP),
                pc: gb.read_register_u16(WordRegister::PC),
            })
        } else {
            Err(Error::NoRomLoaded)
        }
    }

    fn query_memory(
        &mut self,
        start_addr: u16,
        end_addr: u16,
    ) -> Result<QueryMemoryResponse, Error> {
        let mut data: Vec<Option<u8>> = Vec::with_capacity((end_addr - start_addr) as usize + 1);
        if let Some(gb) = self.gameboy.as_ref() {
            for addr in start_addr..=end_addr {
                data.push(gb.read_memory_u8(addr).ok())
            }
            Ok(QueryMemoryResponse { start_addr, data })
        } else {
            Err(Error::NoRomLoaded)
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

    fn notify_listeners<T: Copy>(listeners: &mut HashMap<u32, glib::Sender<T>>, value: T) {
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
        AdapterEventListeners::notify_listeners(&mut self.mode_change, mode);
    }

    fn notify_step(&mut self) {
        AdapterEventListeners::notify_listeners(&mut self.step, ());
    }
}

pub(crate) struct EmulatorAdapter {
    tx: mpsc::Sender<(u32, EmulatorCommand)>,
    pending_responses: Rc<RefCell<PendingResponses>>,
    mode: RefCell<ExecMode>,
    event_listeners: Rc<RefCell<AdapterEventListeners>>,
    cached_registers: RefCell<QueryRegistersResponse>,
    next_id: AtomicU32,
}

impl EmulatorAdapter {
    pub(crate) fn new(ctx: &glib::MainContext) -> EmulatorAdapter {
        let (_thread_handle, tx, rx) = EmulatorThread::start();
        let adapter = EmulatorAdapter {
            tx,
            pending_responses: Rc::new(RefCell::new(PendingResponses::default())),
            mode: RefCell::new(ExecMode::Unloaded),
            event_listeners: Rc::new(RefCell::new(AdapterEventListeners::new())),
            cached_registers: RefCell::new(QueryRegistersResponse::default()),
            next_id: AtomicU32::new(0),
        };
        adapter.attach(rx, ctx);
        adapter
    }

    pub(crate) fn on_step(&self, tx: glib::Sender<()>) {
        self.event_listeners.borrow_mut().add_step(tx)
    }

    pub(crate) fn on_mode_change(&self, tx: glib::Sender<ExecMode>) {
        self.event_listeners.borrow_mut().add_mode_change(tx)
    }

    fn attach(&self, rx: glib::Receiver<EmulatorThreadOutput>, ctx: &glib::MainContext) {
        rx.attach(
            Some(ctx),
            clone!(@strong self.pending_responses as responses => move |output| {
                let mut pending_responses = responses.borrow_mut();
                match output {
                    EmulatorThreadOutput::Response(id, resp) => {
                        pending_responses.responses.insert(id, resp);
                        if let Some(waker) = pending_responses.wakers.remove(&id) {
                            waker.wake();
                        }
                    }
                    _ => {}
                }
                glib::Continue(true)
            }),
        );
    }

    fn apply_mode(&self, mode: ExecMode) {
        self.mode.replace(ExecMode::Paused);
        self.event_listeners.borrow_mut().notify_mode_change(mode);
    }

    pub(crate) async fn load_rom(&self, path: PathBuf) -> Result<(), LoadRomError> {
        let result: Result<(), LoadRomError> = self.send_command(EmulatorCommand::LoadRom(path)).await;

        if result.is_ok() {
            self.apply_mode(ExecMode::Paused);
        }

        self.event_listeners.borrow_mut().notify_step();

        result
    }

    pub(crate) async fn query_memory(
        &self,
        start_addr: u16,
        end_addr: u16,
    ) -> Result<QueryMemoryResponse, Error> {
        self.send_command(EmulatorCommand::QueryMemory(start_addr, end_addr))
            .await
    }

    pub(crate) async fn query_registers(&self) -> Result<QueryRegistersResponse, Error> {
        let result: Result<QueryRegistersResponse, Error> = self.send_command(EmulatorCommand::QueryRegisters).await;
        if let Ok(ref registers) = result {
            self.cached_registers.replace(registers.clone());
        }
        result
    }

    pub(crate) fn pc(&self) -> u16 {
        self.cached_registers.borrow().pc
    }

    pub(crate) async fn step(&self) -> Result<(), Error> {
        let result = self.send_command(EmulatorCommand::Step).await;
        self.event_listeners.borrow_mut().notify_step();
        result
    }

    pub(crate) async fn set_mode(&self, mode: ExecMode) -> Result<(), Error> {
        let result: Result<(), Error> = self.send_command(EmulatorCommand::SetMode(mode)).await;

        if result.is_ok() {
            self.apply_mode(mode);
        }

        result
    }

    pub(crate) async fn add_breakpoint(&self, breakpoint: UiBreakpoint) -> Result<(), ()> {
        self.send_command(EmulatorCommand::AddBreakpoint(breakpoint)).await
    }

    fn send_command<T>(&self, cmd: EmulatorCommand) -> EmulatorCommandExecution<T> {
        let next_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.tx
            .send((next_id, cmd))
            .expect("Could not send command to emulator");
        EmulatorCommandExecution {
            id: next_id,
            pending_responses: self.pending_responses.clone(),
            response_type: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

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
        let adapter = EmulatorAdapter::new(&context);
        let resp = context.block_on(adapter.load_rom(rom_path()));
        assert_eq!(resp, Ok(()));
    }
}
