#include "utils.h"
#include <algorithm>
#include <cctype>

namespace Utils {

anidb_hash_algorithm_t ParseHashAlgorithm(const std::string& algo) {
    std::string lower_algo = algo;
    std::transform(lower_algo.begin(), lower_algo.end(), lower_algo.begin(),
                   [](unsigned char c) { return std::tolower(c); });
    
    if (lower_algo == "ed2k") return ANIDB_HASH_ED2K;
    if (lower_algo == "crc32") return ANIDB_HASH_CRC32;
    if (lower_algo == "md5") return ANIDB_HASH_MD5;
    if (lower_algo == "sha1") return ANIDB_HASH_SHA1;
    if (lower_algo == "tth") return ANIDB_HASH_TTH;
    
    // Default to ED2K if unknown
    return ANIDB_HASH_ED2K;
}

std::string HashAlgorithmToString(anidb_hash_algorithm_t algo) {
    switch (algo) {
        case ANIDB_HASH_ED2K: return "ed2k";
        case ANIDB_HASH_CRC32: return "crc32";
        case ANIDB_HASH_MD5: return "md5";
        case ANIDB_HASH_SHA1: return "sha1";
        case ANIDB_HASH_TTH: return "tth";
        default: return "unknown";
    }
}

std::vector<anidb_hash_algorithm_t> ParseHashAlgorithms(Napi::Value value) {
    std::vector<anidb_hash_algorithm_t> algorithms;
    
    if (value.IsString()) {
        // Single algorithm as string
        std::string algo_str = value.As<Napi::String>().Utf8Value();
        algorithms.push_back(ParseHashAlgorithm(algo_str));
    } else if (value.IsNumber()) {
        // Single algorithm as number
        algorithms.push_back(static_cast<anidb_hash_algorithm_t>(
            value.As<Napi::Number>().Int32Value()
        ));
    } else if (value.IsArray()) {
        // Multiple algorithms
        Napi::Array arr = value.As<Napi::Array>();
        for (uint32_t i = 0; i < arr.Length(); i++) {
            Napi::Value elem = arr.Get(i);
            if (elem.IsString()) {
                algorithms.push_back(ParseHashAlgorithm(
                    elem.As<Napi::String>().Utf8Value()
                ));
            } else if (elem.IsNumber()) {
                algorithms.push_back(static_cast<anidb_hash_algorithm_t>(
                    elem.As<Napi::Number>().Int32Value()
                ));
            }
        }
    }
    
    // Default to ED2K if no valid algorithms
    if (algorithms.empty()) {
        algorithms.push_back(ANIDB_HASH_ED2K);
    }
    
    return algorithms;
}

Napi::Error CreateError(Napi::Env env, anidb_result_t result, const std::string& context) {
    std::string message = anidb_error_string(result);
    if (!context.empty()) {
        message = context + ": " + message;
    }
    
    Napi::Error error = Napi::Error::New(env, message);
    error.Set("code", Napi::Number::New(env, result));
    
    return error;
}

bool ValidateFilePath(const std::string& path) {
    return !path.empty() && path.length() < 4096; // Basic validation
}

std::string StatusToString(anidb_status_t status) {
    switch (status) {
        case ANIDB_STATUS_PENDING: return "pending";
        case ANIDB_STATUS_PROCESSING: return "processing";
        case ANIDB_STATUS_COMPLETED: return "completed";
        case ANIDB_STATUS_FAILED: return "failed";
        case ANIDB_STATUS_CANCELLED: return "cancelled";
        default: return "unknown";
    }
}

} // namespace Utils