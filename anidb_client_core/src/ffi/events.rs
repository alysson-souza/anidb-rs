//! Event system and queue management for FFI
//!
//! This module manages the event system including event creation,
//! queueing, callbacks, and event polling functionality.

use crate::ffi::handles::{CLIENTS, ClientState, EventEntry};
use crate::ffi::helpers::{get_timestamp_ms, validate_mut_ptr};
use crate::ffi::types::{
    AniDBEvent, AniDBEventCallback, AniDBEventData, AniDBEventType, AniDBHashAlgorithm,
    AniDBResult, FileEventData, HashEventData, MemoryEventData,
};
use crate::ffi_catch_panic;
use std::ffi::{CString, c_void};
use std::ptr;
use tokio::sync::mpsc;

/// Create a file event
pub(crate) fn create_file_event(
    event_type: AniDBEventType,
    file_path: &str,
    file_size: u64,
    context: Option<&str>,
) -> EventEntry {
    let file_path_cstring = CString::new(file_path).ok();
    let context_cstring = context.and_then(|s| CString::new(s).ok());

    let event = AniDBEvent {
        event_type,
        timestamp: get_timestamp_ms(),
        data: AniDBEventData {
            file: FileEventData {
                file_path: file_path_cstring
                    .as_ref()
                    .map(|s| s.as_ptr())
                    .unwrap_or(ptr::null()),
                file_size,
            },
        },
        context: context_cstring
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(ptr::null()),
    };

    EventEntry {
        event,
        file_path: file_path_cstring,
        hash_value: None,
        endpoint: None,
        context: context_cstring,
    }
}

/// Create a hash event
#[allow(dead_code)]
pub(crate) fn create_hash_event(
    event_type: AniDBEventType,
    algorithm: AniDBHashAlgorithm,
    hash_value: Option<&str>,
    context: Option<&str>,
) -> EventEntry {
    let hash_value_cstring = hash_value.and_then(|s| CString::new(s).ok());
    let context_cstring = context.and_then(|s| CString::new(s).ok());

    let event = AniDBEvent {
        event_type,
        timestamp: get_timestamp_ms(),
        data: AniDBEventData {
            hash: HashEventData {
                algorithm,
                hash_value: hash_value_cstring
                    .as_ref()
                    .map(|s| s.as_ptr())
                    .unwrap_or(ptr::null()),
            },
        },
        context: context_cstring
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(ptr::null()),
    };

    EventEntry {
        event,
        file_path: None,
        hash_value: hash_value_cstring,
        endpoint: None,
        context: context_cstring,
    }
}

/// Create a memory event
pub(crate) fn create_memory_event(
    current_usage: u64,
    max_usage: u64,
    context: Option<&str>,
) -> EventEntry {
    let context_cstring = context.and_then(|s| CString::new(s).ok());

    let event = AniDBEvent {
        event_type: AniDBEventType::MemoryWarning,
        timestamp: get_timestamp_ms(),
        data: AniDBEventData {
            memory: MemoryEventData {
                current_usage,
                max_usage,
            },
        },
        context: context_cstring
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(ptr::null()),
    };

    EventEntry {
        event,
        file_path: None,
        hash_value: None,
        endpoint: None,
        context: context_cstring,
    }
}

/// Send event to event queue and callback
pub(crate) fn send_event(client: &ClientState, event_entry: EventEntry) {
    // Clone for the queue
    let queue_entry = EventEntry {
        event: event_entry.event.clone(),
        file_path: event_entry.file_path.clone(),
        hash_value: event_entry.hash_value.clone(),
        endpoint: event_entry.endpoint.clone(),
        context: event_entry.context.clone(),
    };

    // Add to queue
    if let Ok(mut queue) = client.event_queue.lock() {
        // Limit queue size to prevent unbounded growth
        if queue.len() < 10000 {
            queue.push_back(queue_entry);
        }
    }

    // Send to event thread if connected
    if let Ok(sender_opt) = client.event_sender.lock()
        && let Some(sender) = sender_opt.as_ref()
    {
        let _ = sender.send(event_entry);
    }
}

/// Connect to the event system for receiving events
#[unsafe(no_mangle)]
pub extern "C" fn anidb_event_connect(
    handle: *mut c_void,
    callback: AniDBEventCallback,
    user_data: *mut c_void,
) -> AniDBResult {
    ffi_catch_panic!({
        if !validate_mut_ptr(handle) {
            return AniDBResult::ErrorInvalidParameter;
        }

        let handle_id = handle as usize;
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

        drop(clients);

        let client = match client_arc.lock() {
            Ok(c) => c,
            Err(_) => return AniDBResult::ErrorBusy,
        };

        // Stop existing event thread if any
        if let Ok(mut thread_handle) = client.event_thread_handle.lock()
            && let Some(handle) = thread_handle.take()
        {
            // Signal thread to stop by dropping sender
            if let Ok(mut sender) = client.event_sender.lock() {
                sender.take();
            }
            let _ = handle.join();
        }

        // Set new callback (convert user_data to usize for thread safety)
        if let Ok(mut event_callback) = client.event_callback.lock() {
            *event_callback = Some((callback, user_data as usize));
        } else {
            return AniDBResult::ErrorBusy;
        }

        // Create new event channel
        let (tx, mut rx) = mpsc::unbounded_channel::<EventEntry>();

        // Store sender
        if let Ok(mut sender) = client.event_sender.lock() {
            *sender = Some(tx);
        }

        // Spawn event thread
        let event_callback_arc = client.event_callback.clone();
        let thread_handle = std::thread::spawn(move || {
            while let Some(event_entry) = rx.blocking_recv() {
                if let Ok(callback_opt) = event_callback_arc.lock()
                    && let Some((callback_fn, user_data_usize)) = *callback_opt
                {
                    // Call the callback with the event
                    // Convert usize back to pointer for the callback
                    let event_ptr = &event_entry.event as *const AniDBEvent;
                    let user_data_ptr = user_data_usize as *mut c_void;
                    callback_fn(event_ptr, user_data_ptr);
                }
            }
        });

        // Store thread handle
        if let Ok(mut thread_handle_guard) = client.event_thread_handle.lock() {
            *thread_handle_guard = Some(thread_handle);
        }

        AniDBResult::Success
    })
}

/// Disconnect from the event system
#[unsafe(no_mangle)]
pub extern "C" fn anidb_event_disconnect(handle: *mut c_void) -> AniDBResult {
    ffi_catch_panic!({
        if !validate_mut_ptr(handle) {
            return AniDBResult::ErrorInvalidParameter;
        }

        let handle_id = handle as usize;
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

        drop(clients);

        let client = match client_arc.lock() {
            Ok(c) => c,
            Err(_) => return AniDBResult::ErrorBusy,
        };

        // Stop event thread
        if let Ok(mut thread_handle) = client.event_thread_handle.lock()
            && let Some(handle) = thread_handle.take()
        {
            // Signal thread to stop by dropping sender
            if let Ok(mut sender) = client.event_sender.lock() {
                sender.take();
            }
            let _ = handle.join();
        }

        // Clear callback
        if let Ok(mut event_callback) = client.event_callback.lock() {
            *event_callback = None;
        }

        // Clear event queue
        if let Ok(mut queue) = client.event_queue.lock() {
            queue.clear();
        }

        AniDBResult::Success
    })
}

/// Poll for events without callback
#[unsafe(no_mangle)]
pub extern "C" fn anidb_event_poll(
    handle: *mut c_void,
    events: *mut AniDBEvent,
    max_events: usize,
    event_count: *mut usize,
) -> AniDBResult {
    ffi_catch_panic!({
        if !validate_mut_ptr(handle)
            || !validate_mut_ptr(events)
            || !validate_mut_ptr(event_count)
            || max_events == 0
        {
            return AniDBResult::ErrorInvalidParameter;
        }

        let handle_id = handle as usize;
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

        drop(clients);

        let client = match client_arc.lock() {
            Ok(c) => c,
            Err(_) => return AniDBResult::ErrorBusy,
        };

        let mut count = 0;
        if let Ok(mut queue) = client.event_queue.lock() {
            let events_slice = unsafe { std::slice::from_raw_parts_mut(events, max_events) };

            while count < max_events && !queue.is_empty() {
                if let Some(event_entry) = queue.pop_front() {
                    events_slice[count] = event_entry.event;
                    count += 1;
                }
            }
        }

        unsafe {
            *event_count = count;
        }

        AniDBResult::Success
    })
}
