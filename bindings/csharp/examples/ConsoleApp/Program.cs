using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using AniDBClient;
using Spectre.Console;

namespace ConsoleApp
{
    class Program
    {
        static async Task<int> Main(string[] args)
        {
            AnsiConsole.Write(
                new FigletText("AniDB Client")
                    .LeftJustified()
                    .Color(Color.Blue));

            if (args.Length == 0)
            {
                ShowUsage();
                return 1;
            }

            try
            {
                return await RunCommand(args);
            }
            catch (Exception ex)
            {
                AnsiConsole.WriteException(ex, ExceptionFormats.ShortenEverything);
                return 1;
            }
        }

        static void ShowUsage()
        {
            AnsiConsole.WriteLine();
            AnsiConsole.MarkupLine("[yellow]Usage:[/]");
            AnsiConsole.MarkupLine("  ConsoleApp hash <file> [algorithm]     - Calculate hash for a file");
            AnsiConsole.MarkupLine("  ConsoleApp process <file> [algorithms]  - Process a file with multiple algorithms");
            AnsiConsole.MarkupLine("  ConsoleApp batch <directory>            - Process all files in a directory");
            AnsiConsole.MarkupLine("  ConsoleApp identify <ed2k> <size>       - Identify anime by ED2K hash and size");
            AnsiConsole.MarkupLine("  ConsoleApp cache stats                  - Show cache statistics");
            AnsiConsole.MarkupLine("  ConsoleApp cache clear                  - Clear the cache");
            AnsiConsole.WriteLine();
            AnsiConsole.MarkupLine("[grey]Algorithms: ED2K, CRC32, MD5, SHA1, TTH[/]");
        }

        static async Task<int> RunCommand(string[] args)
        {
            var command = args[0].ToLower();

            // Create client with custom configuration
            var config = new ClientConfiguration
            {
                CacheDirectory = Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                    "AniDBClient",
                    "Cache"),
                EnableDebugLogging = false
            };

            using var client = new AniDBClient.AniDBClient(config);

            switch (command)
            {
                case "hash":
                    return await HashCommand(client, args);
                
                case "process":
                    return await ProcessCommand(client, args);
                
                case "batch":
                    return await BatchCommand(client, args);
                
                case "identify":
                    return await IdentifyCommand(client, args);
                
                case "cache":
                    return await CacheCommand(client, args);
                
                default:
                    AnsiConsole.MarkupLine($"[red]Unknown command: {command}[/]");
                    ShowUsage();
                    return 1;
            }
        }

        static async Task<int> HashCommand(AniDBClient.AniDBClient client, string[] args)
        {
            if (args.Length < 2)
            {
                AnsiConsole.MarkupLine("[red]Error: File path required[/]");
                return 1;
            }

            var filePath = args[1];
            if (!File.Exists(filePath))
            {
                AnsiConsole.MarkupLine($"[red]Error: File not found: {filePath}[/]");
                return 1;
            }

            var algorithm = args.Length > 2 ? ParseAlgorithm(args[2]) : HashAlgorithm.ED2K;

            await AnsiConsole.Status()
                .StartAsync($"Calculating {algorithm} hash...", async ctx =>
                {
                    var hash = client.CalculateHash(filePath, algorithm);
                    
                    AnsiConsole.WriteLine();
                    var table = new Table();
                    table.AddColumn("File");
                    table.AddColumn("Algorithm");
                    table.AddColumn("Hash");
                    
                    table.AddRow(
                        Path.GetFileName(filePath),
                        algorithm.ToString(),
                        $"[green]{hash}[/]");
                    
                    AnsiConsole.Write(table);
                });

            return 0;
        }

        static async Task<int> ProcessCommand(AniDBClient.AniDBClient client, string[] args)
        {
            if (args.Length < 2)
            {
                AnsiConsole.MarkupLine("[red]Error: File path required[/]");
                return 1;
            }

            var filePath = args[1];
            if (!File.Exists(filePath))
            {
                AnsiConsole.MarkupLine($"[red]Error: File not found: {filePath}[/]");
                return 1;
            }

            // Parse algorithms
            var algorithms = args.Length > 2 
                ? args.Skip(2).Select(ParseAlgorithm).ToArray()
                : new[] { HashAlgorithm.ED2K, HashAlgorithm.CRC32, HashAlgorithm.MD5 };

            var options = new ProcessingOptions
            {
                Algorithms = algorithms,
                EnableProgress = true
            };

            FileResult? result = null;

            await AnsiConsole.Progress()
                .StartAsync(async ctx =>
                {
                    var task = ctx.AddTask($"[green]Processing {Path.GetFileName(filePath)}[/]");
                    
                    options.ProgressCallback = info =>
                    {
                        task.Value = info.Percentage;
                    };
                    
                    result = await client.ProcessFileAsync(filePath, options);
                });

            if (result != null)
            {
                AnsiConsole.WriteLine();
                
                var table = new Table();
                table.AddColumn("Property");
                table.AddColumn("Value");
                
                table.AddRow("File", Path.GetFileName(result.FilePath));
                table.AddRow("Size", $"{result.FileSize:N0} bytes");
                table.AddRow("Status", result.Status.ToString());
                table.AddRow("Processing Time", $"{result.ProcessingTime.TotalMilliseconds:F2} ms");
                table.AddRow("From Cache", result.FromCache ? "[green]Yes[/]" : "[yellow]No[/]");
                
                AnsiConsole.Write(table);
                
                AnsiConsole.WriteLine();
                AnsiConsole.MarkupLine("[bold]Hashes:[/]");
                
                var hashTable = new Table();
                hashTable.AddColumn("Algorithm");
                hashTable.AddColumn("Hash Value");
                
                foreach (var hash in result.Hashes)
                {
                    hashTable.AddRow(hash.Algorithm.ToString(), $"[cyan]{hash.Value}[/]");
                }
                
                AnsiConsole.Write(hashTable);
            }

            return 0;
        }

        static async Task<int> BatchCommand(AniDBClient.AniDBClient client, string[] args)
        {
            if (args.Length < 2)
            {
                AnsiConsole.MarkupLine("[red]Error: Directory path required[/]");
                return 1;
            }

            var directory = args[1];
            if (!Directory.Exists(directory))
            {
                AnsiConsole.MarkupLine($"[red]Error: Directory not found: {directory}[/]");
                return 1;
            }

            var files = Directory.GetFiles(directory, "*", SearchOption.AllDirectories)
                .Where(f => new FileInfo(f).Length > 0)
                .ToArray();

            if (files.Length == 0)
            {
                AnsiConsole.MarkupLine("[yellow]No files found in directory[/]");
                return 0;
            }

            AnsiConsole.MarkupLine($"[green]Found {files.Length} files to process[/]");

            var options = new BatchOptions
            {
                Algorithms = new[] { HashAlgorithm.ED2K },
                MaxConcurrent = 4,
                ContinueOnError = true,
                EnableProgress = true
            };

            BatchResult? result = null;

            await AnsiConsole.Progress()
                .StartAsync(async ctx =>
                {
                    var task = ctx.AddTask($"[green]Processing batch[/]", maxValue: files.Length);
                    
                    options.ProgressCallback = info =>
                    {
                        if (info.FilesCompleted.HasValue && info.TotalFiles.HasValue)
                        {
                            task.Value = info.FilesCompleted.Value;
                            task.Description = $"[green]Processing: {info.CurrentFile ?? "..."}[/]";
                        }
                    };
                    
                    result = await client.ProcessBatchAsync(files, options);
                });

            if (result != null)
            {
                AnsiConsole.WriteLine();
                
                var table = new Table();
                table.AddColumn("Metric");
                table.AddColumn("Value");
                
                table.AddRow("Total Files", result.TotalFiles.ToString());
                table.AddRow("Successful", $"[green]{result.SuccessfulFiles}[/]");
                table.AddRow("Failed", result.FailedFiles > 0 ? $"[red]{result.FailedFiles}[/]" : "0");
                table.AddRow("Total Time", $"{result.TotalTime.TotalSeconds:F2} seconds");
                table.AddRow("Average Time", $"{result.TotalTime.TotalMilliseconds / result.TotalFiles:F2} ms/file");
                
                AnsiConsole.Write(table);
            }

            return 0;
        }

        static async Task<int> IdentifyCommand(AniDBClient.AniDBClient client, string[] args)
        {
            if (args.Length < 3)
            {
                AnsiConsole.MarkupLine("[red]Error: ED2K hash and file size required[/]");
                return 1;
            }

            var ed2kHash = args[1];
            if (!long.TryParse(args[2], out var fileSize))
            {
                AnsiConsole.MarkupLine("[red]Error: Invalid file size[/]");
                return 1;
            }

            AnimeInfo? info = null;

            await AnsiConsole.Status()
                .StartAsync("Identifying anime...", async ctx =>
                {
                    info = await client.IdentifyFileAsync(ed2kHash, fileSize);
                });

            if (info != null)
            {
                AnsiConsole.WriteLine();
                
                var table = new Table();
                table.AddColumn("Property");
                table.AddColumn("Value");
                
                table.AddRow("Anime ID", info.AnimeId.ToString());
                table.AddRow("Title", $"[cyan]{info.Title}[/]");
                table.AddRow("Episode", info.EpisodeNumber.ToString());
                table.AddRow("Episode ID", info.EpisodeId.ToString());
                table.AddRow("Confidence", $"{info.Confidence:P0}");
                table.AddRow("Source", info.Source.ToString());
                
                AnsiConsole.Write(table);
            }
            else
            {
                AnsiConsole.MarkupLine("[yellow]No anime information found for this file[/]");
            }

            return 0;
        }

        static async Task<int> CacheCommand(AniDBClient.AniDBClient client, string[] args)
        {
            if (args.Length < 2)
            {
                AnsiConsole.MarkupLine("[red]Error: Subcommand required (stats or clear)[/]");
                return 1;
            }

            var subcommand = args[1].ToLower();

            switch (subcommand)
            {
                case "stats":
                    var stats = client.GetCacheStatistics();
                    
                    var table = new Table();
                    table.AddColumn("Metric");
                    table.AddColumn("Value");
                    
                    table.AddRow("Total Entries", stats.TotalEntries.ToString("N0"));
                    table.AddRow("Cache Size", FormatBytes(stats.SizeInBytes));
                    table.AddRow("Hit Rate", $"{stats.HitRate:P0}");
                    
                    AnsiConsole.Write(table);
                    break;
                
                case "clear":
                    await AnsiConsole.Status()
                        .StartAsync("Clearing cache...", async ctx =>
                        {
                            client.ClearCache();
                            await Task.Delay(500); // Visual feedback
                        });
                    
                    AnsiConsole.MarkupLine("[green]Cache cleared successfully[/]");
                    break;
                
                default:
                    AnsiConsole.MarkupLine($"[red]Unknown cache command: {subcommand}[/]");
                    return 1;
            }

            return 0;
        }

        static HashAlgorithm ParseAlgorithm(string name)
        {
            return name.ToUpper() switch
            {
                "ED2K" => HashAlgorithm.ED2K,
                "CRC32" => HashAlgorithm.CRC32,
                "MD5" => HashAlgorithm.MD5,
                "SHA1" => HashAlgorithm.SHA1,
                "TTH" => HashAlgorithm.TTH,
                _ => throw new ArgumentException($"Unknown algorithm: {name}")
            };
        }

        static string FormatBytes(long bytes)
        {
            string[] sizes = { "B", "KB", "MB", "GB", "TB" };
            double len = bytes;
            int order = 0;
            
            while (len >= 1024 && order < sizes.Length - 1)
            {
                order++;
                len /= 1024;
            }

            return $"{len:F2} {sizes[order]}";
        }
    }
}