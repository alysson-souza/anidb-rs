/**
 * AniDB Client for Node.js
 * High-performance anime file hashing and identification
 */

import * as path from 'path';
import { EventEmitter } from 'events';
import { Readable } from 'stream';
import * as fs from 'fs/promises';

// Import native binding
const binding = require('node-gyp-build')(path.join(__dirname, '..'));

// Re-export types
export * from './types';

// Import types
import {
  AniDBConfig,
  ProcessOptions,
  BatchOptions,
  FileResult,
  BatchResult,
  AnimeInfo,
  HashAlgorithm,
  Status,
  ErrorCode,
  ProgressInfo,
  EventType,
  AniDBEvent,
  CallbackType
} from './types';

/**
 * Main AniDB client class
 * Provides high-performance file hashing and anime identification
 */
export class AniDBClient extends EventEmitter {
  private native: any;
  private eventPollInterval?: NodeJS.Timer;
  private isDestroyed: boolean = false;

  /**
   * Create a new AniDB client instance
   * @param config Optional configuration
   */
  constructor(config?: AniDBConfig) {
    super();
    
    try {
      this.native = new binding.AniDBClientNative(config);
      this.setupEventPolling();
    } catch (error) {
      throw this.wrapError(error);
    }
  }

  /**
   * Process a single file
   * @param filePath Path to the file
   * @param options Processing options
   * @returns File processing result
   */
  async processFile(filePath: string, options?: ProcessOptions): Promise<FileResult> {
    this.checkDestroyed();
    
    const opts = this.normalizeProcessOptions(options);
    
    try {
      return await this.native.processFileAsync(filePath, opts);
    } catch (error) {
      throw this.wrapError(error);
    }
  }

  /**
   * Process a single file synchronously
   * @param filePath Path to the file
   * @param options Processing options
   * @returns File processing result
   */
  processFileSync(filePath: string, options?: ProcessOptions): FileResult {
    this.checkDestroyed();
    
    const opts = this.normalizeProcessOptions(options);
    
    try {
      return this.native.processFile(filePath, opts);
    } catch (error) {
      throw this.wrapError(error);
    }
  }

  /**
   * Process multiple files in batch
   * @param filePaths Array of file paths
   * @param options Batch processing options
   * @returns Batch processing result
   */
  async processBatch(filePaths: string[], options?: BatchOptions): Promise<BatchResult> {
    this.checkDestroyed();
    
    const opts = this.normalizeBatchOptions(options);
    
    try {
      return await this.native.processBatchAsync(filePaths, opts);
    } catch (error) {
      throw this.wrapError(error);
    }
  }

  /**
   * Process multiple files in batch synchronously
   * @param filePaths Array of file paths
   * @param options Batch processing options
   * @returns Batch processing result
   */
  processBatchSync(filePaths: string[], options?: BatchOptions): BatchResult {
    this.checkDestroyed();
    
    const opts = this.normalizeBatchOptions(options);
    
    try {
      return this.native.processBatch(filePaths, opts);
    } catch (error) {
      throw this.wrapError(error);
    }
  }

  /**
   * Calculate hash for a file
   * @param filePath Path to the file
   * @param algorithm Hash algorithm to use
   * @returns Hash string
   */
  async calculateHash(filePath: string, algorithm: HashAlgorithm | string = 'ed2k'): Promise<string> {
    this.checkDestroyed();
    
    const algo = this.parseHashAlgorithm(algorithm);
    
    try {
      // Use async version through processFile for consistency
      const result = await this.processFile(filePath, { algorithms: [algorithm] });
      const algoName = typeof algorithm === 'string' ? algorithm : this.getAlgorithmName(algo);
      return result.hashes[algoName] || '';
    } catch (error) {
      throw this.wrapError(error);
    }
  }

  /**
   * Calculate hash for a buffer
   * @param buffer Data buffer
   * @param algorithm Hash algorithm to use
   * @returns Hash string
   */
  calculateHashBuffer(buffer: Buffer, algorithm: HashAlgorithm | string = 'ed2k'): string {
    this.checkDestroyed();
    
    const algo = this.parseHashAlgorithm(algorithm);
    
    try {
      return this.native.calculateHashBuffer(buffer, algo);
    } catch (error) {
      throw this.wrapError(error);
    }
  }

  /**
   * Create a streaming hash calculator
   * @param algorithms Hash algorithms to calculate
   * @returns Readable stream that emits progress events
   */
  createHashStream(algorithms: (HashAlgorithm | string)[] = ['ed2k']): HashStream {
    this.checkDestroyed();
    
    return new HashStream(this, algorithms);
  }

  /**
   * Identify an anime file by ED2K hash and size
   * @param ed2kHash ED2K hash of the file
   * @param fileSize File size in bytes
   * @returns Anime identification info
   */
  async identifyFile(ed2kHash: string, fileSize: number): Promise<AnimeInfo | null> {
    this.checkDestroyed();
    
    try {
      return await this.native.identifyFile(ed2kHash, fileSize);
    } catch (error) {
      if (error.code === ErrorCode.NETWORK) {
        return null;
      }
      throw this.wrapError(error);
    }
  }

  /**
   * Clear the hash cache
   */
  clearCache(): void {
    this.checkDestroyed();
    
    try {
      this.native.cacheClear();
    } catch (error) {
      throw this.wrapError(error);
    }
  }

  /**
   * Get cache statistics
   * @returns Cache statistics
   */
  getCacheStats(): { totalEntries: number; sizeBytes: number } {
    this.checkDestroyed();
    
    try {
      return this.native.cacheGetStats();
    } catch (error) {
      throw this.wrapError(error);
    }
  }

  /**
   * Check if a file hash is in cache
   * @param filePath Path to the file
   * @param algorithm Hash algorithm
   * @returns True if cached
   */
  isCached(filePath: string, algorithm: HashAlgorithm | string = 'ed2k'): boolean {
    this.checkDestroyed();
    
    const algo = this.parseHashAlgorithm(algorithm);
    
    try {
      return this.native.cacheCheckFile(filePath, algo);
    } catch (error) {
      throw this.wrapError(error);
    }
  }

  /**
   * Get the last error message
   * @returns Error message
   */
  getLastError(): string {
    if (this.isDestroyed) {
      return 'Client has been destroyed';
    }
    
    try {
      return this.native.getLastError();
    } catch (error) {
      return 'Failed to get last error';
    }
  }

  /**
   * Destroy the client and release resources
   */
  destroy(): void {
    if (this.isDestroyed) {
      return;
    }
    
    this.isDestroyed = true;
    
    // Stop event polling
    if (this.eventPollInterval) {
      clearInterval(this.eventPollInterval);
      this.eventPollInterval = undefined;
    }
    
    // Disconnect events
    try {
      this.native.disconnectEvents();
    } catch {}
    
    // Remove all listeners
    this.removeAllListeners();
    
    // Native destructor will be called by GC
    this.native = null;
  }

  /**
   * Setup event system
   */
  private setupEventPolling(): void {
    // Connect native events
    this.native.connectEvents((event: AniDBEvent) => {
      this.handleNativeEvent(event);
    });
  }

  /**
   * Handle native events
   */
  private handleNativeEvent(event: AniDBEvent): void {
    // Emit specific event
    this.emit(event.type, event);
    
    // Emit generic event
    this.emit('event', event);
    
    // Handle specific event types
    switch (event.type) {
      case EventType.FILE_START:
        this.emit('file:start', {
          filePath: event.data.file.filePath,
          fileSize: event.data.file.fileSize
        });
        break;
        
      case EventType.FILE_COMPLETE:
        this.emit('file:complete', {
          filePath: event.data.file.filePath,
          fileSize: event.data.file.fileSize
        });
        break;
        
      case EventType.HASH_COMPLETE:
        this.emit('hash:complete', {
          algorithm: event.data.hash.algorithm,
          hash: event.data.hash.hashValue
        });
        break;
    }
  }

  /**
   * Normalize process options
   */
  private normalizeProcessOptions(options?: ProcessOptions): any {
    const opts = options || {};
    
    return {
      algorithms: this.parseHashAlgorithms(opts.algorithms || ['ed2k']),
      enableProgress: opts.enableProgress || false,
      verifyExisting: opts.verifyExisting || false
    };
  }

  /**
   * Normalize batch options
   */
  private normalizeBatchOptions(options?: BatchOptions): any {
    const opts = options || {};
    
    return {
      algorithms: this.parseHashAlgorithms(opts.algorithms || ['ed2k']),
      maxConcurrent: opts.maxConcurrent || 4,
      continueOnError: opts.continueOnError || false,
      skipExisting: opts.skipExisting || false
    };
  }

  /**
   * Parse hash algorithm
   */
  private parseHashAlgorithm(algorithm: HashAlgorithm | string): number {
    if (typeof algorithm === 'number') {
      return algorithm;
    }
    
    switch (algorithm.toLowerCase()) {
      case 'ed2k': return binding.HashAlgorithm.ED2K;
      case 'crc32': return binding.HashAlgorithm.CRC32;
      case 'md5': return binding.HashAlgorithm.MD5;
      case 'sha1': return binding.HashAlgorithm.SHA1;
      case 'tth': return binding.HashAlgorithm.TTH;
      default: return binding.HashAlgorithm.ED2K;
    }
  }

  /**
   * Parse multiple hash algorithms
   */
  private parseHashAlgorithms(algorithms: (HashAlgorithm | string)[]): number[] {
    return algorithms.map(algo => this.parseHashAlgorithm(algo));
  }

  /**
   * Get algorithm name
   */
  private getAlgorithmName(algorithm: number): string {
    return binding.hashAlgorithmName(algorithm).toLowerCase();
  }

  /**
   * Check if client is destroyed
   */
  private checkDestroyed(): void {
    if (this.isDestroyed) {
      throw new Error('Client has been destroyed');
    }
  }

  /**
   * Wrap native errors
   */
  private wrapError(error: any): Error {
    if (error instanceof Error) {
      return error;
    }
    
    const err = new Error(error.message || 'Unknown error');
    if (error.code) {
      (err as any).code = error.code;
    }
    return err;
  }
}

/**
 * Stream-based hash calculator
 */
export class HashStream extends Readable {
  private client: AniDBClient;
  private algorithms: (HashAlgorithm | string)[];
  private filePath?: string;
  private processing: boolean = false;

  constructor(client: AniDBClient, algorithms: (HashAlgorithm | string)[]) {
    super({ objectMode: true });
    this.client = client;
    this.algorithms = algorithms;
  }

  /**
   * Process a file
   */
  async processFile(filePath: string): Promise<void> {
    if (this.processing) {
      throw new Error('Already processing a file');
    }
    
    this.filePath = filePath;
    this.processing = true;
    
    try {
      // Setup progress tracking
      const progressHandler = (info: ProgressInfo) => {
        this.push({
          type: 'progress',
          ...info
        });
      };
      
      this.client.on('progress', progressHandler);
      
      // Process file
      const result = await this.client.processFile(filePath, {
        algorithms: this.algorithms,
        enableProgress: true
      });
      
      // Emit result
      this.push({
        type: 'complete',
        result
      });
      
      // End stream
      this.push(null);
      
      // Cleanup
      this.client.off('progress', progressHandler);
    } catch (error) {
      this.destroy(error as Error);
    } finally {
      this.processing = false;
    }
  }

  _read(): void {
    // Required for Readable, but we push data asynchronously
  }
}

// Re-export constants
export { HashAlgorithm, Status, ErrorCode, EventType, CallbackType } from './types';

// Export utility functions
export const version = binding.version;
export const abiVersion = binding.abiVersion;
export const errorString = binding.errorString;
export const hashAlgorithmName = binding.hashAlgorithmName;
export const hashBufferSize = binding.hashBufferSize;

// Default export
export default AniDBClient;