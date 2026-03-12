use anyhow::Result;

pub trait ResourceProvider {
    fn read_bytes(&mut self, path: &str) -> Result<Vec<u8>>;
    fn read_string(&mut self, path: &str) -> Result<String>;
}
