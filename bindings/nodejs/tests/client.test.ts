import { AniDBClient, HashAlgorithm, Status, ErrorCode } from '../src';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

describe('AniDBClient', () => {
  let client: AniDBClient;
  let testFile: string;
  
  beforeAll(async () => {
    // Create a test file
    const tmpDir = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'anidb-test-'));
    testFile = path.join(tmpDir, 'test.dat');
    
    // Create 10MB test file with predictable content
    const buffer = Buffer.alloc(10 * 1024 * 1024);
    for (let i = 0; i < buffer.length; i++) {
      buffer[i] = i % 256;
    }
    await fs.promises.writeFile(testFile, buffer);
  });
  
  afterAll(async () => {
    // Clean up test file
    if (testFile) {
      await fs.promises.unlink(testFile).catch(() => {});
      await fs.promises.rmdir(path.dirname(testFile)).catch(() => {});
    }
  });
  
  beforeEach(() => {
    client = new AniDBClient();
  });
  
  afterEach(() => {
    if (client) {
      client.destroy();
    }
  });
  
  describe('constructor', () => {
    it('should create client with default config', () => {
      expect(client).toBeDefined();
    });
    
    it('should create client with custom config', () => {
      const customClient = new AniDBClient({
        cacheDir: './test_cache',
        maxConcurrentFiles: 8,
        chunkSize: 128 * 1024,
        enableDebugLogging: true
      });
      
      expect(customClient).toBeDefined();
      customClient.destroy();
    });
  });
  
  describe('processFile', () => {
    it('should process file with default options', async () => {
      const result = await client.processFile(testFile);
      
      expect(result).toBeDefined();
      expect(result.filePath).toBe(testFile);
      expect(result.fileSize).toBe(10 * 1024 * 1024);
      expect(result.status).toBe(Status.COMPLETED);
      expect(result.hashes.ed2k).toBeDefined();
      expect(result.hashes.ed2k).toMatch(/^[a-f0-9]{32}$/);
      expect(result.processingTimeMs).toBeGreaterThan(0);
    });
    
    it('should process file with multiple algorithms', async () => {
      const result = await client.processFile(testFile, {
        algorithms: ['ed2k', 'crc32', 'md5', 'sha1']
      });
      
      expect(result.hashes.ed2k).toBeDefined();
      expect(result.hashes.crc32).toBeDefined();
      expect(result.hashes.md5).toBeDefined();
      expect(result.hashes.sha1).toBeDefined();
      
      // Verify hash formats
      expect(result.hashes.ed2k).toMatch(/^[a-f0-9]{32}$/);
      expect(result.hashes.crc32).toMatch(/^[a-f0-9]{8}$/);
      expect(result.hashes.md5).toMatch(/^[a-f0-9]{32}$/);
      expect(result.hashes.sha1).toMatch(/^[a-f0-9]{40}$/);
    });
    
    it('should accept string algorithm names', async () => {
      const result = await client.processFile(testFile, {
        algorithms: ['ed2k', 'CRC32', 'MD5']
      });
      
      expect(result.hashes.ed2k).toBeDefined();
      expect(result.hashes.crc32).toBeDefined();
      expect(result.hashes.md5).toBeDefined();
    });
    
    it('should handle non-existent file', async () => {
      await expect(client.processFile('/non/existent/file.mkv'))
        .rejects
        .toThrow(/not found/i);
    });
    
    it('should support progress tracking', async () => {
      const progressEvents: any[] = [];
      
      client.on('event', (event) => {
        if (event.type === 'file:start' || event.type === 'file:complete') {
          progressEvents.push(event);
        }
      });
      
      await client.processFile(testFile, {
        enableProgress: true
      });
      
      expect(progressEvents.length).toBeGreaterThan(0);
    });
  });
  
  describe('processFileSync', () => {
    it('should process file synchronously', () => {
      const result = client.processFileSync(testFile);
      
      expect(result).toBeDefined();
      expect(result.filePath).toBe(testFile);
      expect(result.status).toBe(Status.COMPLETED);
      expect(result.hashes.ed2k).toBeDefined();
    });
  });
  
  describe('processBatch', () => {
    let testFiles: string[];
    
    beforeAll(async () => {
      // Create multiple test files
      testFiles = [];
      for (let i = 0; i < 3; i++) {
        const file = path.join(path.dirname(testFile), `test${i}.dat`);
        await fs.promises.writeFile(file, Buffer.alloc(1024 * 1024)); // 1MB each
        testFiles.push(file);
      }
    });
    
    afterAll(async () => {
      // Clean up
      for (const file of testFiles) {
        await fs.promises.unlink(file).catch(() => {});
      }
    });
    
    it('should process multiple files', async () => {
      const result = await client.processBatch(testFiles);
      
      expect(result.totalFiles).toBe(3);
      expect(result.successfulFiles).toBe(3);
      expect(result.failedFiles).toBe(0);
      expect(result.results).toHaveLength(3);
      expect(result.totalTimeMs).toBeGreaterThan(0);
      
      result.results.forEach((fileResult, i) => {
        expect(fileResult.filePath).toBe(testFiles[i]);
        expect(fileResult.status).toBe(Status.COMPLETED);
        expect(fileResult.hashes.ed2k).toBeDefined();
      });
    });
    
    it('should continue on error when specified', async () => {
      const mixedFiles = [...testFiles, '/non/existent/file.mkv'];
      
      const result = await client.processBatch(mixedFiles, {
        continueOnError: true
      });
      
      expect(result.totalFiles).toBe(4);
      expect(result.successfulFiles).toBe(3);
      expect(result.failedFiles).toBe(1);
    });
    
    it('should skip existing cached files', async () => {
      // Process once to cache
      await client.processBatch(testFiles);
      
      // Process again with skip existing
      const result = await client.processBatch(testFiles, {
        skipExisting: true
      });
      
      // Should still complete successfully
      expect(result.successfulFiles).toBe(3);
    });
  });
  
  describe('calculateHash', () => {
    it('should calculate single hash', async () => {
      const hash = await client.calculateHash(testFile, 'md5');
      
      expect(hash).toBeDefined();
      expect(hash).toMatch(/^[a-f0-9]{32}$/);
    });
    
    it('should accept enum values', async () => {
      const hash = await client.calculateHash(testFile, HashAlgorithm.CRC32);
      
      expect(hash).toBeDefined();
      expect(hash).toMatch(/^[a-f0-9]{8}$/);
    });
  });
  
  describe('calculateHashBuffer', () => {
    it('should calculate hash for buffer', () => {
      const buffer = Buffer.from('Hello, AniDB!', 'utf8');
      const hash = client.calculateHashBuffer(buffer, 'md5');
      
      expect(hash).toBeDefined();
      expect(hash).toMatch(/^[a-f0-9]{32}$/);
    });
  });
  
  describe('cache operations', () => {
    it('should check if file is cached', async () => {
      // Process file first
      await client.processFile(testFile);
      
      // Check cache
      const isCached = client.isCached(testFile, 'ed2k');
      expect(isCached).toBe(true);
      
      const notCached = client.isCached(testFile, 'tth');
      expect(notCached).toBe(false);
    });
    
    it('should get cache statistics', async () => {
      const stats = client.getCacheStats();
      
      expect(stats).toBeDefined();
      expect(stats.totalEntries).toBeGreaterThanOrEqual(0);
      expect(stats.sizeBytes).toBeGreaterThanOrEqual(0);
    });
    
    it('should clear cache', () => {
      expect(() => client.clearCache()).not.toThrow();
    });
  });
  
  describe('error handling', () => {
    it('should get last error message', () => {
      const error = client.getLastError();
      expect(typeof error).toBe('string');
    });
    
    it('should handle destroyed client', () => {
      client.destroy();
      
      expect(() => client.processFileSync(testFile))
        .toThrow(/destroyed/i);
    });
  });
  
  describe('events', () => {
    it('should emit file events', async () => {
      const events: string[] = [];
      
      client.on('file:start', () => events.push('start'));
      client.on('file:complete', () => events.push('complete'));
      
      await client.processFile(testFile);
      
      expect(events).toContain('start');
      expect(events).toContain('complete');
    });
    
    it('should emit hash complete events', async () => {
      const hashEvents: any[] = [];
      
      client.on('hash:complete', (data) => {
        hashEvents.push(data);
      });
      
      await client.processFile(testFile, {
        algorithms: ['ed2k', 'crc32']
      });
      
      expect(hashEvents.length).toBeGreaterThanOrEqual(2);
    });
  });
  
  describe('streaming', () => {
    it('should create hash stream', () => {
      const stream = client.createHashStream(['ed2k', 'md5']);
      
      expect(stream).toBeDefined();
      expect(stream.readable).toBe(true);
    });
  });
});