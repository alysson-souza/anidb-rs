#include "stream_worker.h"
#include "client_wrapper.h"

StreamProcessWorker::StreamProcessWorker(Napi::Function& callback, 
                                       anidb_client_handle_t handle,
                                       const std::string& file_path,
                                       const std::vector<anidb_hash_algorithm_t>& algorithms)
    : Napi::AsyncProgressWorker<float>(callback), handle_(handle), 
      file_path_(file_path), algorithms_(algorithms), result_(nullptr),
      status_(ANIDB_SUCCESS), last_progress_(0.0f) {
}

StreamProcessWorker::~StreamProcessWorker() {
    if (result_) {
        anidb_free_file_result(result_);
    }
}

void StreamProcessWorker::Execute(const ExecutionProgress& progress) {
    anidb_process_options_t options = {};
    options.algorithms = algorithms_.data();
    options.algorithm_count = algorithms_.size();
    options.enable_progress = 1;
    options.verify_existing = 0;
    options.progress_callback = &StreamProcessWorker::ProgressCallback;
    options.user_data = const_cast<ExecutionProgress*>(&progress);
    
    status_ = anidb_process_file(handle_, file_path_.c_str(), &options, &result_);
}

void StreamProcessWorker::OnOK() {
    Napi::HandleScope scope(Env());
    
    if (status_ == ANIDB_SUCCESS && result_) {
        Napi::Object js_result = ClientWrapper::ConvertFileResult(Env(), result_);
        Callback().Call({Env().Null(), js_result});
    } else {
        Callback().Call({Napi::Error::New(Env(), anidb_error_string(status_)).Value()});
    }
}

void StreamProcessWorker::OnError(const Napi::Error& error) {
    Napi::HandleScope scope(Env());
    Callback().Call({error.Value()});
}

void StreamProcessWorker::OnProgress(const float* data, size_t count) {
    Napi::HandleScope scope(Env());
    
    if (count > 0) {
        Napi::Object progress = Napi::Object::New(Env());
        progress.Set("percentage", Napi::Number::New(Env(), data[0]));
        
        Callback().Call({Env().Null(), Env().Undefined(), progress});
    }
}

void StreamProcessWorker::ProgressCallback(float percentage, uint64_t bytes_processed,
                                         uint64_t total_bytes, void* user_data) {
    auto* progress = static_cast<const ExecutionProgress*>(user_data);
    const_cast<ExecutionProgress*>(progress)->Send(&percentage, 1);
}