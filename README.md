# forceops-rs

[![CI](https://github.com/domsleee/forceops-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/domsleee/forceops-rs/actions/workflows/ci.yml)

Forcefully delete files and directories by killing processes holding locks. A fast Rust port of [forceops](https://github.com/domsleee/forceops).

**Windows only** — Linux doesn't need this because [unlink](https://man7.org/linux/man-pages/man2/unlink.2.html) allows deleting files even when they're in use.

## Features

- 🚀 **Fast** — Native Rust binary with no runtime dependencies
- 🔍 **Smart lock detection** — Uses Windows Restart Manager API for files, process PEB reading for directories
- ⚡ **Auto-elevation** — Automatically relaunches as admin when needed to kill system processes
- 🔄 **Retry logic** — Configurable retries with delays for stubborn locks
- 📁 **Recursive deletion** — Handles directories and their contents

## Installation

### From releases (recommended)

Download the latest `fops.exe` from [releases](https://github.com/domsleee/forceops-rs/releases).

### Build from source

```shell
cargo install --git https://github.com/domsleee/forceops-rs
```

Or clone and build:

```shell
git clone https://github.com/domsleee/forceops-rs
cd forceops-rs
cargo build --release
# Binary at target/release/fops.exe
```

## Usage

### Delete files or directories

```shell
# Delete a single file
fops delete myfile.dll

# Delete a directory recursively
fops rm ./bin/

# Delete multiple items
fops rm file1.txt file2.txt ./build/

# Ignore errors for non-existent files
fops rm -f ./maybe-exists/
```

### Example output

When a process is holding a lock:

```
2024-01-30T10:15:33Z INFO Deleting 'C:\project\bin\myapp.dll'
2024-01-30T10:15:33Z INFO Could not delete file. Found 1 locking process: [12345 - myapp.exe]
2024-01-30T10:15:33Z INFO Killed process 12345
2024-01-30T10:15:33Z INFO Successfully deleted 'C:\project\bin\myapp.dll'
```

When elevation is needed (process owned by another user):

```
2024-01-30T10:15:33Z INFO Could not delete file. Found 1 locking process: [67890 - MyService.exe]
2024-01-30T10:15:33Z WARN Failed to kill process 67890: Access is denied
2024-01-30T10:15:33Z INFO Relaunching as administrator...
2024-01-30T10:15:35Z INFO Successfully deleted as admin
```

### List processes locking a file

```shell
fops list myfile.dll
```

Output (CSV format):
```
ProcessId,ExecutableName,ApplicationName
12345,C:\project\bin\myapp.exe,myapp.exe
```

### CLI options

```
fops delete [OPTIONS] <FILES>...

Arguments:
  <FILES>...  Files or directories to delete

Options:
  -f, --force            Ignore nonexistent files and arguments
  -e, --disable-elevate  Do not attempt to elevate if the file can't be deleted
  -d, --retry-delay <MS> Delay in ms when retrying after killing processes [default: 50]
  -n, --max-retries <N>  Number of retries when deleting a locked file [default: 10]
  -h, --help             Print help
```

## How it works

1. **Try to delete** the file or directory
2. **On failure**, detect which processes hold locks:
   - For files: Uses the [Windows Restart Manager API](https://docs.microsoft.com/en-us/windows/win32/rstmgr/restart-manager-portal)
   - For directories: Enumerates processes and reads their PEB (Process Environment Block) to find working directories
3. **Kill the locking processes** using `TerminateProcess`
4. **Retry the deletion** with configurable delay
5. **If access denied**, relaunch as administrator and retry

## Performance

fops is a native Rust binary that:
- Starts instantly (no runtime initialization)
- Uses direct Windows API calls via [windows-rs](https://github.com/microsoft/windows-rs)
- Compiles with LTO for optimal performance
- Produces a small, self-contained executable (~1.6 MB)

## Comparison with forceops (C#)

| Feature | fops (Rust) | forceops (C#) |
|---------|-------------|---------------|
| Startup time | ~5ms | ~50ms (Native AOT) |
| Binary size | ~1.6 MB | ~15 MB (Native AOT) |
| Dependencies | None | .NET runtime or Native AOT |
| Lock detection | Direct Windows API | LockCheck library |

## Related

- [forceops](https://github.com/domsleee/forceops) — The original C# implementation
- [LockCheck](https://github.com/cklutz/LockCheck) — Library for finding processes locking files

## License

MIT
