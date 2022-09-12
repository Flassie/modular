use crate::state::WasmModuleState;
use crate::utils::{read_bytes, read_string};
use modular_core::{Callback, CallbackError, CallbackSuccess, Registry};
use std::thread;
use tracing::error;
use wasmer::FunctionEnvMut;

struct GuestCallback {
    tx: std::sync::mpsc::SyncSender<CallbackData>,
}

enum CallbackData {
    Success(Option<Vec<u8>>),
    Error(OwnedError),
}

struct OwnedError {
    code: i32,
    err_name: Option<String>,
    err_description: Option<String>,
    err_data: Option<Vec<u8>>,
}

impl Callback for GuestCallback {
    fn on_success(&self, result: CallbackSuccess) {
        self.tx
            .send(CallbackData::Success(result.data.map(|i| i.to_vec())))
            .unwrap();
    }

    fn on_error(&self, err: CallbackError) {
        self.tx
            .send(CallbackData::Error(OwnedError {
                code: err.code,
                err_name: err.err_name.map(|i| i.to_string()),
                err_description: err.description.map(|i| i.to_string()),
                err_data: err.data.map(|i| i.to_vec()),
            }))
            .unwrap();
    }
}

#[allow(clippy::too_many_arguments)]
pub fn registry_invoke(
    mut env: FunctionEnvMut<WasmModuleState>,
    package: i32,
    package_len: u32,
    method: i32,
    method_len: u32,
    data: i32,
    data_len: u32,
    callback_id: i32,
) -> i32 {
    let vtable = env.data().get_vtable().clone();
    let mem = env.data_mut().get_memory().cloned().unwrap();
    let package = read_string(&mem, package, package_len as _, &env);
    let method = read_string(&mem, method, method_len as _, &env);
    let data = read_bytes(&mem, data, data_len as _, &env);

    if package.is_none() || method.is_none() {
        match vtable.callback_on_error(
            callback_id,
            -1,
            Some(b"InvalidArguments"),
            Some(b"Invalid package or method name"),
            None,
            &mut env,
            &mem,
        ) {
            Ok(_) => {}
            Err(err) => {
                eprintln!("Error calling callback_on_error: {}", err);
            }
        }

        return -1;
    }

    let (tx, rx) = std::sync::mpsc::sync_channel(1);

    let callback = Box::new(GuestCallback { tx });
    let registry = env.data_mut().registry().clone();

    thread::spawn(move || {
        registry.invoke(
            &package.unwrap(),
            &method.unwrap(),
            data.as_deref(),
            callback,
        );
    });

    match rx.recv() {
        Ok(CallbackData::Success(data)) => {
            match vtable.callback_on_success(callback_id, data.as_deref(), &mut env, &mem) {
                Ok(_) => {}
                Err(err) => {
                    error!("Error calling callback_on_success: {}", err);
                }
            }
        }
        Ok(CallbackData::Error(err)) => {
            match vtable.callback_on_error(
                callback_id,
                err.code,
                err.err_name.as_deref().map(|i| i.as_bytes()),
                err.err_description.as_deref().map(|i| i.as_bytes()),
                err.err_data.as_deref(),
                &mut env,
                &mem,
            ) {
                Ok(_) => {}
                Err(err) => {
                    error!("Error calling callback_on_error: {}", err);
                }
            }
        }
        Err(_) => {}
    }

    if let Err(err) = vtable.callback_destroy(callback_id, &mut env) {
        error!("Error calling callback_destroy: {}", err);
    }

    0
}
