use crate::utils::{OptionalCallback, OptionalCallbackRef};
use crate::vtable::WasmModuleVTable;
use modular_core::{Callback, NativeRegistry, Registry};
use std::collections::HashMap;
use uuid::Uuid;
use wasmer::Memory;

pub struct WasmModuleState {
    callbacks: HashMap<Uuid, Box<dyn Callback>>,
    memory: Option<Memory>,
    registry: NativeRegistry,
    vtable: Option<WasmModuleVTable>,
}

impl WasmModuleState {
    pub fn new<R: Registry + 'static>(registry: R) -> Self {
        Self {
            callbacks: HashMap::new(),
            memory: None,
            registry: NativeRegistry::new(registry),
            vtable: None,
        }
    }
    
    pub fn registry(&self) -> &NativeRegistry {
        &self.registry
    }

    pub fn set_vtable(&mut self, vtable: &WasmModuleVTable) {
        self.vtable = Some(vtable.clone());
    }

    pub fn get_vtable(&self) -> &WasmModuleVTable {
        self.vtable.as_ref().unwrap()
    }

    pub fn set_memory(&mut self, memory: Memory) {
        self.memory = Some(memory);
    }

    pub fn get_memory(&self) -> Option<&Memory> {
        self.memory.as_ref()
    }

    pub fn add_callback(&mut self, uid: Uuid, callback: Box<dyn Callback>) {
        self.callbacks.insert(uid, callback);
    }

    pub fn remove_callback(&mut self, uid: &Uuid) -> OptionalCallback {
        OptionalCallback::new(self.callbacks.remove(uid))
    }

    pub fn get_callback(&self, uid: &Uuid) -> OptionalCallbackRef {
        OptionalCallbackRef::new(self.callbacks.get(uid).map(|i| i.as_ref()))
    }
}
