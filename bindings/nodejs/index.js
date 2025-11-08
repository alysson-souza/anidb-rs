// Entry point for the package
// This file is used when the TypeScript build is not available

try {
  // Try to load the compiled TypeScript version
  module.exports = require('./dist/index.js');
} catch (error) {
  // Fallback to loading the native binding directly
  console.warn('TypeScript build not found, loading native binding directly');
  
  const binding = require('./build/Release/anidb_client.node');
  
  // Create a minimal wrapper
  class AniDBClient {
    constructor(config) {
      this.native = new binding.AniDBClientNative(config);
    }
    
    processFile(filePath, options) {
      return this.native.processFileAsync(filePath, options || {});
    }
    
    processFileSync(filePath, options) {
      return this.native.processFile(filePath, options || {});
    }
    
    destroy() {
      // Native destructor will handle cleanup
      this.native = null;
    }
  }
  
  module.exports = {
    AniDBClient,
    HashAlgorithm: binding.HashAlgorithm,
    Status: binding.Status,
    ErrorCode: binding.ErrorCode,
    version: binding.version,
    abiVersion: binding.abiVersion
  };
}