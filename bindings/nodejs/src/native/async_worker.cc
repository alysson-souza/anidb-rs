#include "async_worker.h"
#include "client_wrapper.h"

// ProcessFileWorker implementation
ProcessFileWorker::ProcessFileWorker(Napi::Env env, anidb_client_handle_t handle,
                                   const std::string& file_path, const Napi::Object& options,
                                   Napi::Promise::Deferred deferred)
    : AniDBAsyncWorker(env, handle, deferred), file_path_(file_path), 
      enable_progress_(false), verify_existing_(false), file_result_(nullptr) {
    
    // Parse options
    if (options.Has("algorithms") && options.Get("algorithms").IsArray()) {
        Napi::Array algo_array = options.Get("algorithms").As<Napi::Array>();
        for (uint32_t i = 0; i < algo_array.Length(); i++) {
            if (algo_array.Get(i).IsNumber()) {
                algorithms_.push_back(static_cast<anidb_hash_algorithm_t>(
                    algo_array.Get(i).As<Napi::Number>().Int32Value()
                ));
            }
        }
    }
    
    if (algorithms_.empty()) {
        algorithms_.push_back(ANIDB_HASH_ED2K);
    }
    
    enable_progress_ = options.Has("enableProgress") && 
        options.Get("enableProgress").As<Napi::Boolean>().Value();
    verify_existing_ = options.Has("verifyExisting") && 
        options.Get("verifyExisting").As<Napi::Boolean>().Value();
}

ProcessFileWorker::~ProcessFileWorker() {
    if (file_result_) {
        anidb_free_file_result(file_result_);
    }
}

void ProcessFileWorker::Execute() {
    anidb_process_options_t options = {};
    options.algorithms = algorithms_.data();
    options.algorithm_count = algorithms_.size();
    options.enable_progress = enable_progress_ ? 1 : 0;
    options.verify_existing = verify_existing_ ? 1 : 0;
    
    result_ = anidb_process_file(handle_, file_path_.c_str(), &options, &file_result_);
}

void ProcessFileWorker::OnOK() {
    Napi::Env env = Env();
    
    if (result_ != ANIDB_SUCCESS) {
        deferred_.Reject(Napi::Error::New(env, anidb_error_string(result_)).Value());
        return;
    }
    
    Napi::Object js_result = ClientWrapper::ConvertFileResult(env, file_result_);
    deferred_.Resolve(js_result);
}

// ProcessBatchWorker implementation
ProcessBatchWorker::ProcessBatchWorker(Napi::Env env, anidb_client_handle_t handle,
                                     const std::vector<std::string>& file_paths,
                                     const Napi::Object& options,
                                     Napi::Promise::Deferred deferred)
    : AniDBAsyncWorker(env, handle, deferred), file_paths_(file_paths),
      max_concurrent_(4), continue_on_error_(false), skip_existing_(false),
      batch_result_(nullptr) {
    
    // Parse options
    if (options.Has("algorithms") && options.Get("algorithms").IsArray()) {
        Napi::Array algo_array = options.Get("algorithms").As<Napi::Array>();
        for (uint32_t i = 0; i < algo_array.Length(); i++) {
            if (algo_array.Get(i).IsNumber()) {
                algorithms_.push_back(static_cast<anidb_hash_algorithm_t>(
                    algo_array.Get(i).As<Napi::Number>().Int32Value()
                ));
            }
        }
    }
    
    if (algorithms_.empty()) {
        algorithms_.push_back(ANIDB_HASH_ED2K);
    }
    
    if (options.Has("maxConcurrent")) {
        max_concurrent_ = options.Get("maxConcurrent").As<Napi::Number>().Uint32Value();
    }
    
    continue_on_error_ = options.Has("continueOnError") && 
        options.Get("continueOnError").As<Napi::Boolean>().Value();
    skip_existing_ = options.Has("skipExisting") && 
        options.Get("skipExisting").As<Napi::Boolean>().Value();
}

ProcessBatchWorker::~ProcessBatchWorker() {
    if (batch_result_) {
        anidb_free_batch_result(batch_result_);
    }
}

void ProcessBatchWorker::Execute() {
    std::vector<const char*> file_path_ptrs;
    for (const auto& path : file_paths_) {
        file_path_ptrs.push_back(path.c_str());
    }
    
    anidb_batch_options_t options = {};
    options.algorithms = algorithms_.data();
    options.algorithm_count = algorithms_.size();
    options.max_concurrent = max_concurrent_;
    options.continue_on_error = continue_on_error_ ? 1 : 0;
    options.skip_existing = skip_existing_ ? 1 : 0;
    
    result_ = anidb_process_batch(handle_, file_path_ptrs.data(),
        file_path_ptrs.size(), &options, &batch_result_);
}

void ProcessBatchWorker::OnOK() {
    Napi::Env env = Env();
    
    if (result_ != ANIDB_SUCCESS) {
        deferred_.Reject(Napi::Error::New(env, anidb_error_string(result_)).Value());
        return;
    }
    
    Napi::Object js_result = ClientWrapper::ConvertBatchResult(env, batch_result_);
    deferred_.Resolve(js_result);
}

// CalculateHashWorker implementation
CalculateHashWorker::CalculateHashWorker(Napi::Env env, const std::string& file_path,
                                       anidb_hash_algorithm_t algorithm,
                                       Napi::Promise::Deferred deferred)
    : AniDBAsyncWorker(env, nullptr, deferred), file_path_(file_path), algorithm_(algorithm) {
}

void CalculateHashWorker::Execute() {
    size_t buffer_size = anidb_hash_buffer_size(algorithm_);
    hash_result_.resize(buffer_size);
    
    result_ = anidb_calculate_hash(file_path_.c_str(), algorithm_,
        &hash_result_[0], buffer_size);
    
    if (result_ == ANIDB_SUCCESS) {
        // Trim to actual string length
        hash_result_.resize(strlen(hash_result_.c_str()));
    }
}

void CalculateHashWorker::OnOK() {
    Napi::Env env = Env();
    
    if (result_ != ANIDB_SUCCESS) {
        deferred_.Reject(Napi::Error::New(env, anidb_error_string(result_)).Value());
        return;
    }
    
    deferred_.Resolve(Napi::String::New(env, hash_result_));
}

// IdentifyFileWorker implementation
IdentifyFileWorker::IdentifyFileWorker(Napi::Env env, anidb_client_handle_t handle,
                                     const std::string& ed2k_hash, uint64_t file_size,
                                     Napi::Promise::Deferred deferred)
    : AniDBAsyncWorker(env, handle, deferred), ed2k_hash_(ed2k_hash),
      file_size_(file_size), anime_info_(nullptr) {
}

IdentifyFileWorker::~IdentifyFileWorker() {
    if (anime_info_) {
        anidb_free_anime_info(anime_info_);
    }
}

void IdentifyFileWorker::Execute() {
    result_ = anidb_identify_file(handle_, ed2k_hash_.c_str(), file_size_, &anime_info_);
}

void IdentifyFileWorker::OnOK() {
    Napi::Env env = Env();
    
    if (result_ != ANIDB_SUCCESS) {
        deferred_.Reject(Napi::Error::New(env, anidb_error_string(result_)).Value());
        return;
    }
    
    Napi::Object js_info = ClientWrapper::ConvertAnimeInfo(env, anime_info_);
    deferred_.Resolve(js_info);
}