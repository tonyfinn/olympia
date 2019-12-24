//! olympia_core provides definitions of fundamental types for
//! olympia that are required by both olympia_core and
//! olympia_derive.

pub mod address;
pub mod instructions;
pub mod registers;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
