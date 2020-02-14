use crate::emulator;
use glib::clone;
use gtk::prelude::*;
use olympia_engine::registers::WordRegister;
use std::cell::RefCell;
use std::rc::Rc;

struct MemoryViewerRow {
    addr: gtk::Label,
    layout: gtk::Box,
    offset: RefCell<u16>,
    value_labels: Vec<gtk::Label>,
}

impl MemoryViewerRow {
    fn new(offset: u16) -> MemoryViewerRow {
        let addr = gtk::Label::new(Some(&format!("0x{:04X}", offset)));
        let value_labels: Vec<gtk::Label> = (0..16).map(|_| gtk::Label::new(Some("--"))).collect();
        let layout = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        layout.add(&addr);
        for val in value_labels.iter() {
            layout.add(val);
        }
        for label in value_labels.iter().chain(std::iter::once(&addr)) {
            let font_attr = pango::Attribute::new_family("monospace").unwrap();
            let attr_list = pango::AttrList::new();
            attr_list.insert(font_attr);
            label.set_attributes(Some(&attr_list));
        }
        MemoryViewerRow {
            addr,
            layout,
            value_labels,
            offset: RefCell::new(offset),
        }
    }

    fn set_offset(&self, offset: u16) {
        self.offset.replace(offset);
        self.addr.set_text(&format!("0x{:04X}", offset))
    }

    fn update(&self, offset: u16, pc: u16, result: &emulator::QueryMemoryResponse) {
        self.set_offset(offset);
        let data_offset = offset - result.start_addr;
        for i in 0..16 {
            let address_value_index = data_offset + i;
            let memory_value = result.data.get(address_value_index as usize).and_then(|x| x.clone());
            let is_pc = offset + i == pc;
            let formatted = match memory_value {
                Some(val) => format!("{:02X}", val),
                None => "--".into(),
            };
            let label = &self.value_labels[i as usize];
            label.set_text(&formatted);
            label.set_property_has_focus(is_pc);
        }
    }
}

pub struct MemoryViewer {
    offset: RefCell<u16>,
    rows: Vec<MemoryViewerRow>,
    adapter: Rc<emulator::EmulatorAdapter>,
}

impl MemoryViewer {
    pub(crate) fn from_builder(builder: &gtk::Builder, num_visible_rows: u16, adapter: Rc<emulator::EmulatorAdapter>) -> Rc<MemoryViewer> {
        let rows = (0..num_visible_rows)
            .map(|row| MemoryViewerRow::new(row * 0x10))
            .collect();
        let viewer = Rc::new(MemoryViewer {
            offset: RefCell::new(0),
            rows: rows,
            adapter,
        });
        viewer.connect_ui_events(builder);
        viewer.connect_adapter_events();
        viewer
    }

    fn address_range(&self) -> (u16, u16) {
        let start_addr = self.offset.borrow().clone();
        let end_addr = self.offset.borrow().clone().saturating_add(self.rows.len() as u16 * 0x10);
        (start_addr, end_addr)
    }

    fn resolve(&self, addr: u16) -> u16 {
        let max_offset = u16::max_value() - ((self.rows.len() as u16) * 0x10) + 1;
        let offset = if addr > max_offset { max_offset } else { addr };
        offset & 0xFFF0
    }

    fn scroll_up(&self, scroll: u16) {
        let new_row_offset = self.offset.borrow().saturating_sub(scroll * 0x10);
        let resolved_offset = self.resolve(new_row_offset);
        self.offset.replace(resolved_offset);
    }

    fn scroll_down(&self, scroll: u16) {
        let new_row_offset = self.offset.borrow().saturating_add(scroll * 0x10);
        let resolved_offset = self.resolve(new_row_offset);
        self.offset.replace(resolved_offset);
    }

    pub(crate) fn get_layout(&self) -> gtk::EventBox {
        let event_catcher = gtk::EventBox::new();
        let layout = gtk::Box::new(gtk::Orientation::Vertical, 5);
        layout.set_margin_start(20);
        layout.set_margin_end(20);
        for row in self.rows.iter() {
            layout.add(&row.layout);
        }
        event_catcher.add(&layout);
        event_catcher
    }

    fn render(&self, pc: u16, result: emulator::QueryMemoryResponse) {
        let offset = result.start_addr;
        self.offset.replace(offset);
        for (i, row) in self.rows.iter().enumerate() {
            let row_offset = offset + (i as u16 * 0x10);
            row.update(row_offset, pc, &result);
        }
    }

    async fn set_target_to_pc(self: Rc<Self>, address_entry: gtk::Entry) -> () {
        let result = self.adapter.query_registers().await;
        if let Ok(registers) = result {
            let pc_value = registers.read_u16(WordRegister::PC);
            address_entry.set_text(&format!("{:04X}", pc_value));
        }
    }
    
    fn connect_ui_events(self: &Rc<Self>, builder: &gtk::Builder) {
        let address_entry: gtk::Entry = builder.get_object("MemoryViewerAddressEntry").unwrap();
        address_entry.set_text("0000");

        let viewer_panel: gtk::Box = builder.get_object("MemoryViewerPanel").unwrap();
        let viewer_box = self.get_layout();
        viewer_box.connect_scroll_event(clone!(@strong self as mem_viewer => move |_, evt| {
            mem_viewer.clone().handle_scroll_evt(evt);
            Inhibit(true)
        }));
        viewer_box.add_events(gdk::EventMask::SCROLL_MASK);
        viewer_panel.add(&viewer_box);

        let pc_button: gtk::Button = builder.get_object("MemoryViewerPCButton").unwrap();
        pc_button.connect_clicked(
            clone!(@strong self as mem_viewer, @strong address_entry => move |_| {
                let ctx = glib::MainContext::default();
                ctx.spawn_local(mem_viewer.clone().set_target_to_pc(address_entry.clone()));
            }),
        );
        let go_button: gtk::Button = builder.get_object("MemoryViewerGoButton").unwrap();
        go_button.connect_clicked(
            clone!(@strong self as mem_viewer, @strong address_entry => move |_| {
                mem_viewer.clone().goto_address(&address_entry)
            }),
        );
        address_entry.connect_activate(clone!(@strong self as mem_viewer => move |entry| {
            mem_viewer.clone().goto_address(&entry)
        }));
    }

    fn connect_adapter_events(self: &Rc<Self>) {
        let (tx, rx) = glib::MainContext::channel(glib::source::PRIORITY_DEFAULT);
        self.adapter.on_step(tx);
        let ctx = glib::MainContext::default();
        rx.attach(Some(&ctx), clone!(@strong self as viewer, @strong ctx => move |_| {
            ctx.spawn_local(viewer.clone().refresh());
            glib::Continue(true)
        }));
    }

    async fn refresh(self: Rc<Self>) {
        let (start_addr, end_addr) = self.address_range();
        let query_result = self.adapter.query_memory(start_addr, end_addr).await;
        match query_result {
            Ok(mem_response) => {
                self.render(self.adapter.pc(), mem_response)
            },
            Err(_) => {}
        }
    }

    fn goto_address(self: Rc<Self>, address_entry: &gtk::Entry) {
        if let Some(text) = address_entry.get_text() {
            let parsed = u16::from_str_radix(text.as_str(), 16);
            if let Ok(val) = parsed {
                let ctx = glib::MainContext::default();
                self.offset.replace(self.resolve(val));
                ctx.spawn_local(self.refresh())
            }
        }
    }

    fn handle_scroll_evt(self: Rc<Self>, evt: &gdk::EventScroll) {
        let ctx = glib::MainContext::default();
        match evt.get_direction() {
            gdk::ScrollDirection::Down => self.scroll_down(1),
            gdk::ScrollDirection::Up => self.scroll_up(1),
            _ => ()
        };
        ctx.spawn_local(self.refresh());
    }
}
