use derive_more::Display;
#[derive(Debug, Display)]
#[display(fmt = "Missing element for {} in builder", "_0")]
pub struct MissingUIElement(pub(crate) String);

#[macro_export]
macro_rules! builder_struct {
    (
        $vis:vis struct $struct_name:ident {
            $(
                #[ogtk(id=$id:literal)]
                $field_name:ident: $ty:ty
            ),+
            $(,)?
        }
    ) => {
        $vis struct $struct_name {
            $($field_name: $ty),+
        }

        impl $struct_name {
            fn from_builder(builder: &gtk::Builder) -> Result<$struct_name, crate::builder::MissingUIElement> {
                Ok($struct_name {
                    $($field_name: builder.get_object($id).ok_or_else(
                        || crate::builder::MissingUIElement($id.into())
                    )?),+
                })
            }
        }
    }
}
