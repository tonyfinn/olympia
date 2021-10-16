use crate::emulator::emu_thread::EmulatorThread;

use gtk::glib;

use olympia_engine::{
    events::{EventHandlerId, Repeat},
    remote::{
        CommandId, EmulatorCommand, Event as RemoteEvent, EventSendError, RemoteEmulator,
        RemoteEmulatorChannel, RemoteEmulatorOutput, RemoteEventListeners, Sender,
    },
};

use std::{
    any::TypeId,
    collections::HashMap,
    convert::{TryFrom, TryInto},
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
};

pub(crate) struct GlibEmulatorChannel {
    tx: mpsc::Sender<(CommandId, EmulatorCommand)>,
    rx: Option<glib::Receiver<RemoteEmulatorOutput>>,
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

pub struct WrappedGlibSender<R>(glib::Sender<R>);

impl<T, R> Sender<T> for WrappedGlibSender<R>
where
    R: TryFrom<T>,
{
    fn send(&self, event: T) -> Result<(), EventSendError> {
        match event.try_into() {
            Ok(evt) => self
                .0
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

    fn handle_output(&mut self, f: Box<dyn Fn(RemoteEmulatorOutput) -> Repeat>) {
        self.rx
            .take()
            .expect("Attempted to register two output sources")
            .attach(Some(&self.ctx), move |output| glib::Continue(f(output).0));
    }
}

pub(crate) struct GlibAdapterEventListeners {
    listeners: HashMap<TypeId, HashMap<EventHandlerId, Box<dyn Sender<RemoteEvent>>>>,
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

impl RemoteEventListeners for GlibAdapterEventListeners {
    fn on(
        &mut self,
        event_type_id: TypeId,
        f: Box<dyn Fn(RemoteEvent) -> Repeat + 'static>,
    ) -> EventHandlerId {
        let event_handler_id = EventHandlerId(self.next_listener_id);
        let (tx, rx) = glib::MainContext::channel::<RemoteEvent>(glib::PRIORITY_DEFAULT);
        let wrapped = Box::new(WrappedGlibSender(tx));
        let map = self
            .listeners
            .entry(event_type_id)
            .or_insert_with(HashMap::new);
        map.insert(event_handler_id, wrapped);
        self.next_listener_id += 1;

        rx.attach(Some(&self.context), move |evt| glib::Continue(f(evt).0));

        event_handler_id
    }

    fn emit(&mut self, evt: RemoteEvent) {
        let event_type_id = evt.event_type_id();
        if let Some(listeners) = self.listeners.get_mut(&event_type_id) {
            let mut listener_ids_to_remove = Vec::new();
            for (id, listener) in listeners.iter_mut() {
                let send_result = listener.send(evt.clone());
                if send_result.is_err() {
                    listener_ids_to_remove.push(*id);
                    log::warn!(target: "emu_thread", "Removing listener {:?} due to closed channel", id);
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
    let glib_listeners = GlibAdapterEventListeners::new(context);
    Rc::new(RemoteEmulator::new(
        Box::new(glib_listeners),
        Box::new(channel),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils;
    use olympia_engine::{
        events::{ManualStepEvent, ModeChangeEvent, RomLoadedEvent},
        monitor::{Breakpoint, BreakpointCondition, Comparison},
        registers::WordRegister,
        remote,
        remote::{ExecMode, LoadRomError, QueryMemoryResponse, QueryRegistersResponse},
    };
    use std::{cell::RefCell, rc::Rc, time::Duration};

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
    fn test_load_rom() {
        test_utils::with_context(|context| {
            let emu = test_utils::get_unloaded_remote_emu(context.clone());
            let (f, events) = track_event();
            emu.on::<RomLoadedEvent, _>(f);
            let task = async { emu.load_rom(test_utils::fizzbuzz_rom()).await };
            let resp = test_utils::wait_for_task(context, task);
            assert_eq!(resp, Ok(()));
            assert_eq!(events.borrow().clone(), vec![RomLoadedEvent]);
        })
    }

    #[test]
    fn test_load_rom_error() {
        test_utils::with_context(|context| {
            let emu = test_utils::get_unloaded_remote_emu(context.clone());
            let resp = context.block_on(emu.load_rom(vec![0x00]));
            assert!(matches!(resp, Err(LoadRomError::InvalidRom(_))));
        });
    }

    #[test]
    fn test_load_starts_paused() {
        test_utils::with_context(|context| {
            let emu = test_utils::get_unloaded_remote_emu(context.clone());
            let (f, events) = track_event();
            emu.on::<ModeChangeEvent, _>(f);
            let task = async {
                emu.load_rom(test_utils::fizzbuzz_rom()).await.unwrap();
            };
            test_utils::wait_for_task(context, task);
            assert_eq!(
                events.borrow().clone(),
                vec![ModeChangeEvent::new(ExecMode::Unloaded, ExecMode::Paused)]
            );
        });
    }

    #[test]
    fn test_step() {
        test_utils::with_context(|context| {
            let emu = test_utils::get_unloaded_remote_emu(context.clone());
            let (f, events) = track_event();
            emu.on::<ManualStepEvent, _>(f);
            let task = async {
                emu.load_rom(test_utils::fizzbuzz_rom()).await.unwrap();
                emu.step().await
            };
            let step_result = test_utils::wait_for_task(context, task);
            assert_eq!(events.borrow().clone(), vec![ManualStepEvent]);
            assert_eq!(step_result, Ok(()))
        });
    }

    #[test]
    fn test_step_unloaded() {
        test_utils::with_context(|context| {
            let emu = test_utils::get_unloaded_remote_emu(context.clone());
            let task = async { emu.step().await };
            let step_result = test_utils::wait_for_task(context, task);
            assert_eq!(step_result, Err(remote::Error::NoRomLoaded))
        });
    }

    #[test]
    fn test_query_memory() {
        test_utils::with_context(|context| {
            let emu = test_utils::get_loaded_remote_emu(context.clone());
            let task = async {
                emu.step().await.unwrap();
                emu.query_memory(0x00, 0x04).await
            };
            let memory_result = test_utils::wait_for_task(context, task);
            let expected_data = vec![201, 0, 0, 0, 0].into_iter().map(Some).collect();
            assert_eq!(
                memory_result,
                Ok(QueryMemoryResponse {
                    start_addr: 0x00,
                    data: expected_data
                })
            )
        });
    }

    #[test]
    fn test_query_memory_unloaded() {
        test_utils::with_context(|context| {
            let emu = test_utils::get_unloaded_remote_emu(context.clone());
            let task = async { emu.query_memory(0x00, 0x04).await };
            let memory_result = test_utils::wait_for_task(context, task);
            assert_eq!(memory_result, Err(remote::Error::NoRomLoaded))
        });
    }

    #[test]
    fn test_query_register() {
        test_utils::with_context(|context| {
            let emu = test_utils::get_loaded_remote_emu(context.clone());
            let task = async {
                emu.step().await.unwrap();
                emu.query_registers().await
            };
            let register_result = test_utils::wait_for_task(context, task);
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
        });
    }

    #[test]
    fn test_query_register_unloaded() {
        test_utils::with_context(|context| {
            let emu = test_utils::get_unloaded_remote_emu(context.clone());
            let task = async { emu.query_registers().await };
            let register_result = test_utils::wait_for_task(context, task);
            assert_eq!(register_result, Err(remote::Error::NoRomLoaded))
        });
    }

    #[test]
    fn test_run_to_breakpoint() {
        test_utils::with_unloaded_emu(|context, emu| {
            let (f, events) = track_event();
            emu.on::<ModeChangeEvent, _>(f);
            let bp = Breakpoint::new(
                WordRegister::PC.into(),
                BreakpointCondition::Test(Comparison::Equal, 0x150),
            );
            let task = async {
                emu.load_rom(test_utils::fizzbuzz_rom()).await.unwrap();
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
            assert!(emulation_time.duration() >= actual_gb_time);
            assert_eq!(
                events.borrow().clone(),
                vec![
                    ModeChangeEvent::new(ExecMode::Unloaded, ExecMode::Paused),
                    ModeChangeEvent::new(ExecMode::Paused, ExecMode::Standard),
                    ModeChangeEvent::new(ExecMode::Standard, ExecMode::HitBreakpoint(bp)),
                ]
            );
        });
    }

    #[test]
    fn test_ff_to_breakpoint() {
        test_utils::with_unloaded_emu(|context, emu| {
            let (f, events) = track_event();
            emu.on::<ModeChangeEvent, _>(f);
            let bp = Breakpoint::new(
                WordRegister::PC.into(),
                BreakpointCondition::Test(Comparison::Equal, 0x150),
            );
            let task = async {
                emu.load_rom(test_utils::fizzbuzz_rom()).await.unwrap();
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
        });
    }
}
