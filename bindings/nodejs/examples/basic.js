/**
 * Basic usage example for AniDB Client
 */

const { AniDBClient, HashAlgorithm } = require('anidb-client');
const path = require('path');

async function main() {
  // Create client with default configuration
  const client = new AniDBClient();
  
  try {
    console.log('AniDB Client Version:', require('anidb-client').version);
    console.log('');
    
    // Example file path (replace with your actual file)
    const filePath = process.argv[2] || './sample.mkv';
    
    console.log('Processing file:', filePath);
    console.log('');
    
    // Process file with ED2K hash (default)
    console.log('1. Processing with ED2K hash...');
    const result1 = await client.processFile(filePath);
    console.log('ED2K Hash:', result1.hashes.ed2k);
    console.log('File Size:', result1.fileSize, 'bytes');
    console.log('Processing Time:', result1.processingTimeMs, 'ms');
    console.log('');
    
    // Process file with multiple hash algorithms
    console.log('2. Processing with multiple hash algorithms...');
    const result2 = await client.processFile(filePath, {
      algorithms: ['ed2k', 'crc32', 'md5', 'sha1'],
      enableProgress: true
    });
    
    console.log('Hash Results:');
    for (const [algo, hash] of Object.entries(result2.hashes)) {
      console.log(`  ${algo.toUpperCase()}: ${hash}`);
    }
    console.log('');
    
    // Calculate a single hash
    console.log('3. Calculating only CRC32...');
    const crc32 = await client.calculateHash(filePath, 'crc32');
    console.log('CRC32:', crc32);
    console.log('');
    
    // Check cache
    console.log('4. Checking cache...');
    const isCached = client.isCached(filePath, 'ed2k');
    console.log('Is ED2K hash cached?', isCached);
    
    const stats = client.getCacheStats();
    console.log('Cache Statistics:');
    console.log('  Total Entries:', stats.totalEntries);
    console.log('  Size:', stats.sizeBytes, 'bytes');
    console.log('');
    
    // Try to identify anime (will fail without network/AniDB credentials)
    if (result1.hashes.ed2k) {
      console.log('5. Attempting anime identification...');
      try {
        const animeInfo = await client.identifyFile(result1.hashes.ed2k, result1.fileSize);
        if (animeInfo) {
          console.log('Anime Identified:');
          console.log('  Title:', animeInfo.title);
          console.log('  Episode:', animeInfo.episodeNumber);
          console.log('  Confidence:', (animeInfo.confidence * 100).toFixed(1) + '%');
          console.log('  Source:', animeInfo.source);
        } else {
          console.log('No anime identification available (network/credentials required)');
        }
      } catch (error) {
        console.log('Identification failed:', error.message);
      }
    }
    
  } catch (error) {
    console.error('Error:', error.message);
    if (error.code) {
      console.error('Error Code:', error.code);
    }
  } finally {
    // Clean up
    client.destroy();
  }
}

// Run the example
main().catch(console.error);