use crate::core::*;
use modular_native::Callback;
use parking_lot::lock_api::ArcRwLockReadGuard;
use parking_lot::{RawRwLock, RwLock};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use tracing::*;

#[derive(Default, Clone)]
pub struct Registry {
    modules: Arc<RwLock<HashMap<String, RegistryModule>>>,
    call_chain: Vec<String>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn call_chain(&self) -> &[String] {
        &self.call_chain
    }

    pub fn root_caller(&self) -> Option<&str> {
        self.call_chain.first().map(|s| s.as_str())
    }

    #[instrument(
        skip(self),
        fields(
            chain = %self.call_chain.join("->")
        ),
    )]
    pub async fn run(&self) {
        let modules = self.modules.read().clone();

        let mut handles = vec![];

        for (name, module) in modules {
            let module = module.clone();
            let module_name = name.clone();

            let handle = tokio::task::spawn_blocking(move || {
                debug!("running module {:?}", name);
                match module.read().run(&module.registry) {
                    Ok(_) => debug!("module {:?} finished", name),
                    Err(err) => error!("module {:?} failed: {}", name, err.message),
                }
            });

            handles.push((module_name, handle));
        }

        for (module, handle) in handles {
            match handle.await {
                Ok(()) => {
                    debug!("module {:?} thread finished", module);
                }
                Err(err) => {
                    error!("module {:?} join failed: {}", module, err);
                }
            }
        }
    }

    #[instrument(
        skip(self, module),
        fields(
            chain = %self.call_chain.join("->")
        )
    )]
    pub fn register_module<M>(&self, module: M)
    where
        M: Module + 'static,
    {
        trace!(
            "registering module \"{}:{}\"",
            module.package(),
            module.version()
        );

        let package = module.package().to_string();
        let module = RegistryModule::new(module, self);

        self.modules.write().insert(package, module.clone());
    }

    #[instrument(
        skip(self, package),
        fields(
            chain = %self.call_chain.join("->")
        )
    )]
    pub fn deregister_module<P: AsRef<str>>(&self, package: P) {
        let module = self.modules.write().remove(package.as_ref());
        if let Some(module) = module {
            debug!("trying to destroy module {:?}", package.as_ref());
            module.write().destroy(self);
            debug!("module {:?} destroyed", package.as_ref());
        } else {
            debug!("module {:?} not found", package.as_ref());
        }
    }

    #[instrument(
        skip(self, package, method, data, callback),
        fields(
            chain = %self.call_chain.join("->")
        )
    )]
    pub fn invoke<P, M, D, C>(
        &self,
        package: P,
        method: M,
        data: Option<D>,
        callback: C,
    ) -> Result<(), RegistryError>
    where
        P: AsRef<str>,
        M: AsRef<str>,
        D: AsRef<[u8]>,
        C: Callback + 'static,
    {
        let mut module = {
            let modules = self.modules.read();
            modules
                .get(package.as_ref())
                .ok_or(RegistryError::UnknownModule)?
                .clone()
        };

        let call_desc = format!("{}:{}", package.as_ref(), method.as_ref());
        module.registry.call_chain.push(call_desc);

        let registry = &module.registry;
        let module = Arc::new(module.read_arc());

        let ctx = InvocationContext {
            callback: RegistryInvocationCallback {
                callback: Box::new(callback),
                _lock: module.clone(),
            },
            data: data.as_ref().map(|i| i.as_ref()),
            method: method.as_ref(),
        };

        module.invoke(ctx, registry);

        Ok(())
    }

    #[instrument(
        skip(self),
        fields(
            chain = %self.call_chain.join("->")
        )
    )]
    fn clone_inner(&self, module_name: &str) -> Self {
        let mut call_chain = self.call_chain.clone();
        call_chain.push(module_name.to_string());

        Self {
            modules: self.modules.clone(),
            call_chain,
        }
    }
}

pub struct RegistryInvocationCallback {
    callback: Box<dyn Callback>,
    _lock: Arc<ArcRwLockReadGuard<RawRwLock, dyn Module>>,
}

impl Drop for RegistryInvocationCallback {
    fn drop(&mut self) {
        trace!("dropping invocation callback");
    }
}

impl RegistryInvocationCallback {
    pub fn on_error<ErrName, Description, Data>(
        mut self,
        code: i32,
        err_name: ErrName,
        description: Option<Description>,
        data: Option<Data>,
    ) where
        ErrName: AsRef<str>,
        Description: AsRef<str>,
        Data: AsRef<[u8]>,
    {
        self.callback.on_error(
            code,
            err_name.as_ref(),
            description.as_ref().map(|i| i.as_ref()),
            data.as_ref().map(|i| i.as_ref()),
        );
    }

    pub fn on_success<Data>(mut self, result: Option<Data>)
    where
        Data: AsRef<[u8]>,
    {
        self.callback
            .on_success(result.as_ref().map(|i| i.as_ref()));
    }
}

#[derive(Clone)]
struct RegistryModule {
    inner: Arc<RwLock<dyn Module>>,
    registry: Registry,
}

impl RegistryModule {
    pub fn new<M: Module + 'static>(module: M, registry: &Registry) -> Self {
        let registry = registry.clone_inner(module.package());

        Self {
            registry,
            inner: Arc::new(RwLock::new(module)),
        }
    }
}

impl Deref for RegistryModule {
    type Target = Arc<RwLock<dyn Module>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
