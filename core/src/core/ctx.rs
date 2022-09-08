use crate::core::RegistryInvocationCallback;

pub struct InvocationContext<'a> {
    pub data: Option<&'a [u8]>,
    pub method: &'a str,
    pub callback: RegistryInvocationCallback,
}
