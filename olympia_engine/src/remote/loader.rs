pub enum DataType {
    ROM,
}

pub trait DataManager {
    type Error;
    type Identifier;
    fn load(&self, ty: DataType, identifier: Self::Identifier) -> Result<Vec<u8>, Self::Error>;
}
