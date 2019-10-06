pub mod cpu;
pub mod decoder;
pub mod types;
mod instructions;
mod registers;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
