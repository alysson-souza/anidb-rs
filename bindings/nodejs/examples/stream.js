/**
 * Streaming example for AniDB Client
 * Demonstrates stream-based processing for large files
 */

const { AniDBClient, HashStream } = require('anidb-client');
const fs = require('fs');
const path = require('path');
const { pipeline } = require('stream/promises');
const { Transform } = require('stream');

// Create a transform stream that monitors progress
class ProgressMonitor extends Transform {
  constructor(options = {}) {
    super({ objectMode: true });
    this.startTime = Date.now();
    this.lastProgress = 0;
  }
  
  _transform(chunk, encoding, callback) {
    if (chunk.type === 'progress') {
      const progress = chunk.percentage;
      
      // Update progress bar
      const barLength = 40;
      const filled = Math.round(barLength * progress / 100);
      const bar = '█'.repeat(filled) + '░'.repeat(barLength - filled);
      
      // Calculate speed
      const elapsed = (Date.now() - this.startTime) / 1000;
      const speed = chunk.bytesProcessed / elapsed / 1024 / 1024; // MB/s
      
      process.stdout.write(`\r[${bar}] ${progress.toFixed(1)}% | ${speed.toFixed(1)} MB/s`);
      
      this.lastProgress = progress;
    } else if (chunk.type === 'complete') {
      // Clear progress line and show result
      process.stdout.write('\r' + ' '.repeat(60) + '\r');
      console.log('✓ Processing complete');
      console.log('  Hashes:', Object.keys(chunk.result.hashes).join(', '));
      console.log('  Time:', chunk.result.processingTimeMs, 'ms');
    }
    
    this.push(chunk);
    callback();
  }
}

// Process file using streaming API
async function processFileStream(client, filePath) {
  console.log(`\nProcessing with stream: ${path.basename(filePath)}`);
  
  const stream = client.createHashStream(['ed2k', 'crc32', 'md5']);
  const monitor = new ProgressMonitor();
  
  // Setup pipeline
  const results = [];
  
  await pipeline(
    stream,
    monitor,
    new Transform({
      objectMode: true,
      transform(chunk, encoding, callback) {
        if (chunk.type === 'complete') {
          results.push(chunk.result);
        }
        callback();
      }
    })
  );
  
  // Start processing
  await stream.processFile(filePath);
  
  return results[0];
}

// Process multiple files with streaming
async function processMultipleStreams(client, filePaths) {
  console.log('\nProcessing multiple files with streams...');
  
  const streams = filePaths.map(filePath => {
    const stream = client.createHashStream(['ed2k']);
    
    return {
      filePath,
      stream,
      promise: stream.processFile(filePath)
    };
  });
  
  // Process all streams concurrently
  const results = await Promise.all(
    streams.map(async ({ stream, filePath }) => {
      const chunks = [];
      
      for await (const chunk of stream) {
        if (chunk.type === 'complete') {
          return {
            filePath,
            result: chunk.result
          };
        } else if (chunk.type === 'progress') {
          // Could aggregate progress here
        }
      }
    })
  );
  
  return results;
}

// Custom readable stream that processes files
class FileHashStream extends Transform {
  constructor(client, options = {}) {
    super({ objectMode: true });
    this.client = client;
    this.algorithms = options.algorithms || ['ed2k'];
  }
  
  async _transform(filePath, encoding, callback) {
    try {
      const result = await this.client.processFile(filePath, {
        algorithms: this.algorithms
      });
      
      this.push({
        filePath,
        ...result
      });
      
      callback();
    } catch (error) {
      callback(error);
    }
  }
}

// Main example
async function main() {
  const client = new AniDBClient();
  
  try {
    const targetPath = process.argv[2];
    
    if (!targetPath) {
      console.error('Usage: node stream.js <file-or-directory>');
      process.exit(1);
    }
    
    const stats = await fs.promises.stat(targetPath);
    
    if (stats.isFile()) {
      // Single file stream processing
      console.log('Single file stream processing');
      const result = await processFileStream(client, targetPath);
      console.log('\nFinal result:', result);
      
    } else if (stats.isDirectory()) {
      // Directory stream processing
      console.log('Directory stream processing');
      
      // Find all video files
      const files = await fs.promises.readdir(targetPath);
      const videoFiles = files
        .filter(f => ['.mkv', '.avi', '.mp4'].includes(path.extname(f).toLowerCase()))
        .map(f => path.join(targetPath, f))
        .slice(0, 5); // Limit to 5 files for demo
      
      if (videoFiles.length === 0) {
        console.log('No video files found');
        return;
      }
      
      // Create a processing pipeline
      console.log(`\nProcessing ${videoFiles.length} files...`);
      
      const hashStream = new FileHashStream(client, { algorithms: ['ed2k', 'crc32'] });
      const results = [];
      
      await pipeline(
        async function* () {
          for (const file of videoFiles) {
            yield file;
          }
        },
        hashStream,
        new Transform({
          objectMode: true,
          transform(chunk, encoding, callback) {
            console.log(`\n✓ ${path.basename(chunk.filePath)}`);
            console.log(`  ED2K: ${chunk.hashes.ed2k}`);
            console.log(`  CRC32: ${chunk.hashes.crc32}`);
            console.log(`  Size: ${(chunk.fileSize / 1024 / 1024).toFixed(2)} MB`);
            
            results.push(chunk);
            callback();
          }
        })
      );
      
      // Summary
      console.log(`\n\nProcessed ${results.length} files`);
      const totalSize = results.reduce((sum, r) => sum + r.fileSize, 0);
      const totalTime = results.reduce((sum, r) => sum + r.processingTimeMs, 0);
      console.log(`Total size: ${(totalSize / 1024 / 1024).toFixed(2)} MB`);
      console.log(`Total time: ${(totalTime / 1000).toFixed(2)} seconds`);
      console.log(`Average speed: ${(totalSize / totalTime * 1000 / 1024 / 1024).toFixed(2)} MB/s`);
    }
    
    // Demonstrate buffer hashing
    console.log('\n\nBuffer hashing example:');
    const testData = Buffer.from('Hello, AniDB!', 'utf8');
    const bufferHash = client.calculateHashBuffer(testData, 'md5');
    console.log('MD5 of test buffer:', bufferHash);
    
  } catch (error) {
    console.error('\nError:', error.message);
    if (error.stack) {
      console.error(error.stack);
    }
  } finally {
    client.destroy();
  }
}

// Run the example
main().catch(console.error);