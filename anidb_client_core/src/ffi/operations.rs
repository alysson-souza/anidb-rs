//! File processing operations for FFI
//!
//! This module contains all file processing, hashing, caching, and
//! identification operations exposed through the FFI layer.

use crate::Progress;
use crate::ffi::events::{create_file_event, create_memory_event, send_event};
use crate::ffi::handles::CLIENTS;
use crate::ffi::helpers::*;
use crate::ffi::types::*;
use crate::ffi_catch_panic;
use crate::ffi_memory::{
    AllocationType, MemoryPressure, check_memory_pressure, ffi_allocate_buffer, ffi_free_string,
    get_memory_stats,
};
use crate::progress::{ProgressProvider, ProgressUpdate};
use std::ffi::{CString, c_char, c_void};
use std::path::Path;
use std::ptr;
use tokio::sync::mpsc;

/// FFI progress provider that bridges to callbacks
struct FfiProgressProvider {
    tx: mpsc::Sender<Progress>,
}

impl FfiProgressProvider {
    fn new(tx: mpsc::Sender<Progress>) -> Self {
        Self { tx }
    }
}

impl ProgressProvider for FfiProgressProvider {
    fn report(&self, update: ProgressUpdate) {
        // Convert ProgressUpdate to Progress for FFI callbacks
        let progress = match update {
            ProgressUpdate::FileProgress {
                bytes_processed,
                total_bytes,
                operation,
                throughput_mbps,
                memory_usage_bytes,
                buffer_size,
                ..
            } => Progress {
                percentage: if total_bytes > 0 {
                    (bytes_processed as f64 / total_bytes as f64) * 100.0
                } else {
                    0.0
                },
                bytes_processed,
                total_bytes,
                throughput_mbps: throughput_mbps.unwrap_or(0.0),
                current_operation: operation,
                memory_usage_bytes,
                peak_memory_bytes: None,
                buffer_size,
            },
            ProgressUpdate::HashProgress {
                algorithm,
                bytes_processed,
                total_bytes,
            } => Progress {
                percentage: if total_bytes > 0 {
                    (bytes_processed as f64 / total_bytes as f64) * 100.0
                } else {
                    0.0
                },
                bytes_processed,
                total_bytes,
                throughput_mbps: 0.0,
                current_operation: format!("Calculating {algorithm} hash"),
                memory_usage_bytes: None,
                peak_memory_bytes: None,
                buffer_size: None,
            },
            _ => return, // Ignore other update types for FFI
        };

        // Try to send, if channel is full it means the receiver is backlogged
        // In that case, we'll skip this update to avoid blocking
        let _ = self.tx.try_send(progress);
    }

    fn create_child(&self, _name: &str) -> Box<dyn ProgressProvider> {
        // For FFI, just return a clone that uses the same channel
        Box::new(FfiProgressProvider {
            tx: self.tx.clone(),
        })
    }

    fn complete(&self) {
        // No special handling needed for completion in FFI
    }
}

/// Process a single file synchronously
#[unsafe(no_mangle)]
pub extern "C" fn anidb_process_file(
    handle: *mut c_void,
    file_path: *const c_char,
    options: *const AniDBProcessOptions,
    result: *mut *mut AniDBFileResult,
) -> AniDBResult {
    ffi_catch_panic!({
        // Comprehensive parameter validation
        if !validate_mut_ptr(handle)
            || !validate_c_str(file_path)
            || !validate_ptr(options)
            || !validate_mut_ptr(result)
        {
            return AniDBResult::ErrorInvalidParameter;
        }

        let handle_id = handle as usize;

        // Validate handle ID
        if handle_id == 0 || handle_id > usize::MAX / 2 {
            return AniDBResult::ErrorInvalidHandle;
        }

        // Parse file path with safety
        let file_path_str = match c_str_to_string(file_path) {
            Ok(s) => s,
            Err(e) => return e,
        };

        // Parse options with validation
        let opts = unsafe { &*options };

        // Validate algorithm parameters
        if !validate_ptr(opts.algorithms) || opts.algorithm_count == 0 {
            return AniDBResult::ErrorInvalidParameter;
        }

        // Limit algorithm count to prevent excessive memory allocation
        if opts.algorithm_count > 10 {
            return AniDBResult::ErrorInvalidParameter;
        }

        // Parse algorithms with bounds checking
        let mut algorithms = Vec::with_capacity(opts.algorithm_count);
        for i in 0..opts.algorithm_count {
            let ffi_algo = unsafe {
                // Bounds are already checked by algorithm_count validation
                *opts.algorithms.add(i)
            };
            match convert_hash_algorithm(ffi_algo) {
                Ok(algo) => algorithms.push(algo),
                Err(e) => return e,
            }
        }

        // Get client first to check for callbacks
        let clients = match CLIENTS.read() {
            Ok(c) => c,
            Err(_) => return AniDBResult::ErrorBusy,
        };

        let client_arc = match clients.get(&handle_id) {
            Some(c) => c.clone(),
            None => return AniDBResult::ErrorInvalidHandle,
        };

        drop(clients); // Release read lock

        // Get client callbacks for progress
        let client_callbacks = {
            let client = match client_arc.lock() {
                Ok(c) => c,
                Err(_) => return AniDBResult::ErrorBusy,
            };
            client.callbacks.clone()
        };

        // Create progress provider if needed (callback or registered callbacks)
        let progress_provider: Box<dyn ProgressProvider> = if (opts.progress_callback.is_some()
            || has_progress_callbacks(&client_callbacks))
            && opts.enable_progress != 0
        {
            let (tx, mut rx) = mpsc::channel::<Progress>(100);

            // Spawn task to forward progress to callbacks
            let callback = opts.progress_callback;
            let user_data = opts.user_data as usize;

            std::thread::spawn(move || {
                while let Some(progress) = rx.blocking_recv() {
                    let percentage =
                        (progress.bytes_processed as f32 / progress.total_bytes as f32) * 100.0;

                    // Call callback if provided
                    if let Some(cb) = callback {
                        let user_data_ptr = user_data as *mut c_void;
                        cb(
                            percentage,
                            progress.bytes_processed,
                            progress.total_bytes,
                            user_data_ptr,
                        );
                    }

                    // Call registered progress callbacks
                    invoke_callbacks(&client_callbacks, AniDBCallbackType::Progress, |reg| {
                        let callback_fn = unsafe {
                            std::mem::transmute::<*mut c_void, AniDBProgressCallback>(
                                reg.callback_ptr,
                            )
                        };
                        callback_fn(
                            percentage,
                            progress.bytes_processed,
                            progress.total_bytes,
                            reg.user_data,
                        );
                    });
                }
            });

            Box::new(FfiProgressProvider::new(tx))
        } else {
            Box::new(crate::progress::NullProvider)
        };

        // Re-acquire client lock for processing
        let mut client = match client_arc.lock() {
            Ok(c) => c,
            Err(_) => return AniDBResult::ErrorBusy,
        };

        // Send file start event
        let file_metadata = std::fs::metadata(&file_path_str).ok();
        let file_size = file_metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        send_event(
            &client,
            create_file_event(AniDBEventType::FileStart, &file_path_str, file_size, None),
        );

        // Check memory pressure before processing
        let pressure = check_memory_pressure();
        if pressure == MemoryPressure::Critical {
            let stats = get_memory_stats();
            send_event(
                &client,
                create_memory_event(
                    stats.total_memory_used as u64,
                    stats.memory_limit as u64,
                    Some("Critical memory pressure before file processing"),
                ),
            );
        }

        // Process file
        let runtime = client.runtime.clone();
        let file_processor = client.file_processor.clone();
        let path = Path::new(&file_path_str);

        let processing_result = runtime.block_on(async {
            let progress_arc: std::sync::Arc<dyn crate::progress::ProgressProvider> =
                std::sync::Arc::from(progress_provider);
            file_processor
                .process_file(path, &algorithms, progress_arc)
                .await
        });

        match processing_result {
            Ok(proc_result) => {
                // Create FFI result
                let mut ffi_result = Box::new(AniDBFileResult {
                    file_path: string_to_c_string(&proc_result.file_path.to_string_lossy()),
                    file_size: proc_result.file_size,
                    status: AniDBStatus::Completed,
                    hashes: ptr::null_mut(),
                    hash_count: proc_result.hashes.len(),
                    processing_time_ms: proc_result.processing_time.as_millis() as u64,
                    error_message: ptr::null_mut(),
                });

                // Allocate hash results using tracked allocation
                if ffi_result.hash_count > 0 {
                    let hash_results_size =
                        ffi_result.hash_count * std::mem::size_of::<AniDBHashResult>();
                    let buffer =
                        match ffi_allocate_buffer(hash_results_size, AllocationType::HashResult) {
                            Ok(buf) => buf,
                            Err(_) => {
                                // Clean up allocated file path before returning
                                if !ffi_result.file_path.is_null() {
                                    unsafe {
                                        ffi_free_string(ffi_result.file_path);
                                    }
                                }
                                return AniDBResult::ErrorOutOfMemory;
                            }
                        };

                    let hashes_ptr = buffer.as_ptr() as *mut AniDBHashResult;
                    std::mem::forget(buffer); // Don't drop, we'll manage it manually

                    let hash_slice = unsafe {
                        std::slice::from_raw_parts_mut(hashes_ptr, ffi_result.hash_count)
                    };

                    for (i, (algo, hash)) in proc_result.hashes.iter().enumerate() {
                        hash_slice[i] = AniDBHashResult {
                            algorithm: convert_hash_algorithm_to_ffi(algo),
                            hash_value: string_to_c_string(hash),
                            hash_length: hash.len(),
                        };
                    }

                    ffi_result.hashes = hashes_ptr;
                }

                unsafe {
                    *result = Box::into_raw(ffi_result);
                }

                // Send file complete event
                send_event(
                    &client,
                    create_file_event(
                        AniDBEventType::FileComplete,
                        &file_path_str,
                        proc_result.file_size,
                        Some(&format!(
                            "Processed in {}ms",
                            proc_result.processing_time.as_millis()
                        )),
                    ),
                );

                // Call completion callbacks
                invoke_callbacks(&client.callbacks, AniDBCallbackType::Completion, |reg| {
                    let callback_fn = unsafe {
                        std::mem::transmute::<*mut c_void, AniDBCompletionCallback>(
                            reg.callback_ptr,
                        )
                    };
                    callback_fn(AniDBResult::Success, reg.user_data);
                });

                client.last_error = None;
                AniDBResult::Success
            }
            Err(e) => {
                let error_msg = e.to_string();
                client.last_error = Some(error_msg.clone());
                let error_result = error_to_result(&e);

                // Call error callbacks
                let error_msg_cstr = CString::new(error_msg.clone()).unwrap_or_default();
                let file_path_cstr = CString::new(file_path_str.clone()).unwrap_or_default();

                invoke_callbacks(&client.callbacks, AniDBCallbackType::Error, |reg| {
                    let callback_fn = unsafe {
                        std::mem::transmute::<*mut c_void, AniDBErrorCallback>(reg.callback_ptr)
                    };
                    callback_fn(
                        error_result,
                        error_msg_cstr.as_ptr(),
                        file_path_cstr.as_ptr(),
                        reg.user_data,
                    );
                });

                // Call completion callbacks with error
                invoke_callbacks(&client.callbacks, AniDBCallbackType::Completion, |reg| {
                    let callback_fn = unsafe {
                        std::mem::transmute::<*mut c_void, AniDBCompletionCallback>(
                            reg.callback_ptr,
                        )
                    };
                    callback_fn(error_result, reg.user_data);
                });

                error_result
            }
        }
    })
}

/// Calculate hash for a file
#[unsafe(no_mangle)]
pub extern "C" fn anidb_calculate_hash(
    file_path: *const c_char,
    _algorithm: AniDBHashAlgorithm,
    hash_buffer: *mut c_char,
    buffer_size: usize,
) -> AniDBResult {
    ffi_catch_panic!({
        if !validate_c_str(file_path) || !validate_buffer(hash_buffer, buffer_size) {
            return AniDBResult::ErrorInvalidParameter;
        }
        // TODO: Implement direct hash calculation
        AniDBResult::ErrorProcessing
    })
}

/// Calculate hash for memory buffer
#[unsafe(no_mangle)]
pub extern "C" fn anidb_calculate_hash_buffer(
    data: *const u8,
    data_size: usize,
    _algorithm: AniDBHashAlgorithm,
    hash_buffer: *mut c_char,
    buffer_size: usize,
) -> AniDBResult {
    ffi_catch_panic!({
        if !validate_ptr(data) || data_size == 0 || !validate_buffer(hash_buffer, buffer_size) {
            return AniDBResult::ErrorInvalidParameter;
        }
        // TODO: Implement buffer hash calculation
        AniDBResult::ErrorProcessing
    })
}

/// Identify an anime file by hash and size
#[unsafe(no_mangle)]
pub extern "C" fn anidb_identify_file(
    handle: *mut c_void,
    ed2k_hash: *const c_char,
    _file_size: u64,
    info: *mut *mut AniDBAnimeInfo,
) -> AniDBResult {
    ffi_catch_panic!({
        if !validate_mut_ptr(handle) || !validate_c_str(ed2k_hash) || !validate_mut_ptr(info) {
            return AniDBResult::ErrorInvalidParameter;
        }
        // TODO: Implement anime identification
        AniDBResult::ErrorNetwork
    })
}

/// Get the last error message for a client
#[unsafe(no_mangle)]
pub extern "C" fn anidb_client_get_last_error(
    handle: *mut c_void,
    buffer: *mut c_char,
    buffer_size: usize,
) -> AniDBResult {
    ffi_catch_panic!({
        // Comprehensive parameter validation
        if !validate_mut_ptr(handle) || !validate_buffer(buffer, buffer_size) {
            return AniDBResult::ErrorInvalidParameter;
        }

        let handle_id = handle as usize;

        // Validate handle ID
        if handle_id == 0 || handle_id > usize::MAX / 2 {
            return AniDBResult::ErrorInvalidHandle;
        }

        let clients = match CLIENTS.read() {
            Ok(c) => c,
            Err(_) => return AniDBResult::ErrorBusy,
        };

        let client_arc = match clients.get(&handle_id) {
            Some(c) => c.clone(),
            None => return AniDBResult::ErrorInvalidHandle,
        };

        // Release read lock before acquiring client lock
        drop(clients);

        let client = match client_arc.lock() {
            Ok(c) => c,
            Err(_) => return AniDBResult::ErrorBusy,
        };

        let error_msg = client.last_error.as_deref().unwrap_or("No error");

        // Safe buffer copy with overflow prevention
        unsafe {
            let len = error_msg.len().min(buffer_size.saturating_sub(1));
            if len > 0 {
                ptr::copy_nonoverlapping(error_msg.as_ptr(), buffer as *mut u8, len);
            }
            // Always null-terminate
            *buffer.add(len) = 0;
        }

        AniDBResult::Success
    })
}
