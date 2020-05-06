use crate::builder_struct;
use crate::emulator::{commands::QueryMemoryResponse, remote::RemoteEmulator};

use glib::clone;
use gtk::prelude::*;
use olympia_engine::address::LiteralAddress;
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

    fn cell(&self, idx: usize) -> Option<&gtk::Label> {
        self.value_labels.get(idx)
    }

    fn set_offset(&self, offset: u16) {
        self.offset.replace(offset);
        self.addr.set_text(&format!("0x{:04X}", offset))
    }

    fn update(&self, offset: u16, pc: u16, result: &QueryMemoryResponse) {
        self.set_offset(offset);
        let data_offset = offset - result.start_addr;
        for i in 0..16 {
            let address_value_index = data_offset + i;
            let memory_value = result
                .data
                .get(address_value_index as usize)
                .and_then(|x| x.clone());
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

builder_struct!(
    pub struct MemoryViewerWidget {
        #[ogtk(id = "MemoryViewerAddressEntry")]
        address_entry: gtk::Entry,
        #[ogtk(id = "MemoryViewerPanel")]
        panel: gtk::Box,
        #[ogtk(id = "MemoryViewerPCButton")]
        pc_button: gtk::Button,
        #[ogtk(id = "MemoryViewerGoButton")]
        go_button: gtk::Button,
    }
);

pub struct MemoryViewer {
    context: glib::MainContext,
    emu: Rc<RemoteEmulator>,
    rows: Vec<MemoryViewerRow>,
    offset: RefCell<u16>,
    widget: MemoryViewerWidget,
}

impl MemoryViewer {
    pub(crate) fn from_widget(
        context: glib::MainContext,
        emu: Rc<RemoteEmulator>,
        widget: MemoryViewerWidget,
        num_visible_rows: u16,
    ) -> Rc<MemoryViewer> {
        let rows = (0..num_visible_rows)
            .map(|row| MemoryViewerRow::new(row * 0x10))
            .collect();
        let viewer = Rc::new(MemoryViewer {
            context,
            emu,
            rows: rows,
            offset: RefCell::new(0),
            widget,
        });
        viewer.connect_ui_events();
        viewer.connect_adapter_events();
        viewer
    }

    pub(crate) fn from_builder(
        builder: &gtk::Builder,
        context: glib::MainContext,
        emu: Rc<RemoteEmulator>,
        num_visible_rows: u16,
    ) -> Rc<MemoryViewer> {
        let widget = MemoryViewerWidget::from_builder(builder).unwrap();
        MemoryViewer::from_widget(context, emu, widget, num_visible_rows)
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn row(&self, idx: usize) -> Option<&MemoryViewerRow> {
        self.rows.get(idx)
    }

    fn address_range(&self) -> (u16, u16) {
        let start_addr = self.offset.borrow().clone();
        let end_addr = self
            .offset
            .borrow()
            .clone()
            .saturating_add(self.rows.len() as u16 * 0x10);
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

    fn render(&self, pc: u16, result: QueryMemoryResponse) {
        let offset = result.start_addr;
        self.offset.replace(offset);
        for (i, row) in self.rows.iter().enumerate() {
            let row_offset = offset + (i as u16 * 0x10);
            row.update(row_offset, pc, &result);
        }
    }

    async fn set_target_to_pc(self: Rc<Self>, address_entry: gtk::Entry) -> () {
        let result = self.emu.query_registers().await;
        if let Ok(registers) = result {
            let pc_value = registers.read_u16(WordRegister::PC);
            address_entry.set_text(&format!("{:04X}", pc_value));
        }
    }

    fn connect_ui_events(self: &Rc<Self>) {
        self.widget.address_entry.set_text("0000");

        let viewer_box = self.get_layout();
        viewer_box.connect_scroll_event(clone!(@strong self as mem_viewer => move |_, evt| {
            mem_viewer.clone().handle_scroll_evt(evt);
            Inhibit(true)
        }));
        viewer_box.add_events(gdk::EventMask::SCROLL_MASK);
        self.widget.panel.add(&viewer_box);

        self.widget.pc_button.connect_clicked(
            clone!(@weak self as mem_viewer, @strong self.context as ctx, @strong self.widget.address_entry as address_entry => move |_| {
                ctx.spawn_local(mem_viewer.clone().set_target_to_pc(address_entry.clone()));
            }),
        );

        self.widget.go_button.connect_clicked(
            clone!(@weak self as mem_viewer, @strong self.widget.address_entry as address_entry => move |_| {
                mem_viewer.clone().goto_address(&address_entry)
            }),
        );
        self.widget.address_entry.connect_activate(
            clone!(@weak self as mem_viewer => move |entry| {
                mem_viewer.clone().goto_address(&entry)
            }),
        );
    }

    fn handle_write(&self, addr: LiteralAddress, val: u8) {
        let addr_value = addr.0;
        let (start_addr, end_addr) = self.address_range();
        if start_addr <= addr_value && addr_value < end_addr {
            let address_of_row = addr_value & 0xFFF0;
            let cell_index = addr_value & 0xF;
            let row_index = (address_of_row - start_addr) / 0x10;
            println!("Setting row {} cell {} to {}", row_index, cell_index, val);
            self.rows
                .get(usize::from(row_index))
                .and_then(|row| row.cell(usize::from(cell_index)))
                .map(|cell| cell.set_text(&format!("{:02X}", val)));
        }
    }

    fn connect_adapter_events(self: &Rc<Self>) {
        let (tx, rx) = glib::MainContext::channel(glib::source::PRIORITY_DEFAULT);
        self.emu.on_step(tx);
        rx.attach(
            Some(&self.context),
            clone!(@weak self as viewer, @strong self.context as ctx => @default-return glib::Continue(false), move |_| {
                ctx.spawn_local(viewer.clone().refresh());
                glib::Continue(true)
            }),
        );

        let (tx, rx) = glib::MainContext::channel(glib::source::PRIORITY_DEFAULT);
        self.emu.on_memory_write(tx);
        rx.attach(
            Some(&self.context),
            clone!(@strong self as viewer => @default-return glib::Continue(false), move |mw| {
                viewer.handle_write(mw.address, mw.new_value);
                glib::Continue(true)
            }),
        );
    }

    async fn refresh(self: Rc<Self>) {
        let (start_addr, end_addr) = self.address_range();
        let query_result = self.emu.query_memory(start_addr, end_addr).await;
        match query_result {
            Ok(mem_response) => self.render(self.emu.pc(), mem_response),
            Err(_) => {}
        }
    }

    fn goto_address(self: Rc<Self>, address_entry: &gtk::Entry) {
        if let Some(text) = address_entry.get_text() {
            let text_string = text.as_str();
            let parsed = u16::from_str_radix(text_string, 16);
            if let Ok(val) = parsed {
                let ctx = self.context.clone();
                self.offset.replace(self.resolve(val));
                ctx.spawn_local(self.refresh())
            }
        }
    }

    fn handle_scroll_evt(self: Rc<Self>, evt: &gdk::EventScroll) {
        let ctx = self.context.clone();
        match evt.get_direction() {
            gdk::ScrollDirection::Down => self.scroll_down(1),
            gdk::ScrollDirection::Up => self.scroll_up(1),
            _ => (),
        };
        ctx.spawn_local(self.refresh());
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::test_utils;

    fn setup_widget(
        num_visible_rows: u16,
    ) -> (glib::MainContext, Rc<RemoteEmulator>, Rc<MemoryViewer>) {
        test_utils::setup_gtk().unwrap();
        let context = test_utils::setup_context();
        let emu = Rc::new(test_utils::get_unloaded_remote_emu(context.clone()));
        let builder = gtk::Builder::new_from_string(include_str!("../../res/debugger.ui"));
        let component =
            MemoryViewer::from_builder(&builder, context.clone(), emu.clone(), num_visible_rows);
        (context, emu, component)
    }

    #[test]
    fn gtk_test_initial_load() {
        let (_, _, component) = setup_widget(16);

        for i in 0..16 {
            let row = component.row(i).expect(&format!("No row found at {}", i));
            for j in 0..16 {
                let col = row
                    .cell(j)
                    .expect(&format!("No cell found at row {} column {}", i, j));
                assert_eq!(col.get_text().unwrap(), "--");
            }
        }
    }

    #[test]
    fn gtk_test_rom_loaded() {
        let (context, emu, component) = setup_widget(16);

        let task = async {
            emu.load_rom(test_utils::fizzbuzz_path()).await.unwrap();
            emu.query_memory(0x00, 0xFF).await
        };
        let memory_data = test_utils::wait_for_task(&context, task).unwrap();

        test_utils::next_tick(&context, &emu);

        for i in 0..16 {
            let row = component.row(i).expect(&format!("No row found at {}", i));
            for j in 0..16 {
                let col = row
                    .cell(j)
                    .expect(&format!("No cell found at row {} column {}", i, j));
                let actual_value = memory_data.data.get((i * 0x10) + j).unwrap().unwrap();
                assert_eq!(
                    col.get_text().unwrap().as_str(),
                    format!("{:02X}", actual_value)
                );
            }
        }
    }

    #[test]
    fn gtk_handle_write() {
        let (context, emu, component) = setup_widget(2);

        let task = async {
            emu.load_rom(test_utils::fizzbuzz_path()).await.unwrap();
        };

        test_utils::wait_for_task(&context, task);

        component.widget.address_entry.get_buffer().set_text("8000");
        component.widget.go_button.clicked();
        test_utils::next_tick(&context, &emu);

        for x in 0..0x30 {
            let addr = 0x7FF0 + u16::from(x);
            component.handle_write(addr.into(), x);
        }

        for row_idx in 0..2 {
            for cell_idx in 0..0xF {
                let actual_value = component
                    .row(row_idx)
                    .and_then(|row| row.cell(cell_idx))
                    .and_then(|cell| cell.get_text())
                    .map(|s| s.as_str().to_string());
                let expected_value = format!("{:02X}", ((1 + row_idx) * 0x10) + cell_idx);

                assert_eq!(actual_value, Some(expected_value));
            }
        }
    }

    #[test]
    fn gtk_test_goto_address() {
        let (context, emu, component) = setup_widget(16);

        let task = async {
            emu.load_rom(test_utils::fizzbuzz_path()).await.unwrap();
        };

        test_utils::wait_for_task(&context, task);

        component.widget.address_entry.get_buffer().set_text("8000");
        component.widget.go_button.clicked();
        test_utils::next_tick(&context, &emu);

        for i in 0..16 {
            let row = component.row(i).expect(&format!("No row found at {}", i));
            for j in 0..16 {
                let col = row
                    .cell(j)
                    .expect(&format!("No cell found at row {} column {}", i, j));
                assert_eq!(col.get_text().unwrap().as_str(), "00");
            }
        }
    }
}
