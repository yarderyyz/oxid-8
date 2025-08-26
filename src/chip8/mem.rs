use crate::chip8::consts::RAM_SIZE;

#[derive(Clone)]
pub struct Memory(pub [u8; RAM_SIZE]);
impl Default for Memory {
    fn default() -> Self {
        Self([0; RAM_SIZE])
    }
}
impl std::ops::Deref for Memory {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for Memory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
