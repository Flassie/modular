use modular_core::Error;
use modular_core::{Callback, CallbackError, Module, NativeRegistry, Registry};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use tracing::{debug, error, info};

type ModularEntity = Arc<RwLock<Box<dyn Module>>>;

#[derive(Clone)]
pub struct Modular {
    modules: Arc<RwLock<HashMap<String, ModularEntity>>>,
    is_running: Arc<Mutex<bool>>,
}

impl Modular {
    pub fn new() -> Self {
        let _ = tracing_subscriber::fmt::SubscriberBuilder::default()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        Self {
            modules: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(Mutex::new(false)),
        }
    }
}

impl Registry for Modular {
    fn run(&self) -> Result<(), Error> {
        let mut lock = self.is_running.lock();
        if *lock {
            return Err(Error::RegistryAlreadyRunning);
        }
        *lock = true;
        drop(lock);

        let modules = self
            .modules
            .read()
            .iter()
            .map(|(_, module)| module.clone())
            .collect::<Vec<_>>();

        let mut join_handles = vec![];

        for module in modules {
            let jh = thread::spawn(move || {
                module.read().run();
                module
            });

            join_handles.push(jh);
        }

        for jh in join_handles {
            match jh.join() {
                Ok(module) => {
                    debug!("module {:?} thread finished", module.read().package());
                }
                Err(_) => {
                    error!("error joining thread")
                }
            }
        }

        Ok(())
    }

    fn register_module(&self, module: Box<dyn Module>) {
        let package = module.package().to_string();
        let module = Arc::new(RwLock::new(module));

        info!("registering module {:?}", package);

        self.modules.write().insert(package, module);
    }

    fn deregister_module(&self, package: &str) {
        let m = self.modules.write().remove(package);
        if m.is_none() {
            error!("module {:?} not found", package);
        } else {
            info!("module {:?} deregistered", package);
        }
    }

    fn invoke(
        &self,
        package: &str,
        method: &str,
        data: Option<&[u8]>,
        callback: Box<dyn Callback>,
    ) {
        let module = self.modules.read().get(package).cloned();

        match module {
            Some(v) => {
                v.read().invoke(method, data, callback);
            }
            None => callback.on_error(CallbackError {
                code: Error::ModuleNotFound as i32,
                err_name: Error::ModuleNotFound.as_ref().into(),
                description: Some(&format!("Module {:?} not found", package)),
                data: None,
            }),
        }
    }
}

#[no_mangle]
pub extern "C" fn create_modular() -> NativeRegistry {
    NativeRegistry::new(Modular::new())
}
