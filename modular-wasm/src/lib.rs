// // mod main;
//
// use modular_core::NativeCallback;
// use std::collections::HashMap;
// use wasmtime::{
//     Caller, Config, Engine, Func, FuncType, Instance, Linker, Module, Store, Val, ValRaw,
// };
//
// struct WasmStore {
//     callbacks: Vec<NativeCallback>,
// }
//
// pub struct WasmModulesFactory {
//     engine: Engine,
// }
//
// impl WasmModulesFactory {
//     pub fn new() -> Self {
//         Self {
//             engine: Engine::new(&Config::new().wasm_reference_types(true)).unwrap(),
//         }
//     }
//
//     pub fn load(&self, bytes: impl AsRef<[u8]>) -> anyhow::Result<()> {
//         let module = Module::new(&self.engine, bytes)?;
//         let linker = Linker::new(&self.engine);
//         let mut store = Store::new(&self.engine, ());
//         let instance = linker.instantiate(&mut store, &module)?;
//
//         let f = Func::wrap(&mut store, |mut caller: Caller<'_, ()>| 100500);
//
//         let f = unsafe { f.to_raw(&mut store) };
//
//         instance.
//
//         // let table = instance
//         //     .get_table(&mut store, "__indirect_function_table")
//         //     .unwrap();
//         // for v in 0..table.size(&store) {
//         //     println!("table entry: {:?}", table.get(&mut store, v).unwrap());
//         //     if let Some(Val::FuncRef(Some(f))) = table.get(&mut store, v) {
//         //         println!("f: {:?}", f.ty(&store))
//         //     }
//         // }
//
//         println!("f: {:?}", f);
//
//         let a = instance
//             .get_typed_func::<i32, i32, _>(&mut store, "test")
//             .unwrap();
//
//         a.call(&mut store, f as i32).unwrap();
//
//         // let alloc = instance
//         //     .get_typed_func::<i32, i32, _>(&mut store, "__allocate")
//         //     .unwrap();
//         //
//         // let dealloc = instance
//         //     .get_typed_func::<i32, (), _>(&mut store, "__deallocate")
//         //     .unwrap();
//         //
//         // let create_module = instance
//         //     .get_typed_func::<i32, (), _>(&mut store, "create_module")
//         //     .unwrap();
//         //
//         // let struct_ptr = alloc.call(&mut store, 96).unwrap();
//         // create_module.call(&mut store, struct_ptr).unwrap();
//         //
//         // let mem = instance.get_memory(&mut store, "memory").unwrap();
//         // let value = &mem.data(&store)[(struct_ptr as usize)..(struct_ptr as usize + 12)];
//         //
//         // #[repr(C)]
//         // #[derive(Debug)]
//         // struct Test {
//         //     instance: i32,
//         //     fun: i32,
//         //     fun3: i32,
//         // }
//         //
//         // let value = unsafe { &*(value.as_ptr() as *const () as *const Test) };
//         //
//         // println!("v: {:?}", value);
//         //
//         // dealloc.call(&mut store, struct_ptr).unwrap();
//         //
//
//         Ok(())
//     }
// }
//
// #[test]
// fn a() {
//     let bytes = include_bytes!("../../target/wasm32-unknown-unknown/debug/wasm_example.wasm");
//     let factory = WasmModulesFactory::new();
//     factory.load(bytes).unwrap();
// }
