#include <napi.h>
#include "client_wrapper.h"
#include "async_worker.h"
#include "stream_worker.h"

// Module initialization
Napi::Object Init(Napi::Env env, Napi::Object exports) {
    // Initialize the library
    uint32_t abi_version = 1; // ANIDB_ABI_VERSION
    anidb_result_t init_result = anidb_init(abi_version);
    if (init_result != ANIDB_SUCCESS) {
        Napi::Error::New(env, "Failed to initialize AniDB library").ThrowAsJavaScriptException();
        return exports;
    }

    // Export the ClientWrapper class
    ClientWrapper::Init(env, exports);
    
    // Export version information
    exports.Set("version", Napi::String::New(env, anidb_get_version()));
    exports.Set("abiVersion", Napi::Number::New(env, anidb_get_abi_version()));
    
    // Export hash algorithm constants
    Napi::Object hashAlgorithms = Napi::Object::New(env);
    hashAlgorithms.Set("ED2K", Napi::Number::New(env, ANIDB_HASH_ED2K));
    hashAlgorithms.Set("CRC32", Napi::Number::New(env, ANIDB_HASH_CRC32));
    hashAlgorithms.Set("MD5", Napi::Number::New(env, ANIDB_HASH_MD5));
    hashAlgorithms.Set("SHA1", Napi::Number::New(env, ANIDB_HASH_SHA1));
    hashAlgorithms.Set("TTH", Napi::Number::New(env, ANIDB_HASH_TTH));
    exports.Set("HashAlgorithm", hashAlgorithms);
    
    // Export status constants
    Napi::Object status = Napi::Object::New(env);
    status.Set("PENDING", Napi::Number::New(env, ANIDB_STATUS_PENDING));
    status.Set("PROCESSING", Napi::Number::New(env, ANIDB_STATUS_PROCESSING));
    status.Set("COMPLETED", Napi::Number::New(env, ANIDB_STATUS_COMPLETED));
    status.Set("FAILED", Napi::Number::New(env, ANIDB_STATUS_FAILED));
    status.Set("CANCELLED", Napi::Number::New(env, ANIDB_STATUS_CANCELLED));
    exports.Set("Status", status);
    
    // Export error codes
    Napi::Object errors = Napi::Object::New(env);
    errors.Set("SUCCESS", Napi::Number::New(env, ANIDB_SUCCESS));
    errors.Set("INVALID_HANDLE", Napi::Number::New(env, ANIDB_ERROR_INVALID_HANDLE));
    errors.Set("INVALID_PARAMETER", Napi::Number::New(env, ANIDB_ERROR_INVALID_PARAMETER));
    errors.Set("FILE_NOT_FOUND", Napi::Number::New(env, ANIDB_ERROR_FILE_NOT_FOUND));
    errors.Set("PROCESSING", Napi::Number::New(env, ANIDB_ERROR_PROCESSING));
    errors.Set("OUT_OF_MEMORY", Napi::Number::New(env, ANIDB_ERROR_OUT_OF_MEMORY));
    errors.Set("IO", Napi::Number::New(env, ANIDB_ERROR_IO));
    errors.Set("NETWORK", Napi::Number::New(env, ANIDB_ERROR_NETWORK));
    errors.Set("CANCELLED", Napi::Number::New(env, ANIDB_ERROR_CANCELLED));
    errors.Set("INVALID_UTF8", Napi::Number::New(env, ANIDB_ERROR_INVALID_UTF8));
    errors.Set("VERSION_MISMATCH", Napi::Number::New(env, ANIDB_ERROR_VERSION_MISMATCH));
    errors.Set("TIMEOUT", Napi::Number::New(env, ANIDB_ERROR_TIMEOUT));
    errors.Set("PERMISSION_DENIED", Napi::Number::New(env, ANIDB_ERROR_PERMISSION_DENIED));
    errors.Set("CACHE", Napi::Number::New(env, ANIDB_ERROR_CACHE));
    errors.Set("BUSY", Napi::Number::New(env, ANIDB_ERROR_BUSY));
    errors.Set("UNKNOWN", Napi::Number::New(env, ANIDB_ERROR_UNKNOWN));
    exports.Set("ErrorCode", errors);
    
    // Export utility functions
    exports.Set("errorString", Napi::Function::New(env, [](const Napi::CallbackInfo& info) {
        if (info.Length() < 1 || !info[0].IsNumber()) {
            Napi::TypeError::New(info.Env(), "Error code must be a number").ThrowAsJavaScriptException();
            return info.Env().Null();
        }
        
        anidb_result_t error = static_cast<anidb_result_t>(info[0].As<Napi::Number>().Int32Value());
        const char* error_str = anidb_error_string(error);
        return Napi::String::New(info.Env(), error_str);
    }));
    
    exports.Set("hashAlgorithmName", Napi::Function::New(env, [](const Napi::CallbackInfo& info) {
        if (info.Length() < 1 || !info[0].IsNumber()) {
            Napi::TypeError::New(info.Env(), "Algorithm must be a number").ThrowAsJavaScriptException();
            return info.Env().Null();
        }
        
        anidb_hash_algorithm_t algo = static_cast<anidb_hash_algorithm_t>(info[0].As<Napi::Number>().Int32Value());
        const char* algo_name = anidb_hash_algorithm_name(algo);
        return Napi::String::New(info.Env(), algo_name);
    }));
    
    exports.Set("hashBufferSize", Napi::Function::New(env, [](const Napi::CallbackInfo& info) {
        if (info.Length() < 1 || !info[0].IsNumber()) {
            Napi::TypeError::New(info.Env(), "Algorithm must be a number").ThrowAsJavaScriptException();
            return info.Env().Null();
        }
        
        anidb_hash_algorithm_t algo = static_cast<anidb_hash_algorithm_t>(info[0].As<Napi::Number>().Int32Value());
        size_t size = anidb_hash_buffer_size(algo);
        return Napi::Number::New(info.Env(), size);
    }));
    
    // Set up cleanup on process exit
    env.SetInstanceData(nullptr, [](Napi::Env env, void* data) {
        anidb_cleanup();
    });
    
    return exports;
}

NODE_API_MODULE(anidb_client, Init)