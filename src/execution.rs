use std::env;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use deno_core::error::AnyError;
use deno_core::ModuleId;
use deno_core::{v8, PollEventLoopOptions};
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_permissions::PermissionsOptions;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::worker::{MainWorker, WorkerServiceOptions};
use tokio::sync::RwLock;

use crate::extension::my_extension;
use crate::extension::HostState;
use crate::loader::TypescriptModuleLoader;

pub async fn run_js(
    file_path: &str,
    fn_name: &str,
    host_state: Arc<RwLock<HostState>>,
) -> Result<(), AnyError> {
    let module_loader = Rc::new(TypescriptModuleLoader);
    let fs = Arc::new(RealFs);
    let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(fs.clone()));
    let permissions = Permissions::from_options(
        permission_desc_parser.as_ref(),
        // Deno has a fine-grained permission system that allows configuration of
        // network access, file system access, environment variables, and more.
        // These can be further restricted to certain hostnames, IP addresses, directories, etc.
        // Further information on Deno's security mechanisms can be found at:
        //   https://docs.deno.com/runtime/fundamentals/security/
        &PermissionsOptions {
            // We only allow network access, and only to specific hostnames we want
            // our scripts to call.
            allow_net: Some(vec!["api.ipify.org".to_string()]),
            // If set to true, scripts trying to access functions not enabled
            // by our setup will result in a command-line prompt.
            // If set to false, they are treated as if they were denied.
            prompt: false,
            ..Default::default()
        },
    )?;
    let services = WorkerServiceOptions {
        module_loader,
        permissions: PermissionsContainer::new(permission_desc_parser, permissions),
        blob_store: Default::default(),
        broadcast_channel: Default::default(),
        feature_checker: Default::default(),
        node_services: Default::default(),
        npm_process_state_provider: Default::default(),
        root_cert_store_provider: Default::default(),
        shared_array_buffer_store: Default::default(),
        compiled_wasm_module_store: Default::default(),
        v8_code_cache: Default::default(),
        fs,
    };
    let options = WorkerOptions {
        extensions: vec![my_extension::init_ops_and_esm()],
        ..Default::default()
    };

    let main_module = deno_core::resolve_path(file_path, &env::current_dir()?)?;

    let mut worker =
        MainWorker::bootstrap_from_options(main_module.clone().into(), services, options);

    // In `op_scripting_demo`, we borrow the host state struct from the OpState.
    // For this to work, we must first insert the host state into Deno's OpState.
    // Values inside the OpState are identified by their type signature and must be
    // retrieved with the same.
    worker.js_runtime.op_state().borrow_mut().put(host_state);

    // We could call `worker.execute_main_module` here, but then we would not be able to access functions
    // exported by the user script.
    // By manually preloading and evaluating the module, we gain access to the internal module id, from
    // which we can extract functions and variables exported by the script.
    let module_id = worker.preload_main_module(&main_module).await?;
    worker.evaluate_module(module_id).await?;
    worker.run_event_loop(false).await?;

    // After the script has been loaded and evaluated, we can access its exports.
    // We retrieve the exported function under the user-provided function name and directly execute it.
    // Theoretically, we could store the function reference for later use and call it in reaction to
    // certain application events.
    let global = get_export_function_global(&mut worker, module_id, fn_name)?;
    let call = worker.js_runtime.call(&global);
    worker
        .js_runtime
        .with_event_loop_promise(call, PollEventLoopOptions::default())
        .await?;

    Ok(())
}

fn get_export_function_global(
    worker: &mut MainWorker,
    module_id: ModuleId,
    export_name: &str,
) -> Result<v8::Global<v8::Function>> {
    // The module namespace holds all exports of the evaluated module.
    let exports_handle = worker.js_runtime.get_module_namespace(module_id)?;
    let mut scope = worker.js_runtime.handle_scope();

    let export_name_v8 =
        v8::String::new(&mut scope, export_name).context("creation of v8 string failed")?;
    let exports = exports_handle.open(&mut scope);
    let binding = exports
        .get(&mut scope, export_name_v8.into())
        .context(format!("no export named '{export_name}'"))?;

    let function: v8::Local<v8::Function> = binding.try_into()?;
    // By converting the function into a v8 global, we can decouple it from the lifetime of the
    // runtime's handle scope.
    let global = v8::Global::new(&mut scope, function);

    Ok(global)
}
