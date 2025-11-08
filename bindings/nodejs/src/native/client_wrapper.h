#ifndef CLIENT_WRAPPER_H
#define CLIENT_WRAPPER_H

#include <napi.h>
#include "../../../anidb_client_core/include/anidb.h"
#include <memory>
#include <map>
#include <mutex>

class ClientWrapper : public Napi::ObjectWrap<ClientWrapper> {
public:
    static Napi::Object Init(Napi::Env env, Napi::Object exports);
    ClientWrapper(const Napi::CallbackInfo& info);
    ~ClientWrapper();

private:
    static Napi::FunctionReference constructor;
    
    // Native handle
    anidb_client_handle_t handle_;
    
    // Callback management
    struct CallbackData {
        Napi::ThreadSafeFunction tsfn;
        void* user_data;
    };
    
    std::map<uint64_t, std::unique_ptr<CallbackData>> callbacks_;
    std::mutex callback_mutex_;
    
    // Event callback
    Napi::ThreadSafeFunction event_callback_;
    bool event_connected_;
    
    // Methods
    Napi::Value ProcessFile(const Napi::CallbackInfo& info);
    Napi::Value ProcessFileAsync(const Napi::CallbackInfo& info);
    Napi::Value ProcessBatch(const Napi::CallbackInfo& info);
    Napi::Value ProcessBatchAsync(const Napi::CallbackInfo& info);
    Napi::Value CalculateHash(const Napi::CallbackInfo& info);
    Napi::Value CalculateHashBuffer(const Napi::CallbackInfo& info);
    Napi::Value GetLastError(const Napi::CallbackInfo& info);
    
    // Cache methods
    Napi::Value CacheClear(const Napi::CallbackInfo& info);
    Napi::Value CacheGetStats(const Napi::CallbackInfo& info);
    Napi::Value CacheCheckFile(const Napi::CallbackInfo& info);
    
    // Anime identification
    Napi::Value IdentifyFile(const Napi::CallbackInfo& info);
    
    // Callback management
    Napi::Value RegisterCallback(const Napi::CallbackInfo& info);
    Napi::Value UnregisterCallback(const Napi::CallbackInfo& info);
    Napi::Value ConnectEvents(const Napi::CallbackInfo& info);
    Napi::Value DisconnectEvents(const Napi::CallbackInfo& info);
    Napi::Value PollEvents(const Napi::CallbackInfo& info);
    
    // Utility methods
    static void CheckResult(Napi::Env env, anidb_result_t result);
    static Napi::Object ConvertFileResult(Napi::Env env, const anidb_file_result_t* result);
    static Napi::Object ConvertBatchResult(Napi::Env env, const anidb_batch_result_t* result);
    static Napi::Object ConvertAnimeInfo(Napi::Env env, const anidb_anime_info_t* info);
    static Napi::Object ConvertEvent(Napi::Env env, const anidb_event_t* event);
    
    // Callback handlers
    static void ProgressCallbackHandler(float percentage, uint64_t bytes_processed, 
                                      uint64_t total_bytes, void* user_data);
    static void ErrorCallbackHandler(anidb_result_t error_code, const char* error_message,
                                   const char* file_path, void* user_data);
    static void CompletionCallbackHandler(anidb_result_t result, void* user_data);
    static void EventCallbackHandler(const anidb_event_t* event, void* user_data);
};

#endif // CLIENT_WRAPPER_H