use modular::{Modular, NativeRegistry, Registry};
use modular_dll::DllModule;
use modular_tracing_core::{register_module_tracer, LazyBytesRecorder, LazyRecorder};
use modular_wasm::WasmModule;

fn main() {
    let recorder = modular_tracing_core::DefaultRecorder::new();
    let recorder = Box::leak(Box::new(recorder));
    let (runner, receiver) = LazyRecorder::new(recorder);
    register_module_tracer(recorder);

    runner.run();

    let modular = Modular::default();

    // let (modular, _lib) = unsafe {
    //     let lib = libloading::Library::new("target/debug/libmodular.dylib").unwrap();
    //     let create_modular = lib
    //         .get::<extern "C" fn() -> NativeRegistry>(b"create_modular")
    //         .unwrap();
    //
    //     (create_modular(), lib)
    // };

    let module1 =
        DllModule::new("target/debug/libmodule1.dylib", &modular, receiver.clone()).unwrap();
    let module2 =
        DllModule::new("target/debug/libmodule2.dylib", &modular, receiver.clone()).unwrap();
    // let module3 = WasmModule::new(
    //     include_bytes!("../../target/wasm32-wasi/debug/wasm_example.wasm"),
    //     modular.clone(),
    // )
    // .unwrap();

    modular.register_module(Box::new(module1));
    modular.register_module(Box::new(module2));
    // modular.register_module(Box::new(module3));

    let _ = modular.run();

    // modular.deregister_module("wasm-example.module1");
    // modular.deregister_module("wasm-example.module2");

    drop(modular)
}
