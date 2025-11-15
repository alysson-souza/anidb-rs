//! Handle registry and lifecycle management for FFI
//!
//! This module manages context-scoped registries for all FFI handles, ensuring
//! each client (and derived operations) operate in isolation without global
//! cross-talk.

use crate::ffi::helpers::{c_str_to_string, validate_mut_ptr, validate_ptr};
use crate::ffi::types::{
    AniDBCallbackType, AniDBConfig, AniDBEvent, AniDBEventCallback, AniDBResult, AniDBStatus,
};
use crate::ffi_catch_panic;
use crate::{ClientConfig, Error, FileProcessor};
use std::collections::{HashMap, VecDeque};
use std::ffi::{CString, c_void};
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

type ContextId = u32;
type HandleIndex = usize;

const CONTEXT_ID_BITS: u32 = 16;
const HANDLE_INDEX_BITS: u32 = usize::BITS - CONTEXT_ID_BITS;
const HANDLE_INDEX_MASK: usize = (1usize << HANDLE_INDEX_BITS) - 1;
const CONTEXT_ID_MASK: usize = (1usize << CONTEXT_ID_BITS) - 1;
const MAX_CONTEXT_ID: ContextId = (1 << CONTEXT_ID_BITS) - 1;

const _: [(); 1] = [(); (usize::BITS > CONTEXT_ID_BITS) as usize];

/// Callback registration information
pub(crate) struct CallbackRegistration {
    pub callback_type: AniDBCallbackType,
    pub callback_ptr: *mut c_void,
    pub user_data: *mut c_void,
}

// Ensure CallbackRegistration is Send + Sync by using raw pointers
unsafe impl Send for CallbackRegistration {}
unsafe impl Sync for CallbackRegistration {}

/// Event queue entry
pub(crate) struct EventEntry {
    pub event: AniDBEvent,
    // Store owned strings to ensure they remain valid
    #[allow(dead_code)]
    pub file_path: Option<CString>,
    #[allow(dead_code)]
    pub hash_value: Option<CString>,
    #[allow(dead_code)]
    pub endpoint: Option<CString>,
    #[allow(dead_code)]
    pub context: Option<CString>,
}

// EventEntry is Send because we own all the data
unsafe impl Send for EventEntry {}

/// Internal client state
pub(crate) struct ClientState {
    #[allow(dead_code)]
    pub config: ClientConfig,
    pub file_processor: Arc<FileProcessor>,
    pub runtime: Arc<Runtime>,
    pub last_error: Option<String>,
    #[allow(dead_code)]
    pub reference_count: AtomicUsize,
    #[allow(dead_code)]
    pub context_id: ContextId,
    #[allow(dead_code)]
    pub handle_index: HandleIndex,

    // Callback management
    pub callbacks: Arc<Mutex<HashMap<u64, CallbackRegistration>>>,
    pub next_callback_id: Arc<AtomicU64>,

    // Event system
    pub event_callback: Arc<Mutex<Option<(AniDBEventCallback, usize)>>>, // Store user_data as usize
    pub event_queue: Arc<Mutex<VecDeque<EventEntry>>>,
    pub event_thread_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    pub event_sender: Arc<Mutex<Option<mpsc::UnboundedSender<EventEntry>>>>,
}

/// Internal operation state
#[allow(dead_code)]
pub(crate) struct OperationState {
    pub status: AniDBStatus,
    pub result: Option<crate::FileProcessingResult>,
    pub error: Option<Error>,
}

/// Internal batch state
#[allow(dead_code)]
pub(crate) struct BatchState {
    pub total_files: usize,
    pub completed_files: AtomicUsize,
    pub results: Mutex<Vec<Result<crate::FileProcessingResult, Error>>>,
    pub status: AniDBStatus,
}

// Handle registries
lazy_static::lazy_static! {
    static ref CONTEXTS: RwLock<HashMap<ContextId, Arc<ContextState>>> = RwLock::new(HashMap::new());
    static ref NEXT_CONTEXT_ID: AtomicU32 = AtomicU32::new(1);
    pub(crate) static ref INITIALIZED: AtomicUsize = AtomicUsize::new(0);
}

struct ContextState {
    id: ContextId,
    clients: RwLock<HashMap<HandleIndex, Arc<Mutex<ClientState>>>>,
    operations: RwLock<HashMap<HandleIndex, Arc<Mutex<OperationState>>>>,
    batches: RwLock<HashMap<HandleIndex, Arc<Mutex<BatchState>>>>,
    next_handle_index: AtomicUsize,
}

impl ContextState {
    fn new(id: ContextId) -> Self {
        Self {
            id,
            clients: RwLock::new(HashMap::new()),
            operations: RwLock::new(HashMap::new()),
            batches: RwLock::new(HashMap::new()),
            next_handle_index: AtomicUsize::new(1),
        }
    }

    fn allocate_handle(&self) -> Result<HandleIndex, AniDBResult> {
        let handle_index = self.next_handle_index.fetch_add(1, Ordering::SeqCst);
        if handle_index == 0 || handle_index > HANDLE_INDEX_MASK {
            return Err(AniDBResult::ErrorBusy);
        }
        Ok(handle_index)
    }

    fn is_empty(&self) -> bool {
        let clients_empty = self
            .clients
            .read()
            .map(|clients| clients.is_empty())
            .unwrap_or(false);
        let operations_empty = self
            .operations
            .read()
            .map(|operations| operations.is_empty())
            .unwrap_or(false);
        let batches_empty = self
            .batches
            .read()
            .map(|batches| batches.is_empty())
            .unwrap_or(false);

        clients_empty && operations_empty && batches_empty
    }

    fn clear(&self) {
        if let Ok(mut clients) = self.clients.write() {
            clients.clear();
        }
        if let Ok(mut operations) = self.operations.write() {
            operations.clear();
        }
        if let Ok(mut batches) = self.batches.write() {
            batches.clear();
        }
    }
}

fn allocate_context_id() -> Result<ContextId, AniDBResult> {
    let id = NEXT_CONTEXT_ID.fetch_add(1, Ordering::SeqCst);
    if id == 0 || id > MAX_CONTEXT_ID {
        return Err(AniDBResult::ErrorBusy);
    }
    Ok(id)
}

fn register_context(context: Arc<ContextState>) -> Result<(), AniDBResult> {
    let mut contexts = CONTEXTS.write().map_err(|_| AniDBResult::ErrorBusy)?;
    contexts.insert(context.id, Arc::clone(&context));
    Ok(())
}

fn get_context(context_id: ContextId) -> Result<Arc<ContextState>, AniDBResult> {
    let contexts = CONTEXTS.read().map_err(|_| AniDBResult::ErrorBusy)?;
    contexts
        .get(&context_id)
        .cloned()
        .ok_or(AniDBResult::ErrorInvalidHandle)
}

fn remove_context_if_unused(context_id: ContextId, context: &Arc<ContextState>) {
    if !context.is_empty() {
        return;
    }

    if let Ok(mut contexts) = CONTEXTS.write()
        && let Some(current) = contexts.get(&context_id)
        && Arc::ptr_eq(current, context)
    {
        contexts.remove(&context_id);
    }
}

fn encode_handle(context_id: ContextId, handle_index: HandleIndex) -> Result<usize, AniDBResult> {
    if context_id == 0 || context_id > MAX_CONTEXT_ID {
        return Err(AniDBResult::ErrorInvalidHandle);
    }

    if handle_index == 0 || handle_index > HANDLE_INDEX_MASK {
        return Err(AniDBResult::ErrorInvalidHandle);
    }

    Ok(((handle_index & HANDLE_INDEX_MASK) << CONTEXT_ID_BITS) | context_id as usize)
}

pub(crate) fn decode_handle_parts(
    handle_id: usize,
) -> Result<(ContextId, HandleIndex), AniDBResult> {
    if handle_id == 0 {
        return Err(AniDBResult::ErrorInvalidHandle);
    }

    let context_id = (handle_id & CONTEXT_ID_MASK) as ContextId;
    let handle_index = handle_id >> CONTEXT_ID_BITS;

    if context_id == 0 || handle_index == 0 {
        return Err(AniDBResult::ErrorInvalidHandle);
    }

    Ok((context_id, handle_index))
}

fn insert_client(
    context: &Arc<ContextState>,
    handle_index: HandleIndex,
    client_arc: Arc<Mutex<ClientState>>,
) -> Result<(), AniDBResult> {
    let mut clients = context
        .clients
        .write()
        .map_err(|_| AniDBResult::ErrorBusy)?;
    clients.insert(handle_index, client_arc);
    Ok(())
}

fn remove_client(
    context: &Arc<ContextState>,
    handle_index: HandleIndex,
) -> Result<bool, AniDBResult> {
    let mut clients = context
        .clients
        .write()
        .map_err(|_| AniDBResult::ErrorBusy)?;
    Ok(clients.remove(&handle_index).is_some())
}

fn create_context() -> Result<Arc<ContextState>, AniDBResult> {
    let context_id = allocate_context_id()?;
    let context = Arc::new(ContextState::new(context_id));
    register_context(Arc::clone(&context))?;
    Ok(context)
}

fn cleanup_context_by_id(context_id: ContextId) {
    let context = match CONTEXTS.write() {
        Ok(mut contexts) => contexts.remove(&context_id),
        Err(_) => None,
    };

    if let Some(context) = context {
        context.clear();
    }
}

pub(crate) fn cleanup_context_for_handle(handle_id: usize) -> AniDBResult {
    match decode_handle_parts(handle_id) {
        Ok((context_id, _)) => {
            cleanup_context_by_id(context_id);
            AniDBResult::Success
        }
        Err(err) => err,
    }
}

pub(crate) fn cleanup_all_contexts() {
    if let Ok(mut contexts) = CONTEXTS.write() {
        let drained: Vec<_> = contexts.drain().map(|(_, ctx)| ctx).collect();
        drop(contexts);
        for context in drained {
            context.clear();
        }
    }
}

pub(crate) fn resolve_client(handle_id: usize) -> Result<Arc<Mutex<ClientState>>, AniDBResult> {
    let (context_id, handle_index) = decode_handle_parts(handle_id)?;
    let context = get_context(context_id)?;
    let clients = context.clients.read().map_err(|_| AniDBResult::ErrorBusy)?;
    clients
        .get(&handle_index)
        .cloned()
        .ok_or(AniDBResult::ErrorInvalidHandle)
}

/// Create a new AniDB client instance with default configuration
#[unsafe(no_mangle)]
pub extern "C" fn anidb_client_create(handle: *mut *mut c_void) -> AniDBResult {
    ffi_catch_panic!({
        if !validate_mut_ptr(handle) {
            return AniDBResult::ErrorInvalidParameter;
        }

        let config = ClientConfig::default();
        create_client_with_config(config, handle)
    })
}

/// Create a new AniDB client instance with custom configuration
#[unsafe(no_mangle)]
pub extern "C" fn anidb_client_create_with_config(
    config: *const AniDBConfig,
    handle: *mut *mut c_void,
) -> AniDBResult {
    ffi_catch_panic!({
        // Validate parameters
        if !validate_ptr(config) || !validate_mut_ptr(handle) {
            return AniDBResult::ErrorInvalidParameter;
        }

        // Safe config access
        let ffi_config = unsafe { &*config };

        // Parse configuration with validation

        let username = if ffi_config.username.is_null() {
            None
        } else {
            match c_str_to_string(ffi_config.username) {
                Ok(s) => Some(s),
                Err(e) => return e,
            }
        };

        let password = if ffi_config.password.is_null() {
            None
        } else {
            match c_str_to_string(ffi_config.password) {
                Ok(s) => Some(s),
                Err(e) => return e,
            }
        };

        let client_name = if ffi_config.client_name.is_null() {
            None
        } else {
            match c_str_to_string(ffi_config.client_name) {
                Ok(s) => Some(s),
                Err(e) => return e,
            }
        };

        let client_version = if ffi_config.client_version.is_null() {
            None
        } else {
            match c_str_to_string(ffi_config.client_version) {
                Ok(s) => Some(s),
                Err(e) => return e,
            }
        };

        // Validate numeric parameters
        let max_concurrent = ffi_config.max_concurrent_files.clamp(1, 100);
        let chunk_size = ffi_config.chunk_size.clamp(1024, 10 * 1024 * 1024);
        let max_memory = ffi_config
            .max_memory_usage
            .clamp(10 * 1024 * 1024, 2 * 1024 * 1024 * 1024); // 10MB to 2GB

        let client_config = ClientConfig {
            max_concurrent_files: max_concurrent,
            chunk_size,
            max_memory_usage: max_memory,
            username,
            password,
            client_name,
            client_version,
        };

        create_client_with_config(client_config, handle)
    })
}

/// Internal helper to create client with config
pub(crate) fn create_client_with_config(
    config: ClientConfig,
    handle: *mut *mut c_void,
) -> AniDBResult {
    // Set the global memory limit based on config
    crate::buffer::set_memory_limit(config.max_memory_usage);

    // Create runtime
    let runtime = match Runtime::new() {
        Ok(rt) => Arc::new(rt),
        Err(_) => return AniDBResult::ErrorProcessing,
    };

    // Create file processor
    let file_processor = Arc::new(FileProcessor::new(config.clone()));

    let context = match create_context() {
        Ok(ctx) => ctx,
        Err(err) => return err,
    };

    let handle_index = match context.allocate_handle() {
        Ok(idx) => idx,
        Err(err) => {
            cleanup_context_by_id(context.id);
            return err;
        }
    };

    let state = ClientState {
        config,
        file_processor,
        runtime,
        last_error: None,
        reference_count: AtomicUsize::new(1),
        context_id: context.id,
        handle_index,
        callbacks: Arc::new(Mutex::new(HashMap::new())),
        next_callback_id: Arc::new(AtomicU64::new(1)),
        event_callback: Arc::new(Mutex::new(None)),
        event_queue: Arc::new(Mutex::new(VecDeque::new())),
        event_thread_handle: Arc::new(Mutex::new(None)),
        event_sender: Arc::new(Mutex::new(None)),
    };

    let client_arc = Arc::new(Mutex::new(state));

    if let Err(err) = insert_client(&context, handle_index, Arc::clone(&client_arc)) {
        cleanup_context_by_id(context.id);
        return err;
    }

    let handle_id = match encode_handle(context.id, handle_index) {
        Ok(id) => id,
        Err(err) => {
            cleanup_context_by_id(context.id);
            return err;
        }
    };

    unsafe {
        *handle = handle_id as *mut c_void;
    }

    AniDBResult::Success
}

/// Destroy an AniDB client instance
#[unsafe(no_mangle)]
pub extern "C" fn anidb_client_destroy(handle: *mut c_void) -> AniDBResult {
    ffi_catch_panic!({
        if !validate_mut_ptr(handle) {
            return AniDBResult::ErrorInvalidHandle;
        }

        let handle_id = handle as usize;
        let (context_id, handle_index) = match decode_handle_parts(handle_id) {
            Ok(parts) => parts,
            Err(err) => return err,
        };

        let context = match get_context(context_id) {
            Ok(ctx) => ctx,
            Err(err) => return err,
        };

        match remove_client(&context, handle_index) {
            Ok(true) => {
                remove_context_if_unused(context_id, &context);
                AniDBResult::Success
            }
            Ok(false) => AniDBResult::ErrorInvalidHandle,
            Err(err) => err,
        }
    })
}
