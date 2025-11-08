using System;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
using AniDBClient.Native;

namespace AniDBClient
{
    /// <summary>
    /// Safe handle for AniDB client instances
    /// </summary>
    public sealed class SafeClientHandle : SafeHandleZeroOrMinusOneIsInvalid
    {
        private SafeClientHandle() : base(true) { }

        protected override bool ReleaseHandle()
        {
            if (!IsInvalid)
            {
                var result = NativeMethods.anidb_client_destroy(handle);
                return result == AniDBResult.Success;
            }
            return true;
        }
    }

    /// <summary>
    /// Safe handle for AniDB operation instances
    /// </summary>
    public sealed class SafeOperationHandle : SafeHandleZeroOrMinusOneIsInvalid
    {
        private SafeOperationHandle() : base(true) { }

        protected override bool ReleaseHandle()
        {
            if (!IsInvalid)
            {
                var result = NativeMethods.anidb_operation_destroy(handle);
                return result == AniDBResult.Success;
            }
            return true;
        }
    }

    /// <summary>
    /// Safe handle for AniDB batch instances
    /// </summary>
    public sealed class SafeBatchHandle : SafeHandleZeroOrMinusOneIsInvalid
    {
        private SafeBatchHandle() : base(true) { }

        protected override bool ReleaseHandle()
        {
            if (!IsInvalid)
            {
                var result = NativeMethods.anidb_batch_destroy(handle);
                return result == AniDBResult.Success;
            }
            return true;
        }
    }

    /// <summary>
    /// Safe handle for native memory allocations
    /// </summary>
    internal sealed class SafeNativeMemoryHandle : SafeHandleZeroOrMinusOneIsInvalid
    {
        private readonly Action<IntPtr>? _freeFunction;

        public SafeNativeMemoryHandle(Action<IntPtr>? freeFunction = null) : base(true)
        {
            _freeFunction = freeFunction;
        }

        protected override bool ReleaseHandle()
        {
            if (!IsInvalid)
            {
                if (_freeFunction != null)
                {
                    _freeFunction(handle);
                }
                else
                {
                    // Default to freeing as a string
                    NativeMethods.anidb_free_string(handle);
                }
            }
            return true;
        }
    }

    /// <summary>
    /// Safe handle for file results
    /// </summary>
    internal sealed class SafeFileResultHandle : SafeHandleZeroOrMinusOneIsInvalid
    {
        private SafeFileResultHandle() : base(true) { }

        protected override bool ReleaseHandle()
        {
            if (!IsInvalid)
            {
                NativeMethods.anidb_free_file_result(handle);
            }
            return true;
        }
    }

    /// <summary>
    /// Safe handle for batch results
    /// </summary>
    internal sealed class SafeBatchResultHandle : SafeHandleZeroOrMinusOneIsInvalid
    {
        private SafeBatchResultHandle() : base(true) { }

        protected override bool ReleaseHandle()
        {
            if (!IsInvalid)
            {
                NativeMethods.anidb_free_batch_result(handle);
            }
            return true;
        }
    }

    /// <summary>
    /// Safe handle for anime info
    /// </summary>
    internal sealed class SafeAnimeInfoHandle : SafeHandleZeroOrMinusOneIsInvalid
    {
        private SafeAnimeInfoHandle() : base(true) { }

        protected override bool ReleaseHandle()
        {
            if (!IsInvalid)
            {
                NativeMethods.anidb_free_anime_info(handle);
            }
            return true;
        }
    }
}