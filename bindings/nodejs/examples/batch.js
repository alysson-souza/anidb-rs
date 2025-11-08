/**
 * Batch processing example for AniDB Client
 * Demonstrates efficient processing of multiple files
 */

const { AniDBClient } = require('anidb-client');
const fs = require('fs').promises;
const path = require('path');
const os = require('os');

// Find media files recursively
async function findMediaFiles(dir, extensions = ['.mkv', '.avi', '.mp4', '.mpg', '.ogm']) {
  const files = [];
  
  async function scan(currentDir) {
    const entries = await fs.readdir(currentDir, { withFileTypes: true });
    
    for (const entry of entries) {
      const fullPath = path.join(currentDir, entry.name);
      
      if (entry.isDirectory() && !entry.name.startsWith('.')) {
        await scan(fullPath);
      } else if (entry.isFile()) {
        const ext = path.extname(entry.name).toLowerCase();
        if (extensions.includes(ext)) {
          const stats = await fs.stat(fullPath);
          files.push({
            path: fullPath,
            size: stats.size,
            name: entry.name
          });
        }
      }
    }
  }
  
  await scan(dir);
  return files;
}

// Format file size
function formatSize(bytes) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let size = bytes;
  let unitIndex = 0;
  
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }
  
  return `${size.toFixed(2)} ${units[unitIndex]}`;
}

// Format duration
function formatDuration(ms) {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  
  if (hours > 0) {
    return `${hours}h ${minutes % 60}m ${seconds % 60}s`;
  } else if (minutes > 0) {
    return `${minutes}m ${seconds % 60}s`;
  } else {
    return `${seconds}s`;
  }
}

// Process files in batches with detailed progress
async function processBatchWithProgress(client, files, options = {}) {
  console.log(`\nProcessing ${files.length} files in batch mode...`);
  console.log(`Algorithms: ${options.algorithms?.join(', ') || 'ed2k'}`);
  console.log(`Max concurrent: ${options.maxConcurrent || 4}`);
  console.log(`Skip existing: ${options.skipExisting ? 'Yes' : 'No'}`);
  console.log('');
  
  let completed = 0;
  const startTime = Date.now();
  
  // Setup progress handler
  const progressHandler = (event) => {
    if (event.type === 'file:complete') {
      completed++;
      const progress = (completed / files.length * 100).toFixed(1);
      const elapsed = Date.now() - startTime;
      const rate = completed / (elapsed / 1000);
      const eta = (files.length - completed) / rate;
      
      process.stdout.write(
        `\rProgress: ${progress}% | Files: ${completed}/${files.length} | ` +
        `Rate: ${rate.toFixed(1)} files/s | ETA: ${formatDuration(eta * 1000)}`
      );
    }
  };
  
  client.on('event', progressHandler);
  
  try {
    // Process batch
    const result = await client.processBatch(
      files.map(f => f.path),
      options
    );
    
    console.log('\n');
    return result;
  } finally {
    client.off('event', progressHandler);
  }
}

// Main example
async function main() {
  const targetDir = process.argv[2] || '.';
  
  console.log('AniDB Batch Processing Example');
  console.log('==============================');
  console.log(`Target directory: ${path.resolve(targetDir)}`);
  console.log(`CPU cores: ${os.cpus().length}`);
  console.log(`Memory: ${formatSize(os.totalmem())}`);
  
  const client = new AniDBClient({
    cacheDir: '.anidb_cache',
    maxConcurrentFiles: Math.min(os.cpus().length, 8),
    chunkSize: 256 * 1024, // 256KB chunks
    enableDebugLogging: false
  });
  
  try {
    // Find all media files
    console.log('\nScanning for media files...');
    const files = await findMediaFiles(targetDir);
    
    if (files.length === 0) {
      console.log('No media files found!');
      return;
    }
    
    // Sort by size (process larger files first for better load balancing)
    files.sort((a, b) => b.size - a.size);
    
    console.log(`Found ${files.length} media files`);
    const totalSize = files.reduce((sum, f) => sum + f.size, 0);
    console.log(`Total size: ${formatSize(totalSize)}`);
    
    // Show top 5 largest files
    console.log('\nLargest files:');
    files.slice(0, 5).forEach((f, i) => {
      console.log(`  ${i + 1}. ${f.name} (${formatSize(f.size)})`);
    });
    
    // Example 1: Process all files with ED2K only
    console.log('\n\n1. Batch processing with ED2K hash:');
    const result1 = await processBatchWithProgress(client, files, {
      algorithms: ['ed2k'],
      maxConcurrent: 4,
      continueOnError: true,
      skipExisting: true
    });
    
    console.log('Results:');
    console.log(`  Successful: ${result1.successfulFiles}`);
    console.log(`  Failed: ${result1.failedFiles}`);
    console.log(`  Total time: ${formatDuration(result1.totalTimeMs)}`);
    console.log(`  Average speed: ${(totalSize / result1.totalTimeMs * 1000 / 1024 / 1024).toFixed(2)} MB/s`);
    
    // Show failed files if any
    if (result1.failedFiles > 0) {
      console.log('\nFailed files:');
      result1.results
        .filter(r => r.status === 3) // FAILED status
        .forEach(r => {
          console.log(`  - ${path.basename(r.filePath)}: ${r.error}`);
        });
    }
    
    // Example 2: Process smaller subset with multiple algorithms
    const smallFiles = files.filter(f => f.size < 100 * 1024 * 1024).slice(0, 10); // Files < 100MB
    
    if (smallFiles.length > 0) {
      console.log(`\n\n2. Processing ${smallFiles.length} smaller files with multiple algorithms:`);
      
      const result2 = await processBatchWithProgress(client, smallFiles, {
        algorithms: ['ed2k', 'crc32', 'md5'],
        maxConcurrent: 2,
        continueOnError: false,
        skipExisting: false
      });
      
      console.log('\nDetailed results:');
      result2.results.forEach((r, i) => {
        console.log(`\n${i + 1}. ${path.basename(r.filePath)}`);
        console.log(`   Size: ${formatSize(r.fileSize)}`);
        console.log(`   Time: ${r.processingTimeMs}ms`);
        console.log(`   Speed: ${(r.fileSize / r.processingTimeMs * 1000 / 1024 / 1024).toFixed(2)} MB/s`);
        console.log('   Hashes:');
        Object.entries(r.hashes).forEach(([algo, hash]) => {
          console.log(`     ${algo}: ${hash}`);
        });
      });
    }
    
    // Example 3: Process with callback-based progress
    console.log('\n\n3. Batch processing with custom progress callback:');
    
    const progressData = {
      startTime: Date.now(),
      lastUpdate: Date.now(),
      processedBytes: 0
    };
    
    const result3 = await client.processBatch(
      files.slice(0, 5).map(f => f.path),
      {
        algorithms: ['ed2k'],
        maxConcurrent: 3,
        onProgress: (progress) => {
          progressData.processedBytes = progress.bytesProcessed;
          
          const now = Date.now();
          if (now - progressData.lastUpdate > 100) { // Update every 100ms
            const elapsed = now - progressData.startTime;
            const speed = progressData.processedBytes / elapsed * 1000 / 1024 / 1024;
            
            process.stdout.write(
              `\rFiles: ${progress.filesCompleted}/${progress.totalFiles} | ` +
              `Progress: ${progress.percentage.toFixed(1)}% | ` +
              `Speed: ${speed.toFixed(1)} MB/s`
            );
            
            progressData.lastUpdate = now;
          }
        },
        onFileComplete: (result) => {
          console.log(`\n✓ Completed: ${path.basename(result.filePath)}`);
        }
      }
    );
    
    console.log('\n\nBatch processing complete!');
    
    // Cache statistics
    const cacheStats = client.getCacheStats();
    console.log('\nCache statistics:');
    console.log(`  Entries: ${cacheStats.totalEntries}`);
    console.log(`  Size: ${formatSize(cacheStats.sizeBytes)}`);
    
  } catch (error) {
    console.error('\nError:', error.message);
    if (error.code) {
      console.error('Error code:', error.code);
    }
  } finally {
    client.destroy();
  }
}

// Run the example
main().catch(console.error);