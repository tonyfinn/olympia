use gtk::glib;
use gtk::glib::clone;

use olympia_engine::{
    events::{propagate_events, EventEmitter, ModeChangeEvent},
    gameboy::{GameBoy, GameBoyModel, StepError, CYCLE_FREQ},
    monitor::{BreakpointState, DebugMonitor},
    registers::WordRegister,
    remote,
    remote::{
        CommandId, EmulatorCommand, EmulatorResponse, ExecMode, ExecTime, LoadRomError,
        QueryMemoryResponse, QueryRegistersResponse, RemoteEmulatorOutput,
        ToggleBreakpointResponse,
    },
    rom::Cartridge,
};

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use std::{cell::RefCell, rc::Rc};

struct SenderClosed {}

pub(crate) struct EmulatorState {
    pub gameboy: Option<GameBoy>,
    pub monitor: Rc<RefCell<DebugMonitor>>,
}

impl EmulatorState {
    pub(crate) fn new() -> EmulatorState {
        EmulatorState {
            gameboy: None,
            monitor: Rc::new(RefCell::new(DebugMonitor::new())),
        }
    }

    pub(crate) fn step(&mut self) -> remote::Result<()> {
        if let Some(gb) = self.gameboy.as_mut() {
            gb.step().map_err(remote::Error::Exec)
        } else {
            Err(remote::Error::NoRomLoaded)
        }
    }

    pub(crate) fn load_rom(&mut self, data: Vec<u8>) -> Result<(), LoadRomError> {
        let gb = GameBoy::new(Cartridge::from_data(data)?, GameBoyModel::GameBoy);
        gb.events.on(Box::new(
            clone!(@weak self.monitor as monitor => move |evt| {
                monitor.borrow_mut().handle_event(evt);
            }),
        ));
        self.gameboy = Some(gb);
        Ok(())
    }

    fn exec_time(&mut self) -> remote::Result<ExecTime> {
        if let Some(gb) = self.gameboy.as_ref() {
            Ok(gb.time_elapsed().into())
        } else {
            Err(remote::Error::NoRomLoaded)
        }
    }

    fn query_registers(&mut self) -> remote::Result<QueryRegistersResponse> {
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
            Err(remote::Error::NoRomLoaded)
        }
    }

    fn query_memory(
        &mut self,
        start_addr: u16,
        end_addr: u16,
    ) -> remote::Result<QueryMemoryResponse> {
        let mut data: Vec<Option<u8>> = Vec::with_capacity((end_addr - start_addr) as usize + 1);
        if let Some(gb) = self.gameboy.as_ref() {
            for addr in start_addr..=end_addr {
                data.push(gb.get_memory_u8(addr).ok())
            }
            Ok(QueryMemoryResponse { start_addr, data })
        } else {
            Err(remote::Error::NoRomLoaded)
        }
    }
}

pub(super) struct EmulatorThread {
    state: EmulatorState,
    rx: mpsc::Receiver<(CommandId, EmulatorCommand)>,
    tx: Rc<glib::Sender<RemoteEmulatorOutput>>,
    events: Rc<EventEmitter<remote::Event>>,
    exec_mode: ExecMode,
}

impl EmulatorThread {
    fn new(
        command_rx: mpsc::Receiver<(CommandId, EmulatorCommand)>,
        event_tx: glib::Sender<RemoteEmulatorOutput>,
    ) -> EmulatorThread {
        let state = EmulatorState::new();
        EmulatorThread {
            state,
            rx: command_rx,
            tx: Rc::new(event_tx),
            events: Rc::new(EventEmitter::new()),
            exec_mode: ExecMode::Unloaded,
        }
    }

    pub fn start() -> (
        thread::JoinHandle<()>,
        mpsc::Sender<(CommandId, EmulatorCommand)>,
        glib::Receiver<RemoteEmulatorOutput>,
    ) {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = glib::MainContext::channel(glib::source::PRIORITY_DEFAULT);

        let thread = thread::spawn(move || {
            let emu_thread = EmulatorThread::new(command_rx, event_tx);
            emu_thread
                .events
                .on(Box::new(clone!(@weak emu_thread.tx as tx => move |evt| {
                    if let Err(e) = tx.send(RemoteEmulatorOutput::Event(evt.clone())) {
                        log::error!(target: "emu_thread", "Cannot report emulator output event: {:?}. Event {:?}", e, evt);
                    }
                })));
            emu_thread.run();
        });

        (thread, command_tx, event_rx)
    }

    fn load_rom(
        state: &mut EmulatorState,
        events: Rc<EventEmitter<remote::Event>>,
        data: Vec<u8>,
    ) -> Result<(), LoadRomError> {
        state.load_rom(data)?;
        if let Some(ref gb) = state.gameboy {
            propagate_events(&gb.events, events);
        } else {
            panic!("Gameboy not present after ROM loaded");
        }
        Ok(())
    }

    fn handle_commands(&mut self) -> Result<(), SenderClosed> {
        for (id, cmd) in self.rx.try_iter() {
            let resp: EmulatorResponse = match cmd {
                EmulatorCommand::LoadRom(data) => {
                    let resp = EmulatorResponse::LoadRom(EmulatorThread::load_rom(
                        &mut self.state,
                        self.events.clone(),
                        data,
                    ));
                    self.exec_mode = ExecMode::Paused;
                    self.tx
                        .send(RemoteEmulatorOutput::Event(
                            ModeChangeEvent::new(ExecMode::Unloaded, ExecMode::Paused).into(),
                        ))
                        .map_err(|_| SenderClosed {})?;
                    resp
                }
                EmulatorCommand::QueryMemory(start_index, end_index) => {
                    EmulatorResponse::QueryMemory(self.state.query_memory(start_index, end_index))
                }
                EmulatorCommand::QueryRegisters => {
                    EmulatorResponse::QueryRegisters(self.state.query_registers())
                }
                EmulatorCommand::Step => EmulatorResponse::Step(self.state.step()),
                EmulatorCommand::QueryExecTime => {
                    EmulatorResponse::QueryExecTime(self.state.exec_time())
                }
                EmulatorCommand::SetMode(mode) => {
                    if mode == ExecMode::Standard || mode == ExecMode::Uncapped {
                        self.state.monitor.borrow_mut().resume();
                    }
                    let old_mode = self.exec_mode.clone();
                    self.exec_mode = mode;
                    self.tx
                        .send(RemoteEmulatorOutput::Event(
                            ModeChangeEvent::new(old_mode, self.exec_mode.clone()).into(),
                        ))
                        .map_err(|_| SenderClosed {})?;
                    EmulatorResponse::SetMode(Ok(self.exec_mode.clone()))
                }
                EmulatorCommand::AddBreakpoint(bp) => {
                    let resp = self.state.monitor.borrow_mut().add_breakpoint(bp);
                    EmulatorResponse::AddBreakpoint(Ok(resp.into()))
                }
                EmulatorCommand::RemoveBreakpoint(id) => {
                    let resp = self.state.monitor.borrow_mut().remove_breakpoint(id);
                    if resp.is_none() {
                        log::info!("Tried to remove invalid breakpoint {:?}", id);
                    }
                    EmulatorResponse::RemoveBreakpoint(Ok(id.into()))
                }
                EmulatorCommand::SetBreakpointActive(id, state) => {
                    let resp = self
                        .state
                        .monitor
                        .borrow_mut()
                        .set_breakpoint_state(id, state);
                    if let Some(state) = resp {
                        EmulatorResponse::ToggleBreakpoint(Ok(ToggleBreakpointResponse::new(
                            id, state,
                        )))
                    } else {
                        EmulatorResponse::ToggleBreakpoint(Err(()))
                    }
                }
            };
            self.tx
                .send(RemoteEmulatorOutput::Response(id, resp))
                .map_err(|_| SenderClosed {})?;
        }
        Ok(())
    }

    fn step(
        gb: &mut GameBoy,
        monitor: &RefCell<DebugMonitor>,
        inital_mode: ExecMode,
    ) -> Result<ExecMode, StepError> {
        gb.step()?;
        if let BreakpointState::HitBreakpoint(bp) = monitor.borrow().state() {
            log::info!(target: "emu_thread", "Hit breakpoint: {:?}", bp);
            return Ok(ExecMode::HitBreakpoint(bp));
        }
        Ok(inital_mode)
    }

    fn run(mut self) {
        loop {
            if self.handle_commands().is_err() {
                break;
            }
            if let Some(gb) = self.state.gameboy.as_mut() {
                let start_time = Instant::now();
                let result = match &self.exec_mode {
                    ExecMode::Paused | ExecMode::Unloaded | ExecMode::HitBreakpoint(_) => {
                        thread::sleep(Duration::from_micros(10000));
                        Ok(self.exec_mode.clone())
                    }
                    ExecMode::Standard => {
                        thread::sleep(Duration::from_secs_f64(1.0 / (f64::from(CYCLE_FREQ))));
                        let step_result =
                            EmulatorThread::step(gb, &self.state.monitor, self.exec_mode.clone());
                        gb.add_exec_time(start_time.elapsed().as_secs_f64());
                        step_result
                    }
                    ExecMode::Uncapped => {
                        let step_result =
                            EmulatorThread::step(gb, &self.state.monitor, self.exec_mode.clone());
                        gb.add_exec_time(start_time.elapsed().as_secs_f64());
                        step_result
                    }
                };
                match result {
                    Err(e) => {
                        self.tx
                            .send(remote::Error::Exec(e).into())
                            .expect("Emulator thread response channel closed");
                    }
                    Ok(mode) => {
                        if mode != self.exec_mode {
                            let change_event =
                                ModeChangeEvent::new(self.exec_mode.clone(), mode.clone());
                            self.exec_mode = mode;
                            self.tx
                                .send(remote::RemoteEmulatorOutput::Event(change_event.into()))
                                .expect("Emulator thread response channel closed");
                        }
                    }
                }
            } else {
                thread::sleep(Duration::from_micros(10000))
            }
        }
    }
}
