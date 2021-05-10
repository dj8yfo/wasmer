use crate::tunables::BaseTunables;
use loupe::MemoryUsage;
use std::any::Any;
use std::fmt;
use std::sync::Arc;
#[cfg(all(feature = "compiler", feature = "engine"))]
use wasmer_compiler::CompilerConfig;
use wasmer_engine::{is_wasm_pc, Engine, Tunables};
use wasmer_vm::{init_traps, SignalHandler, TrapInfo};

/// The store represents all global state that can be manipulated by
/// WebAssembly programs. It consists of the runtime representation
/// of all instances of functions, tables, memories, and globals that
/// have been allocated during the lifetime of the abstract machine.
///
/// The `Store` holds the engine (that is —amongst many things— used to compile
/// the Wasm bytes into a valid module artifact), in addition to the
/// [`Tunables`] (that are used to create the memories, tables and globals).
///
/// Spec: <https://webassembly.github.io/spec/core/exec/runtime.html#store>
#[derive(Clone, MemoryUsage)]
pub struct Store {
    engine: Arc<dyn Engine + Send + Sync>,
    tunables: Arc<dyn Tunables + Send + Sync>,
}

impl Store {
    /// Creates a new `Store` with a specific [`Engine`].
    pub fn new<E>(engine: &E) -> Self
    where
        E: Engine + ?Sized,
    {
        Self::new_with_tunables(engine, BaseTunables::for_target(engine.target()))
    }

    /// Creates a new `Store` with a specific [`Engine`] and [`Tunables`].
    pub fn new_with_tunables<E>(engine: &E, tunables: impl Tunables + Send + Sync + 'static) -> Self
    where
        E: Engine + ?Sized,
    {
        // Make sure the signal handlers are installed.
        // This is required for handling traps.
        init_traps(is_wasm_pc);

        Self {
            engine: engine.cloned(),
            tunables: Arc::new(tunables),
        }
    }

    /// Returns the [`Tunables`].
    pub fn tunables(&self) -> &dyn Tunables {
        self.tunables.as_ref()
    }

    /// Returns the [`Engine`].
    pub fn engine(&self) -> &Arc<dyn Engine + Send + Sync> {
        &self.engine
    }

    /// Checks whether two stores are identical. A store is considered
    /// equal to another store if both have the same engine. The
    /// tunables are excluded from the logic.
    pub fn same(a: &Self, b: &Self) -> bool {
        a.engine.id() == b.engine.id()
    }
}

impl PartialEq for Store {
    fn eq(&self, other: &Self) -> bool {
        Self::same(self, other)
    }
}

unsafe impl TrapInfo for Store {
    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn custom_signal_handler(&self, _call: &dyn Fn(&SignalHandler) -> bool) -> bool {
        // if let Some(handler) = &*self.inner.signal_handler.borrow() {
        //     return call(handler);
        // }
        false
    }
}

// We only implement default if we have assigned a default compiler and engine
#[cfg(all(feature = "default-compiler", feature = "default-engine"))]
impl Default for Store {
    fn default() -> Self {
        // We store them on a function that returns to make
        // sure this function doesn't emit a compile error even if
        // more than one compiler is enabled.
        #[allow(unreachable_code)]
        fn get_config() -> impl CompilerConfig + 'static {
            cfg_if::cfg_if! {
                if #[cfg(feature = "default-cranelift")] {
                    wasmer_compiler_cranelift::Cranelift::default()
                } else if #[cfg(feature = "default-llvm")] {
                    wasmer_compiler_llvm::LLVM::default()
                } else if #[cfg(feature = "default-singlepass")] {
                    wasmer_compiler_singlepass::Singlepass::default()
                } else {
                    compile_error!("No default compiler chosen")
                }
            }
        }

        #[allow(unreachable_code, unused_mut)]
        fn get_engine(mut config: impl CompilerConfig + 'static) -> impl Engine + Send + Sync {
            cfg_if::cfg_if! {
                if #[cfg(feature = "default-jit")] {
                    wasmer_engine_jit::JIT::new(config)
                        .engine()
                } else if #[cfg(feature = "default-native")] {
                    wasmer_engine_native::Native::new(config)
                        .engine()
                } else {
                    compile_error!("No default engine chosen")
                }
            }
        }

        let config = get_config();
        let engine = get_engine(config);
        let tunables = BaseTunables::for_target(engine.target());
        Store {
            engine: Arc::new(engine),
            tunables: Arc::new(tunables),
        }
    }
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Store").finish()
    }
}

/// A trait represinting any object that lives in the `Store`.
pub trait StoreObject {
    /// Return true if the object `Store` is the same as the provided `Store`.
    fn comes_from_same_store(&self, store: &Store) -> bool;
}
