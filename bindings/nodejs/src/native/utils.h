#ifndef UTILS_H
#define UTILS_H

#include <napi.h>
#include "../../../anidb_client_core/include/anidb.h"

namespace Utils {
    // Convert JavaScript hash algorithm string to native enum
    anidb_hash_algorithm_t ParseHashAlgorithm(const std::string& algo);
    
    // Convert native hash algorithm to string
    std::string HashAlgorithmToString(anidb_hash_algorithm_t algo);
    
    // Parse hash algorithms from JavaScript array or string
    std::vector<anidb_hash_algorithm_t> ParseHashAlgorithms(Napi::Value value);
    
    // Create JavaScript error from AniDB result
    Napi::Error CreateError(Napi::Env env, anidb_result_t result, const std::string& context = "");
    
    // Validate file path
    bool ValidateFilePath(const std::string& path);
    
    // Convert status enum to string
    std::string StatusToString(anidb_status_t status);
}

#endif // UTILS_H