#ifndef ASYNC_WORKER_H
#define ASYNC_WORKER_H

#include <napi.h>
#include "../../../anidb_client_core/include/anidb.h"
#include <string>
#include <vector>

// Base class for async operations
class AniDBAsyncWorker : public Napi::AsyncWorker {
protected:
    anidb_client_handle_t handle_;
    anidb_result_t result_;
    
public:
    AniDBAsyncWorker(Napi::Env env, anidb_client_handle_t handle,
                     Napi::Promise::Deferred deferred)
        : Napi::AsyncWorker(env), handle_(handle), result_(ANIDB_SUCCESS),
          deferred_(deferred) {}
    
    void OnError(const Napi::Error& error) override {
        deferred_.Reject(error.Value());
    }
    
protected:
    Napi::Promise::Deferred deferred_;
};

// Async worker for processing a single file
class ProcessFileWorker : public AniDBAsyncWorker {
private:
    std::string file_path_;
    std::vector<anidb_hash_algorithm_t> algorithms_;
    bool enable_progress_;
    bool verify_existing_;
    anidb_file_result_t* file_result_;
    
public:
    ProcessFileWorker(Napi::Env env, anidb_client_handle_t handle,
                      const std::string& file_path, const Napi::Object& options,
                      Napi::Promise::Deferred deferred);
    
    ~ProcessFileWorker();
    
    void Execute() override;
    void OnOK() override;
};

// Async worker for batch processing
class ProcessBatchWorker : public AniDBAsyncWorker {
private:
    std::vector<std::string> file_paths_;
    std::vector<anidb_hash_algorithm_t> algorithms_;
    size_t max_concurrent_;
    bool continue_on_error_;
    bool skip_existing_;
    anidb_batch_result_t* batch_result_;
    
public:
    ProcessBatchWorker(Napi::Env env, anidb_client_handle_t handle,
                       const std::vector<std::string>& file_paths,
                       const Napi::Object& options,
                       Napi::Promise::Deferred deferred);
    
    ~ProcessBatchWorker();
    
    void Execute() override;
    void OnOK() override;
};

// Async worker for hash calculation
class CalculateHashWorker : public AniDBAsyncWorker {
private:
    std::string file_path_;
    anidb_hash_algorithm_t algorithm_;
    std::string hash_result_;
    
public:
    CalculateHashWorker(Napi::Env env, const std::string& file_path,
                        anidb_hash_algorithm_t algorithm,
                        Napi::Promise::Deferred deferred);
    
    void Execute() override;
    void OnOK() override;
};

// Async worker for anime identification
class IdentifyFileWorker : public AniDBAsyncWorker {
private:
    std::string ed2k_hash_;
    uint64_t file_size_;
    anidb_anime_info_t* anime_info_;
    
public:
    IdentifyFileWorker(Napi::Env env, anidb_client_handle_t handle,
                       const std::string& ed2k_hash, uint64_t file_size,
                       Napi::Promise::Deferred deferred);
    
    ~IdentifyFileWorker();
    
    void Execute() override;
    void OnOK() override;
};

#endif // ASYNC_WORKER_H