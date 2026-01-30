# ForceOps-RS Implementation Plan

## Overview

Reimplement [forceops](https://github.com/domsleee/forceops) in Rust with a focus on performance. ForceOps is a Windows CLI tool that forcefully deletes files/directories by detecting and terminating processes holding locks on them, with auto-elevation support.

## Approach

- **Single binary crate** with library modules exposed for reuse
- **Direct Windows API calls** via `windows-rs` for lock detection (Restart Manager API)
- **clap** for CLI argument parsing
- **Performance focus**: zero-copy where possible, minimal allocations, parallel file traversal

## Key Components to Implement

| C# Component | Rust Equivalent | Notes |
|--------------|-----------------|-------|
| LockCheck (external) | `lock_checker` module | Direct Win32 Restart Manager API |
| FileAndDirectoryDeleter | `deleter` module | Recursive delete with retry logic |
| ProcessKiller | `process` module | Win32 TerminateProcess |
| ElevateUtils | `elevation` module | Check admin via GetTokenInformation |
| RelaunchAsElevated | `elevation` module | ShellExecuteW with "runas" |
| ForceOpsContext | `config` module | Configuration struct |
| CLI (System.CommandLine) | clap | `delete`/`rm` and `list` commands |

## Workplan

### Phase 1: Project Setup
- [x] Initialize Cargo project with binary target
- [x] Add dependencies: `windows-rs`, `clap`, `tracing`, `anyhow`
- [x] Create module structure

### Phase 2: Core Windows APIs
- [x] Implement lock detection using Restart Manager API (`RmStartSession`, `RmRegisterResources`, `RmGetList`)
- [x] Implement directory lock detection via process PEB reading (NtQueryInformationProcess)
- [x] Implement process termination (`OpenProcess`, `TerminateProcess`)
- [x] Implement elevation check (`OpenProcessToken`, `GetTokenInformation`)
- [x] Implement re-launch as elevated (`ShellExecuteExW` with "runas")

### Phase 3: File Operations
- [x] Implement `DirectoryUtils` equivalents (path resolution, symlink detection, read-only handling)
- [x] Implement single file deletion with retry loop and lock detection
- [x] Implement recursive directory deletion
- [x] Handle edge cases: symlinks, read-only files, permission errors

### Phase 4: CLI
- [x] Implement `delete`/`rm` command with options: `-f`, `-e`, `-d`, `-n`
- [x] Implement `list` command to show processes locking a file
- [x] Add colored/structured logging via `tracing`
- [x] Implement auto-elevation flow with output tee

### Phase 5: Tests (Ported from C#)

#### Test Utilities (`tests/common/`)
- [x] `test_util.rs` - Port `TestUtil.cs`:
  - `launch_process_in_directory()` - spawn PowerShell in a directory to hold lock
  - `hold_lock_on_file_using_powershell()` - spawn PowerShell holding file lock
  - `get_temporary_file_name()` - generate temp path with GUID
  - `create_temporary_directory()` - create dir with cleanup on drop
- [x] `wrapped_process.rs` - Port `WrappedProcess.cs`: RAII wrapper that kills process on drop
- [x] `test_context.rs` - Port `TestContext.cs`: mock-based test harness with fake logger, mock elevation utils, etc.
- [x] `test_util_stdout.rs` - Port `TestUtilStdout.cs`: stdout capture utilities

#### FileAndDirectoryDeleter Tests (`tests/file_and_directory_deleter_test.rs`)
Port from `FileAndDirectoryDeleterTest.cs`:
- [x] `deleting_directory_open_in_powershell_working_directory` - delete dir held by pwsh CWD
- [x] `deleting_readonly_directory_open_in_powershell_working_directory` - readonly dir held by pwsh
- [x] `deleting_file_open_by_powershell` - delete file with lock held by pwsh
- [x] `deleting_readonly_file_open_by_powershell` - readonly file with lock

#### Program/CLI Tests (`tests/program_test.rs`)
Port from `ProgramTest.cs`:
- [x] `retry_delay_and_max_retries_work` - verify CLI args `--retry-delay` and `--max-retries`
- [x] `delete_multiple_files` - verify deleting multiple files in one command
- [x] `delete_non_existing_file_throws_message` - error message for missing file
- [x] `list_non_existing_file_throws_message` - error message for missing file in list cmd
- [x] `delete_non_existing_file_with_force_succeeds` - -f flag ignores missing files
- [x] `help_command_works` - CLI help works
- [x] `rm_alias_works` - rm command alias works
- [x] `remove_alias_works` - remove command alias works
- [x] `list_command_output_format` - list command CSV output format

#### ListFileOrDirectoryLocks Tests (`tests/list_locks_test.rs`)
Port from `ListFileOrDirectoryLocksTest.cs`:
- [x] `works_for_directory` - list locks on directory
- [x] `works_for_file` - list locks on file
- [x] `file_not_found_error` - error message for missing file

#### Integration Test (`tests/delete_example_test.rs`)
Port from `DeleteExampleTest.cs`:
- [x] `delete_example_works` - end-to-end test using the binary
- [x] `delete_unlocked_file_works` - delete simple file
- [x] `delete_directory_recursively` - delete directory tree

### Phase 6: Polish & Performance
- [ ] Benchmark against C# version
- [ ] Optimize hot paths (parallel directory traversal with `rayon` if beneficial)
- [ ] Add CI workflow for releases

## Dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
windows = { version = "0.58", features = [
    "Win32_System_RestartManager",
    "Win32_System_Threading",
    "Win32_Security",
    "Win32_Foundation",
    "Win32_UI_Shell",
    "Win32_Storage_FileSystem",
] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"

[dev-dependencies]
mockall = "0.13"      # For mocking traits in tests
tempfile = "3"        # For temporary file/directory creation
uuid = { version = "1", features = ["v4"] }  # For unique temp paths
```

## Module Structure

```
forceops-rs/
├── Cargo.toml
├── src/
│   ├── main.rs           # Entry point, CLI setup
│   ├── lib.rs            # Public API for library use
│   ├── cli.rs            # clap command definitions
│   ├── config.rs         # ForceOpsConfig struct
│   ├── deleter.rs        # File/directory deletion logic
│   ├── lock_checker.rs   # Restart Manager API wrapper
│   ├── process.rs        # Process killing
│   ├── elevation.rs      # Admin check + re-launch
│   └── utils.rs          # Path utilities
├── tests/
│   ├── common/
│   │   ├── mod.rs
│   │   ├── test_util.rs
│   │   ├── test_context.rs
│   │   ├── wrapped_process.rs
│   │   └── test_util_stdout.rs
│   ├── file_and_directory_deleter_test.rs
│   ├── program_test.rs
│   ├── list_locks_test.rs
│   └── delete_example_test.rs
```

## Performance Considerations

1. **Restart Manager API** - More efficient than NtQuerySystemInformation for finding process locks
2. **Parallel traversal** - Consider `rayon` for large directories (measure first)
3. **String handling** - Use `OsString`/`OsStr` to avoid UTF-8 conversion overhead on Windows paths
4. **Minimal allocations** - Pre-size vectors, reuse buffers where possible
5. **Release build** - LTO, codegen-units=1, strip symbols

## Exit Codes

| Code | Meaning |
|------|---------|
| 0    | Success |
| 1    | General error |
| 2    | File not found |

## Notes

- Windows-only (like the original)
- The C# version uses Native AOT for fast startup; Rust will be comparable or better
- The original benchmarks show forceops is faster than alternatives - aim to match or beat
