use gtk::{glib, prelude::StaticType};

use crate::utils::EmulatorHandle;

pub const EMU_PROPERTY: &'static str = "emu";

pub trait EmulatorWidget: glib::ObjectExt {
    fn attach_emu(&self, emulator: EmulatorHandle) {
        self.set_property(EMU_PROPERTY, emulator).unwrap();
    }

    fn emu_handle(&self) -> EmulatorHandle {
        self.property(EMU_PROPERTY)
            .expect("Invalid emulator property name")
            .get()
            .expect("No emulator adapter attached")
    }
}

pub fn emu_param_spec() -> glib::ParamSpec {
    glib::ParamSpec::new_boxed(
        EMU_PROPERTY,
        EMU_PROPERTY,
        EMU_PROPERTY,
        EmulatorHandle::static_type(),
        glib::ParamFlags::READWRITE,
    )
}

#[macro_export]
macro_rules! subclass_widget {
    ($internal:ty, $parent:ty, $handle:ty) => {
        #[glib::object_subclass]
        impl ObjectSubclass for $internal {
            const NAME: &'static str = concat!("Olympia", stringify!($handle));
            type ParentType = $parent;
            type Type = $handle;

            fn class_init(klass: &mut Self::Class) {
                Self::bind_template(klass);
            }

            fn instance_init(obj: &InitializingObject<Self>) {
                obj.init_template();
            }
        }
    };
}
