#ifndef STREAM_WORKER_H
#define STREAM_WORKER_H

#include <napi.h>
#include "../../../anidb_client_core/include/anidb.h"
#include <string>
#include <memory>

// Stream-based file processing worker for handling large files
class StreamProcessWorker : public Napi::AsyncProgressWorker<float> {
private:
    anidb_client_handle_t handle_;
    std::string file_path_;
    std::vector<anidb_hash_algorithm_t> algorithms_;
    anidb_file_result_t* result_;
    anidb_result_t status_;
    
    // Progress tracking
    float last_progress_;
    
public:
    StreamProcessWorker(Napi::Function& callback, anidb_client_handle_t handle,
                       const std::string& file_path,
                       const std::vector<anidb_hash_algorithm_t>& algorithms);
    
    ~StreamProcessWorker();
    
    void Execute(const ExecutionProgress& progress) override;
    void OnOK() override;
    void OnError(const Napi::Error& error) override;
    void OnProgress(const float* data, size_t count) override;
    
private:
    static void ProgressCallback(float percentage, uint64_t bytes_processed,
                               uint64_t total_bytes, void* user_data);
};

#endif // STREAM_WORKER_H