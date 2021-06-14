use glib::clone;

use olympia_engine::{
    events::{propagate_events, EventEmitter, ModeChangeEvent},
    gameboy::{GameBoy, GameBoyModel, StepError, CYCLE_FREQ},
    registers::WordRegister,
    remote,
    remote::{
        CommandId, EmulatorCommand, EmulatorResponse, ExecMode, ExecTime, LoadRomError,
        QueryMemoryResponse, QueryRegistersResponse, RemoteEmulatorOutput, UiBreakpoint,
    },
    rom::Cartridge,
};

use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

struct SenderClosed {}

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

    pub(crate) fn step(&mut self) -> remote::Result<()> {
        if let Some(gb) = self.gameboy.as_mut() {
            gb.step().map_err(|e| remote::Error::Exec(e))
        } else {
            Err(remote::Error::NoRomLoaded)
        }
    }

    pub(crate) fn load_rom(&mut self, data: Vec<u8>) -> Result<(), LoadRomError> {
        self.gameboy = Some(GameBoy::new(
            Cartridge::from_data(data)?,
            GameBoyModel::GameBoy,
        ));
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
                data.push(gb.read_memory_u8(addr).ok())
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
    breakpoints: Vec<UiBreakpoint>,
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
            breakpoints: Vec::new(),
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
                        eprintln!("Cannot report emulator output event: {:?}", e);
                    }
                })));
            emu_thread.run();
        });

        (thread, command_tx, event_rx)
    }

    fn load_rom(
        state: &mut EmulatorState,
        data: Vec<u8>,
        events: Rc<EventEmitter<remote::Event>>,
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
        let mut iter = self.rx.try_iter();
        while let Some((id, cmd)) = iter.next() {
            let resp: EmulatorResponse = match cmd {
                EmulatorCommand::LoadRom(data) => EmulatorResponse::LoadRom(
                    EmulatorThread::load_rom(&mut self.state, data, self.events.clone()),
                ),
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
                    self.exec_mode = mode;
                    EmulatorResponse::SetMode(Ok(self.exec_mode.clone()))
                }
                EmulatorCommand::AddBreakpoint(bp) => {
                    self.breakpoints.push(bp);
                    EmulatorResponse::AddBreakpoint(Ok(()))
                }
            };
            self.tx
                .send(RemoteEmulatorOutput::Response(id, resp))
                .map_err(|_| SenderClosed {})?;
        }
        Ok(())
    }

    fn step(
        breakpoints: &Vec<UiBreakpoint>,
        gb: &mut GameBoy,
        inital_mode: ExecMode,
    ) -> Result<ExecMode, StepError> {
        gb.step()?;
        for bp in breakpoints.iter() {
            if bp.breakpoint.should_break(gb) {
                log::info!(
                    "Hit breakpoint: {} {}",
                    bp.breakpoint.monitor,
                    bp.breakpoint.condition
                );
                return Ok(ExecMode::HitBreakpoint(bp.clone()));
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
                let start_time = Instant::now();
                let result = match &self.exec_mode {
                    ExecMode::Paused | ExecMode::Unloaded | ExecMode::HitBreakpoint(_) => {
                        thread::sleep(Duration::from_micros(10000));
                        Ok(self.exec_mode.clone())
                    }
                    ExecMode::Standard => {
                        thread::sleep(Duration::from_secs_f64(1.0 / (f64::from(CYCLE_FREQ))));
                        let step_result =
                            EmulatorThread::step(&self.breakpoints, gb, self.exec_mode.clone());
                        gb.add_exec_time(start_time.elapsed().as_secs_f64());
                        step_result
                    }
                    ExecMode::Uncapped => {
                        let step_result =
                            EmulatorThread::step(&self.breakpoints, gb, self.exec_mode.clone());
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
