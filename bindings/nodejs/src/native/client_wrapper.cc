#include "client_wrapper.h"
#include "async_worker.h"
#include <sstream>

Napi::FunctionReference ClientWrapper::constructor;

Napi::Object ClientWrapper::Init(Napi::Env env, Napi::Object exports) {
    Napi::Function func = DefineClass(env, "AniDBClientNative", {
        // File processing
        InstanceMethod("processFile", &ClientWrapper::ProcessFile),
        InstanceMethod("processFileAsync", &ClientWrapper::ProcessFileAsync),
        InstanceMethod("processBatch", &ClientWrapper::ProcessBatch),
        InstanceMethod("processBatchAsync", &ClientWrapper::ProcessBatchAsync),
        
        // Hash calculation
        InstanceMethod("calculateHash", &ClientWrapper::CalculateHash),
        InstanceMethod("calculateHashBuffer", &ClientWrapper::CalculateHashBuffer),
        
        // Cache management
        InstanceMethod("cacheClear", &ClientWrapper::CacheClear),
        InstanceMethod("cacheGetStats", &ClientWrapper::CacheGetStats),
        InstanceMethod("cacheCheckFile", &ClientWrapper::CacheCheckFile),
        
        // Anime identification
        InstanceMethod("identifyFile", &ClientWrapper::IdentifyFile),
        
        // Error handling
        InstanceMethod("getLastError", &ClientWrapper::GetLastError),
        
        // Callback management
        InstanceMethod("registerCallback", &ClientWrapper::RegisterCallback),
        InstanceMethod("unregisterCallback", &ClientWrapper::UnregisterCallback),
        InstanceMethod("connectEvents", &ClientWrapper::ConnectEvents),
        InstanceMethod("disconnectEvents", &ClientWrapper::DisconnectEvents),
        InstanceMethod("pollEvents", &ClientWrapper::PollEvents),
    });

    constructor = Napi::Persistent(func);
    constructor.SuppressDestruct();

    exports.Set("AniDBClientNative", func);
    return exports;
}

ClientWrapper::ClientWrapper(const Napi::CallbackInfo& info) 
    : Napi::ObjectWrap<ClientWrapper>(info), handle_(nullptr), event_connected_(false) {
    
    Napi::Env env = info.Env();
    
    if (info.Length() == 0) {
        // Create with default config
        anidb_result_t result = anidb_client_create(&handle_);
        CheckResult(env, result);
    } else if (info.Length() == 1 && info[0].IsObject()) {
        // Create with custom config
        Napi::Object config = info[0].As<Napi::Object>();
        anidb_config_t native_config = {};
        
        // Parse configuration
        if (config.Has("cacheDir") && config.Get("cacheDir").IsString()) {
            std::string cache_dir = config.Get("cacheDir").As<Napi::String>().Utf8Value();
            native_config.cache_dir = cache_dir.c_str();
        }
        
        if (config.Has("maxConcurrentFiles") && config.Get("maxConcurrentFiles").IsNumber()) {
            native_config.max_concurrent_files = config.Get("maxConcurrentFiles").As<Napi::Number>().Uint32Value();
        } else {
            native_config.max_concurrent_files = 4;
        }
        
        if (config.Has("chunkSize") && config.Get("chunkSize").IsNumber()) {
            native_config.chunk_size = config.Get("chunkSize").As<Napi::Number>().Uint32Value();
        } else {
            native_config.chunk_size = 65536; // 64KB default
        }
        
        if (config.Has("maxMemoryUsage") && config.Get("maxMemoryUsage").IsNumber()) {
            native_config.max_memory_usage = config.Get("maxMemoryUsage").As<Napi::Number>().Uint32Value();
        }
        
        if (config.Has("enableDebugLogging") && config.Get("enableDebugLogging").IsBoolean()) {
            native_config.enable_debug_logging = config.Get("enableDebugLogging").As<Napi::Boolean>().Value() ? 1 : 0;
        }
        
        if (config.Has("username") && config.Get("username").IsString()) {
            std::string username = config.Get("username").As<Napi::String>().Utf8Value();
            native_config.username = username.c_str();
        }
        
        if (config.Has("password") && config.Get("password").IsString()) {
            std::string password = config.Get("password").As<Napi::String>().Utf8Value();
            native_config.password = password.c_str();
        }
        
        anidb_result_t result = anidb_client_create_with_config(&native_config, &handle_);
        CheckResult(env, result);
    } else {
        Napi::TypeError::New(env, "Invalid arguments").ThrowAsJavaScriptException();
    }
}

ClientWrapper::~ClientWrapper() {
    if (handle_) {
        // Disconnect events if connected
        if (event_connected_) {
            anidb_event_disconnect(handle_);
        }
        
        // Unregister all callbacks
        {
            std::lock_guard<std::mutex> lock(callback_mutex_);
            for (const auto& pair : callbacks_) {
                anidb_unregister_callback(handle_, pair.first);
            }
            callbacks_.clear();
        }
        
        // Destroy the client
        anidb_client_destroy(handle_);
        handle_ = nullptr;
    }
}

Napi::Value ClientWrapper::ProcessFile(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    if (info.Length() < 2 || !info[0].IsString() || !info[1].IsObject()) {
        Napi::TypeError::New(env, "Expected (filePath: string, options: object)").ThrowAsJavaScriptException();
        return env.Null();
    }
    
    std::string file_path = info[0].As<Napi::String>().Utf8Value();
    Napi::Object options = info[1].As<Napi::Object>();
    
    // Parse options
    anidb_process_options_t process_options = {};
    std::vector<anidb_hash_algorithm_t> algorithms;
    
    if (options.Has("algorithms") && options.Get("algorithms").IsArray()) {
        Napi::Array algo_array = options.Get("algorithms").As<Napi::Array>();
        for (uint32_t i = 0; i < algo_array.Length(); i++) {
            if (algo_array.Get(i).IsNumber()) {
                algorithms.push_back(static_cast<anidb_hash_algorithm_t>(
                    algo_array.Get(i).As<Napi::Number>().Int32Value()
                ));
            }
        }
    }
    
    if (algorithms.empty()) {
        algorithms.push_back(ANIDB_HASH_ED2K); // Default to ED2K
    }
    
    process_options.algorithms = algorithms.data();
    process_options.algorithm_count = algorithms.size();
    process_options.enable_progress = options.Has("enableProgress") && 
        options.Get("enableProgress").As<Napi::Boolean>().Value() ? 1 : 0;
    process_options.verify_existing = options.Has("verifyExisting") && 
        options.Get("verifyExisting").As<Napi::Boolean>().Value() ? 1 : 0;
    
    // Process file synchronously
    anidb_file_result_t* result = nullptr;
    anidb_result_t status = anidb_process_file(handle_, file_path.c_str(), &process_options, &result);
    
    if (status != ANIDB_SUCCESS) {
        CheckResult(env, status);
        return env.Null();
    }
    
    // Convert result to JavaScript object
    Napi::Object js_result = ConvertFileResult(env, result);
    
    // Free the native result
    anidb_free_file_result(result);
    
    return js_result;
}

Napi::Value ClientWrapper::ProcessFileAsync(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    if (info.Length() < 2 || !info[0].IsString() || !info[1].IsObject()) {
        Napi::TypeError::New(env, "Expected (filePath: string, options: object)").ThrowAsJavaScriptException();
        return env.Null();
    }
    
    std::string file_path = info[0].As<Napi::String>().Utf8Value();
    Napi::Object options = info[1].As<Napi::Object>();
    
    // Create promise
    auto deferred = Napi::Promise::Deferred::New(env);
    
    // Create async worker
    auto* worker = new ProcessFileWorker(env, handle_, file_path, options, deferred);
    worker->Queue();
    
    return deferred.Promise();
}

Napi::Value ClientWrapper::ProcessBatch(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    if (info.Length() < 2 || !info[0].IsArray() || !info[1].IsObject()) {
        Napi::TypeError::New(env, "Expected (filePaths: string[], options: object)").ThrowAsJavaScriptException();
        return env.Null();
    }
    
    Napi::Array file_array = info[0].As<Napi::Array>();
    Napi::Object options = info[1].As<Napi::Object>();
    
    // Convert file paths
    std::vector<std::string> file_paths;
    std::vector<const char*> file_path_ptrs;
    
    for (uint32_t i = 0; i < file_array.Length(); i++) {
        if (file_array.Get(i).IsString()) {
            file_paths.push_back(file_array.Get(i).As<Napi::String>().Utf8Value());
        }
    }
    
    for (const auto& path : file_paths) {
        file_path_ptrs.push_back(path.c_str());
    }
    
    // Parse batch options
    anidb_batch_options_t batch_options = {};
    std::vector<anidb_hash_algorithm_t> algorithms;
    
    if (options.Has("algorithms") && options.Get("algorithms").IsArray()) {
        Napi::Array algo_array = options.Get("algorithms").As<Napi::Array>();
        for (uint32_t i = 0; i < algo_array.Length(); i++) {
            if (algo_array.Get(i).IsNumber()) {
                algorithms.push_back(static_cast<anidb_hash_algorithm_t>(
                    algo_array.Get(i).As<Napi::Number>().Int32Value()
                ));
            }
        }
    }
    
    if (algorithms.empty()) {
        algorithms.push_back(ANIDB_HASH_ED2K);
    }
    
    batch_options.algorithms = algorithms.data();
    batch_options.algorithm_count = algorithms.size();
    batch_options.max_concurrent = options.Has("maxConcurrent") ? 
        options.Get("maxConcurrent").As<Napi::Number>().Uint32Value() : 4;
    batch_options.continue_on_error = options.Has("continueOnError") && 
        options.Get("continueOnError").As<Napi::Boolean>().Value() ? 1 : 0;
    batch_options.skip_existing = options.Has("skipExisting") && 
        options.Get("skipExisting").As<Napi::Boolean>().Value() ? 1 : 0;
    
    // Process batch synchronously
    anidb_batch_result_t* result = nullptr;
    anidb_result_t status = anidb_process_batch(handle_, file_path_ptrs.data(), 
        file_path_ptrs.size(), &batch_options, &result);
    
    if (status != ANIDB_SUCCESS) {
        CheckResult(env, status);
        return env.Null();
    }
    
    // Convert result to JavaScript object
    Napi::Object js_result = ConvertBatchResult(env, result);
    
    // Free the native result
    anidb_free_batch_result(result);
    
    return js_result;
}

Napi::Value ClientWrapper::ProcessBatchAsync(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    if (info.Length() < 2 || !info[0].IsArray() || !info[1].IsObject()) {
        Napi::TypeError::New(env, "Expected (filePaths: string[], options: object)").ThrowAsJavaScriptException();
        return env.Null();
    }
    
    Napi::Array file_array = info[0].As<Napi::Array>();
    Napi::Object options = info[1].As<Napi::Object>();
    
    // Convert file paths
    std::vector<std::string> file_paths;
    for (uint32_t i = 0; i < file_array.Length(); i++) {
        if (file_array.Get(i).IsString()) {
            file_paths.push_back(file_array.Get(i).As<Napi::String>().Utf8Value());
        }
    }
    
    // Create promise
    auto deferred = Napi::Promise::Deferred::New(env);
    
    // Create async worker
    auto* worker = new ProcessBatchWorker(env, handle_, file_paths, options, deferred);
    worker->Queue();
    
    return deferred.Promise();
}

Napi::Value ClientWrapper::CalculateHash(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    if (info.Length() < 2 || !info[0].IsString() || !info[1].IsNumber()) {
        Napi::TypeError::New(env, "Expected (filePath: string, algorithm: number)").ThrowAsJavaScriptException();
        return env.Null();
    }
    
    std::string file_path = info[0].As<Napi::String>().Utf8Value();
    anidb_hash_algorithm_t algorithm = static_cast<anidb_hash_algorithm_t>(
        info[1].As<Napi::Number>().Int32Value()
    );
    
    size_t buffer_size = anidb_hash_buffer_size(algorithm);
    std::vector<char> hash_buffer(buffer_size);
    
    anidb_result_t result = anidb_calculate_hash(file_path.c_str(), algorithm, 
        hash_buffer.data(), buffer_size);
    
    if (result != ANIDB_SUCCESS) {
        CheckResult(env, result);
        return env.Null();
    }
    
    return Napi::String::New(env, hash_buffer.data());
}

Napi::Value ClientWrapper::CalculateHashBuffer(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    if (info.Length() < 2 || !info[0].IsBuffer() || !info[1].IsNumber()) {
        Napi::TypeError::New(env, "Expected (buffer: Buffer, algorithm: number)").ThrowAsJavaScriptException();
        return env.Null();
    }
    
    Napi::Buffer<uint8_t> buffer = info[0].As<Napi::Buffer<uint8_t>>();
    anidb_hash_algorithm_t algorithm = static_cast<anidb_hash_algorithm_t>(
        info[1].As<Napi::Number>().Int32Value()
    );
    
    size_t hash_buffer_size = anidb_hash_buffer_size(algorithm);
    std::vector<char> hash_buffer(hash_buffer_size);
    
    anidb_result_t result = anidb_calculate_hash_buffer(buffer.Data(), buffer.Length(),
        algorithm, hash_buffer.data(), hash_buffer_size);
    
    if (result != ANIDB_SUCCESS) {
        CheckResult(env, result);
        return env.Null();
    }
    
    return Napi::String::New(env, hash_buffer.data());
}

Napi::Value ClientWrapper::GetLastError(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    char error_buffer[1024];
    anidb_result_t result = anidb_client_get_last_error(handle_, error_buffer, sizeof(error_buffer));
    
    if (result != ANIDB_SUCCESS) {
        return Napi::String::New(env, "Failed to get last error");
    }
    
    return Napi::String::New(env, error_buffer);
}

Napi::Value ClientWrapper::CacheClear(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    anidb_result_t result = anidb_cache_clear(handle_);
    CheckResult(env, result);
    
    return env.Undefined();
}

Napi::Value ClientWrapper::CacheGetStats(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    size_t total_entries = 0;
    uint64_t cache_size_bytes = 0;
    
    anidb_result_t result = anidb_cache_get_stats(handle_, &total_entries, &cache_size_bytes);
    CheckResult(env, result);
    
    Napi::Object stats = Napi::Object::New(env);
    stats.Set("totalEntries", Napi::Number::New(env, total_entries));
    stats.Set("sizeBytes", Napi::Number::New(env, cache_size_bytes));
    
    return stats;
}

Napi::Value ClientWrapper::CacheCheckFile(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    if (info.Length() < 2 || !info[0].IsString() || !info[1].IsNumber()) {
        Napi::TypeError::New(env, "Expected (filePath: string, algorithm: number)").ThrowAsJavaScriptException();
        return env.Null();
    }
    
    std::string file_path = info[0].As<Napi::String>().Utf8Value();
    anidb_hash_algorithm_t algorithm = static_cast<anidb_hash_algorithm_t>(
        info[1].As<Napi::Number>().Int32Value()
    );
    
    int is_cached = 0;
    anidb_result_t result = anidb_cache_check_file(handle_, file_path.c_str(), algorithm, &is_cached);
    CheckResult(env, result);
    
    return Napi::Boolean::New(env, is_cached != 0);
}

Napi::Value ClientWrapper::IdentifyFile(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    if (info.Length() < 2 || !info[0].IsString() || !info[1].IsNumber()) {
        Napi::TypeError::New(env, "Expected (ed2kHash: string, fileSize: number)").ThrowAsJavaScriptException();
        return env.Null();
    }
    
    std::string ed2k_hash = info[0].As<Napi::String>().Utf8Value();
    uint64_t file_size = info[1].As<Napi::Number>().Int64Value();
    
    anidb_anime_info_t* anime_info = nullptr;
    anidb_result_t result = anidb_identify_file(handle_, ed2k_hash.c_str(), file_size, &anime_info);
    
    if (result != ANIDB_SUCCESS) {
        CheckResult(env, result);
        return env.Null();
    }
    
    Napi::Object js_info = ConvertAnimeInfo(env, anime_info);
    anidb_free_anime_info(anime_info);
    
    return js_info;
}

// Utility methods implementation
void ClientWrapper::CheckResult(Napi::Env env, anidb_result_t result) {
    if (result != ANIDB_SUCCESS) {
        const char* error_str = anidb_error_string(result);
        Napi::Error::New(env, error_str).ThrowAsJavaScriptException();
    }
}

Napi::Object ClientWrapper::ConvertFileResult(Napi::Env env, const anidb_file_result_t* result) {
    Napi::Object obj = Napi::Object::New(env);
    
    obj.Set("filePath", Napi::String::New(env, result->file_path));
    obj.Set("fileSize", Napi::Number::New(env, result->file_size));
    obj.Set("status", Napi::Number::New(env, result->status));
    obj.Set("processingTimeMs", Napi::Number::New(env, result->processing_time_ms));
    
    if (result->error_message) {
        obj.Set("error", Napi::String::New(env, result->error_message));
    }
    
    // Convert hashes
    Napi::Object hashes = Napi::Object::New(env);
    for (size_t i = 0; i < result->hash_count; i++) {
        const anidb_hash_result_t* hash = &result->hashes[i];
        const char* algo_name = anidb_hash_algorithm_name(hash->algorithm);
        hashes.Set(algo_name, Napi::String::New(env, hash->hash_value));
    }
    obj.Set("hashes", hashes);
    
    return obj;
}

Napi::Object ClientWrapper::ConvertBatchResult(Napi::Env env, const anidb_batch_result_t* result) {
    Napi::Object obj = Napi::Object::New(env);
    
    obj.Set("totalFiles", Napi::Number::New(env, result->total_files));
    obj.Set("successfulFiles", Napi::Number::New(env, result->successful_files));
    obj.Set("failedFiles", Napi::Number::New(env, result->failed_files));
    obj.Set("totalTimeMs", Napi::Number::New(env, result->total_time_ms));
    
    // Convert individual results
    Napi::Array results = Napi::Array::New(env, result->total_files);
    for (size_t i = 0; i < result->total_files; i++) {
        results.Set(i, ConvertFileResult(env, &result->results[i]));
    }
    obj.Set("results", results);
    
    return obj;
}

Napi::Object ClientWrapper::ConvertAnimeInfo(Napi::Env env, const anidb_anime_info_t* info) {
    Napi::Object obj = Napi::Object::New(env);
    
    obj.Set("animeId", Napi::Number::New(env, info->anime_id));
    obj.Set("episodeId", Napi::Number::New(env, info->episode_id));
    obj.Set("title", Napi::String::New(env, info->title));
    obj.Set("episodeNumber", Napi::Number::New(env, info->episode_number));
    obj.Set("confidence", Napi::Number::New(env, info->confidence));
    
    std::string source;
    switch (info->source) {
        case 0: source = "anidb"; break;
        case 1: source = "cache"; break;
        case 2: source = "filename"; break;
        default: source = "unknown"; break;
    }
    obj.Set("source", Napi::String::New(env, source));
    
    return obj;
}

Napi::Value ClientWrapper::RegisterCallback(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    if (info.Length() < 3 || !info[0].IsNumber() || !info[1].IsFunction()) {
        Napi::TypeError::New(env, "Expected (type: number, callback: function, userData?: any)").ThrowAsJavaScriptException();
        return env.Null();
    }
    
    anidb_callback_type_t type = static_cast<anidb_callback_type_t>(info[0].As<Napi::Number>().Int32Value());
    Napi::Function callback = info[1].As<Napi::Function>();
    
    // Create callback data
    auto callbackData = std::make_unique<CallbackData>();
    callbackData->user_data = info.Length() >= 3 ? new Napi::Reference<Napi::Value>(Napi::Persistent(info[2])) : nullptr;
    
    // Create thread-safe function based on callback type
    std::string tsfnName;
    switch (type) {
        case ANIDB_CALLBACK_PROGRESS:
            tsfnName = "ProgressCallback";
            break;
        case ANIDB_CALLBACK_ERROR:
            tsfnName = "ErrorCallback";
            break;
        case ANIDB_CALLBACK_COMPLETION:
            tsfnName = "CompletionCallback";
            break;
        default:
            Napi::Error::New(env, "Invalid callback type").ThrowAsJavaScriptException();
            return env.Null();
    }
    
    callbackData->tsfn = Napi::ThreadSafeFunction::New(
        env,
        callback,
        tsfnName,
        0,
        1
    );
    
    // Register with native library
    void* callback_ptr = nullptr;
    switch (type) {
        case ANIDB_CALLBACK_PROGRESS:
            callback_ptr = reinterpret_cast<void*>(&ClientWrapper::ProgressCallbackHandler);
            break;
        case ANIDB_CALLBACK_ERROR:
            callback_ptr = reinterpret_cast<void*>(&ClientWrapper::ErrorCallbackHandler);
            break;
        case ANIDB_CALLBACK_COMPLETION:
            callback_ptr = reinterpret_cast<void*>(&ClientWrapper::CompletionCallbackHandler);
            break;
    }
    
    uint64_t callback_id = anidb_register_callback(handle_, type, callback_ptr, callbackData.get());
    
    if (callback_id == 0) {
        Napi::Error::New(env, "Failed to register callback").ThrowAsJavaScriptException();
        return env.Null();
    }
    
    // Store callback data
    {
        std::lock_guard<std::mutex> lock(callback_mutex_);
        callbacks_[callback_id] = std::move(callbackData);
    }
    
    return Napi::Number::New(env, callback_id);
}

Napi::Value ClientWrapper::UnregisterCallback(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    if (info.Length() < 1 || !info[0].IsNumber()) {
        Napi::TypeError::New(env, "Expected callback ID").ThrowAsJavaScriptException();
        return env.Null();
    }
    
    uint64_t callback_id = info[0].As<Napi::Number>().Int64Value();
    
    // Unregister from native
    anidb_result_t result = anidb_unregister_callback(handle_, callback_id);
    CheckResult(env, result);
    
    // Remove from our map
    {
        std::lock_guard<std::mutex> lock(callback_mutex_);
        auto it = callbacks_.find(callback_id);
        if (it != callbacks_.end()) {
            if (it->second->user_data) {
                delete static_cast<Napi::Reference<Napi::Value>*>(it->second->user_data);
            }
            callbacks_.erase(it);
        }
    }
    
    return env.Undefined();
}

Napi::Value ClientWrapper::ConnectEvents(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    if (info.Length() < 1 || !info[0].IsFunction()) {
        Napi::TypeError::New(env, "Expected event callback function").ThrowAsJavaScriptException();
        return env.Null();
    }
    
    // Disconnect existing events
    if (event_connected_) {
        anidb_event_disconnect(handle_);
        event_connected_ = false;
    }
    
    // Create thread-safe function for events
    event_callback_ = Napi::ThreadSafeFunction::New(
        env,
        info[0].As<Napi::Function>(),
        "EventCallback",
        0,
        1
    );
    
    // Connect to native events
    anidb_result_t result = anidb_event_connect(handle_, &ClientWrapper::EventCallbackHandler, this);
    CheckResult(env, result);
    
    event_connected_ = true;
    return env.Undefined();
}

Napi::Value ClientWrapper::DisconnectEvents(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    if (event_connected_) {
        anidb_result_t result = anidb_event_disconnect(handle_);
        CheckResult(env, result);
        event_connected_ = false;
    }
    
    return env.Undefined();
}

Napi::Value ClientWrapper::PollEvents(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    
    const size_t max_events = 100;
    std::vector<anidb_event_t> events(max_events);
    size_t event_count = 0;
    
    anidb_result_t result = anidb_event_poll(handle_, events.data(), max_events, &event_count);
    CheckResult(env, result);
    
    Napi::Array js_events = Napi::Array::New(env, event_count);
    for (size_t i = 0; i < event_count; i++) {
        js_events.Set(i, ConvertEvent(env, &events[i]));
    }
    
    return js_events;
}

Napi::Object ClientWrapper::ConvertEvent(Napi::Env env, const anidb_event_t* event) {
    Napi::Object obj = Napi::Object::New(env);
    
    obj.Set("type", Napi::Number::New(env, event->type));
    obj.Set("timestamp", Napi::Number::New(env, event->timestamp));
    
    if (event->context) {
        obj.Set("context", Napi::String::New(env, event->context));
    }
    
    // Convert event data based on type
    Napi::Object data = Napi::Object::New(env);
    
    switch (event->type) {
        case ANIDB_EVENT_FILE_START:
        case ANIDB_EVENT_FILE_COMPLETE: {
            Napi::Object file = Napi::Object::New(env);
            if (event->data.file.file_path) {
                file.Set("filePath", Napi::String::New(env, event->data.file.file_path));
            }
            file.Set("fileSize", Napi::Number::New(env, event->data.file.file_size));
            data.Set("file", file);
            break;
        }
        
        case ANIDB_EVENT_HASH_START:
        case ANIDB_EVENT_HASH_COMPLETE: {
            Napi::Object hash = Napi::Object::New(env);
            hash.Set("algorithm", Napi::Number::New(env, event->data.hash.algorithm));
            if (event->data.hash.hash_value) {
                hash.Set("hashValue", Napi::String::New(env, event->data.hash.hash_value));
            }
            data.Set("hash", hash);
            break;
        }
        
        case ANIDB_EVENT_CACHE_HIT:
        case ANIDB_EVENT_CACHE_MISS: {
            Napi::Object cache = Napi::Object::New(env);
            if (event->data.cache.file_path) {
                cache.Set("filePath", Napi::String::New(env, event->data.cache.file_path));
            }
            cache.Set("algorithm", Napi::Number::New(env, event->data.cache.algorithm));
            data.Set("cache", cache);
            break;
        }
        
        case ANIDB_EVENT_NETWORK_START:
        case ANIDB_EVENT_NETWORK_COMPLETE: {
            Napi::Object network = Napi::Object::New(env);
            if (event->data.network.endpoint) {
                network.Set("endpoint", Napi::String::New(env, event->data.network.endpoint));
            }
            network.Set("statusCode", Napi::Number::New(env, event->data.network.status_code));
            data.Set("network", network);
            break;
        }
        
        case ANIDB_EVENT_MEMORY_WARNING: {
            Napi::Object memory = Napi::Object::New(env);
            memory.Set("currentUsage", Napi::Number::New(env, event->data.memory.current_usage));
            memory.Set("maxUsage", Napi::Number::New(env, event->data.memory.max_usage));
            data.Set("memory", memory);
            break;
        }
    }
    
    obj.Set("data", data);
    return obj;
}

// Static callback handlers
void ClientWrapper::ProgressCallbackHandler(float percentage, uint64_t bytes_processed,
                                          uint64_t total_bytes, void* user_data) {
    auto* callbackData = static_cast<CallbackData*>(user_data);
    if (!callbackData || !callbackData->tsfn) return;
    
    auto callback = [](Napi::Env env, Napi::Function jsCallback, float* data) {
        Napi::Object progress = Napi::Object::New(env);
        progress.Set("percentage", Napi::Number::New(env, data[0]));
        progress.Set("bytesProcessed", Napi::Number::New(env, data[1]));
        progress.Set("totalBytes", Napi::Number::New(env, data[2]));
        
        jsCallback.Call({progress});
        delete[] data;
    };
    
    float* data = new float[3];
    data[0] = percentage;
    data[1] = static_cast<float>(bytes_processed);
    data[2] = static_cast<float>(total_bytes);
    
    callbackData->tsfn.BlockingCall(data, callback);
}

void ClientWrapper::ErrorCallbackHandler(anidb_result_t error_code, const char* error_message,
                                       const char* file_path, void* user_data) {
    auto* callbackData = static_cast<CallbackData*>(user_data);
    if (!callbackData || !callbackData->tsfn) return;
    
    struct ErrorData {
        anidb_result_t code;
        std::string message;
        std::string path;
    };
    
    auto callback = [](Napi::Env env, Napi::Function jsCallback, ErrorData* data) {
        Napi::Object error = Napi::Object::New(env);
        error.Set("code", Napi::Number::New(env, data->code));
        error.Set("message", Napi::String::New(env, data->message));
        if (!data->path.empty()) {
            error.Set("filePath", Napi::String::New(env, data->path));
        }
        
        jsCallback.Call({error});
        delete data;
    };
    
    auto* data = new ErrorData{
        error_code,
        error_message ? error_message : "",
        file_path ? file_path : ""
    };
    
    callbackData->tsfn.BlockingCall(data, callback);
}

void ClientWrapper::CompletionCallbackHandler(anidb_result_t result, void* user_data) {
    auto* callbackData = static_cast<CallbackData*>(user_data);
    if (!callbackData || !callbackData->tsfn) return;
    
    auto callback = [](Napi::Env env, Napi::Function jsCallback, anidb_result_t* data) {
        jsCallback.Call({Napi::Number::New(env, *data)});
        delete data;
    };
    
    auto* data = new anidb_result_t(result);
    callbackData->tsfn.BlockingCall(data, callback);
}

void ClientWrapper::EventCallbackHandler(const anidb_event_t* event, void* user_data) {
    auto* wrapper = static_cast<ClientWrapper*>(user_data);
    if (!wrapper || !wrapper->event_callback_) return;
    
    // Copy event data for thread safety
    auto* eventCopy = new anidb_event_t(*event);
    
    auto callback = [wrapper](Napi::Env env, Napi::Function jsCallback, anidb_event_t* data) {
        Napi::Object jsEvent = ClientWrapper::ConvertEvent(env, data);
        jsCallback.Call({jsEvent});
        delete data;
    };
    
    wrapper->event_callback_.BlockingCall(eventCopy, callback);
}