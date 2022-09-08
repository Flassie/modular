pub struct RunError {
    pub code: i32,
    pub message: String,
}

impl RunError {
    pub fn new(code: i32, message: String) -> Self {
        Self { code, message }
    }
}

#[repr(i8)]
pub enum RegistryError {
    UnknownModule = -1,
}
