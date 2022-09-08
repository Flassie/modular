use crate::core::*;

#[allow(unused_variables)]
pub trait Module: Send + Sync {
    fn package(&self) -> &str;
    fn version(&self) -> &str;

    fn invoke(&self, ctx: InvocationContext, registry: &Registry);
    fn run(&self, registry: &Registry) -> Result<(), RunError> {
        Ok(())
    }
    fn destroy(&self, registry: &Registry) {}
}
