#[repr(i32)]
#[derive(Default, Eq, PartialEq)]
pub enum Error {
    #[default]
    NoError = 0,
    RegistryAlreadyRunning = i32::MIN,
    ModuleNotFound = i32::MIN + 1,
    FfiInvalidMethodName = i32::MIN + 2,
}

impl AsRef<str> for Error {
    fn as_ref(&self) -> &str {
        match self {
            Self::RegistryAlreadyRunning => "Registry already running",
            Self::ModuleNotFound => "Module not found",
            Self::FfiInvalidMethodName => "Invalid method name",
            _ => "",
        }
    }
}
