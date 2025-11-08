/**
 * Async/Promise example for AniDB Client
 */

const { AniDBClient } = require('anidb-client');
const fs = require('fs').promises;
const path = require('path');

// Process files with progress tracking
async function processWithProgress(client, filePath) {
  console.log(`\nProcessing: ${path.basename(filePath)}`);
  
  // Setup progress listener
  const progressHandler = (event) => {
    if (event.data.file && event.data.file.filePath === filePath) {
      process.stdout.write(`\rProgress: ${event.data.percentage?.toFixed(1)}%`);
    }
  };
  
  client.on('event', progressHandler);
  
  try {
    const result = await client.processFile(filePath, {
      algorithms: ['ed2k', 'crc32'],
      enableProgress: true
    });
    
    console.log('\rProgress: 100.0%');
    console.log(`ED2K: ${result.hashes.ed2k}`);
    console.log(`CRC32: ${result.hashes.crc32}`);
    console.log(`Time: ${result.processingTimeMs}ms`);
    
    return result;
  } finally {
    client.off('event', progressHandler);
  }
}

// Process directory recursively
async function processDirectory(client, dirPath, extensions = ['.mkv', '.avi', '.mp4']) {
  const results = [];
  
  async function* walk(dir) {
    const entries = await fs.readdir(dir, { withFileTypes: true });
    
    for (const entry of entries) {
      const fullPath = path.join(dir, entry.name);
      
      if (entry.isDirectory()) {
        yield* walk(fullPath);
      } else if (entry.isFile()) {
        const ext = path.extname(entry.name).toLowerCase();
        if (extensions.includes(ext)) {
          yield fullPath;
        }
      }
    }
  }
  
  // Process files concurrently with limit
  const files = [];
  for await (const file of walk(dirPath)) {
    files.push(file);
  }
  
  console.log(`Found ${files.length} media files`);
  
  // Process in batches
  const batchSize = 3;
  for (let i = 0; i < files.length; i += batchSize) {
    const batch = files.slice(i, i + batchSize);
    const promises = batch.map(file => processWithProgress(client, file));
    const batchResults = await Promise.all(promises);
    results.push(...batchResults);
  }
  
  return results;
}

// Demonstrate various async patterns
async function main() {
  const client = new AniDBClient({
    cacheDir: '.anidb_cache',
    maxConcurrentFiles: 4,
    enableDebugLogging: false
  });
  
  try {
    const targetPath = process.argv[2] || '.';
    const stats = await fs.stat(targetPath);
    
    if (stats.isDirectory()) {
      // Process directory
      console.log(`Processing directory: ${targetPath}`);
      const results = await processDirectory(client, targetPath);
      
      console.log(`\nProcessed ${results.length} files`);
      console.log('Summary:');
      results.forEach((result, i) => {
        console.log(`${i + 1}. ${path.basename(result.filePath)}: ${result.hashes.ed2k}`);
      });
      
    } else {
      // Process single file with different patterns
      
      // Pattern 1: Simple async/await
      console.log('Pattern 1: Simple async/await');
      const result1 = await client.processFile(targetPath);
      console.log('Result:', result1.hashes.ed2k);
      
      // Pattern 2: Promise chaining
      console.log('\nPattern 2: Promise chaining');
      await client.processFile(targetPath, { algorithms: ['crc32'] })
        .then(result => {
          console.log('CRC32:', result.hashes.crc32);
          return client.calculateHash(targetPath, 'md5');
        })
        .then(md5 => {
          console.log('MD5:', md5);
        });
      
      // Pattern 3: Concurrent processing
      console.log('\nPattern 3: Concurrent hash calculation');
      const [ed2k, crc32, md5] = await Promise.all([
        client.calculateHash(targetPath, 'ed2k'),
        client.calculateHash(targetPath, 'crc32'),
        client.calculateHash(targetPath, 'md5')
      ]);
      console.log('ED2K:', ed2k);
      console.log('CRC32:', crc32);
      console.log('MD5:', md5);
      
      // Pattern 4: Error handling
      console.log('\nPattern 4: Error handling');
      try {
        await client.processFile('non-existent-file.mkv');
      } catch (error) {
        console.log('Expected error:', error.message);
        console.log('Error code:', error.code);
      }
    }
    
    // Event handling
    console.log('\nEvent handling example:');
    
    // Setup event listeners
    client.on('file:start', ({ filePath, fileSize }) => {
      console.log(`Started processing: ${path.basename(filePath)} (${fileSize} bytes)`);
    });
    
    client.on('hash:complete', ({ algorithm, hash }) => {
      console.log(`Hash complete: ${algorithm} = ${hash.substring(0, 16)}...`);
    });
    
    client.on('file:complete', ({ filePath }) => {
      console.log(`Completed: ${path.basename(filePath)}`);
    });
    
    // Process with events
    if (!stats.isDirectory()) {
      await client.processFile(targetPath, {
        algorithms: ['ed2k', 'crc32', 'md5'],
        enableProgress: true
      });
    }
    
  } catch (error) {
    console.error('Error:', error.message);
    console.error('Stack:', error.stack);
  } finally {
    client.destroy();
  }
}

// Run the example
main().catch(console.error);