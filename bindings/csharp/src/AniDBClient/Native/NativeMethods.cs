using System;
using System.Runtime.InteropServices;
using System.Security;

namespace AniDBClient.Native
{
    /// <summary>
    /// P/Invoke declarations for the AniDB Client Core Library
    /// </summary>
    [SuppressUnmanagedCodeSecurity]
    internal static partial class NativeMethods
    {
        private const string LibraryName = "anidb_client_core";

        #region Library Initialization

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_init(uint abi_version);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern void anidb_cleanup();

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern IntPtr anidb_get_version();

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern uint anidb_get_abi_version();

        #endregion

        #region Client Management

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_client_create(out IntPtr handle);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_client_create_with_config(
            in AniDBConfig config,
            out IntPtr handle);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_client_destroy(IntPtr handle);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_client_get_last_error(
            IntPtr handle,
            IntPtr buffer,
            UIntPtr buffer_size);

        #endregion

        #region File Processing

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_process_file(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string file_path,
            in AniDBProcessOptions options,
            out IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_process_file_async(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string file_path,
            in AniDBProcessOptions options,
            out IntPtr operation);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_operation_get_status(
            IntPtr operation,
            out AniDBStatus status);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_operation_get_result(
            IntPtr operation,
            out IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_operation_cancel(IntPtr operation);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_operation_destroy(IntPtr operation);

        #endregion

        #region Batch Processing

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_process_batch(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPArray, ArraySubType = UnmanagedType.LPUTF8Str)]
            string[] file_paths,
            UIntPtr file_count,
            in AniDBBatchOptions options,
            out IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_process_batch_async(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPArray, ArraySubType = UnmanagedType.LPUTF8Str)]
            string[] file_paths,
            UIntPtr file_count,
            in AniDBBatchOptions options,
            out IntPtr batch);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_batch_get_progress(
            IntPtr batch,
            out UIntPtr completed,
            out UIntPtr total);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_batch_cancel(IntPtr batch);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_batch_destroy(IntPtr batch);

        #endregion

        #region Hash Calculation

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_calculate_hash(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string file_path,
            AniDBHashAlgorithm algorithm,
            IntPtr hash_buffer,
            UIntPtr buffer_size);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_calculate_hash_buffer(
            IntPtr data,
            UIntPtr data_size,
            AniDBHashAlgorithm algorithm,
            IntPtr hash_buffer,
            UIntPtr buffer_size);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern UIntPtr anidb_hash_buffer_size(AniDBHashAlgorithm algorithm);

        #endregion

        #region Cache Management

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_cache_clear(IntPtr handle);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_cache_get_stats(
            IntPtr handle,
            out UIntPtr total_entries,
            out ulong cache_size_bytes);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_cache_check_file(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string file_path,
            AniDBHashAlgorithm algorithm,
            out int is_cached);

        #endregion

        #region Anime Identification

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_identify_file(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string ed2k_hash,
            ulong file_size,
            out IntPtr info);

        #endregion

        #region Memory Management

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern void anidb_free_string(IntPtr str);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern void anidb_free_file_result(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern void anidb_free_batch_result(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern void anidb_free_anime_info(IntPtr info);

        #endregion

        #region Callback Management

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern ulong anidb_register_callback(
            IntPtr handle,
            AniDBCallbackType type,
            IntPtr callback,
            IntPtr user_data);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_unregister_callback(
            IntPtr handle,
            ulong callback_id);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_event_connect(
            IntPtr handle,
            AniDBEventCallback callback,
            IntPtr user_data);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_event_disconnect(IntPtr handle);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern AniDBResult anidb_event_poll(
            IntPtr handle,
            IntPtr events,
            UIntPtr max_events,
            out UIntPtr event_count);

        #endregion

        #region Utility Functions

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern IntPtr anidb_error_string(AniDBResult error);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        internal static extern IntPtr anidb_hash_algorithm_name(AniDBHashAlgorithm algorithm);

        #endregion
    }
}