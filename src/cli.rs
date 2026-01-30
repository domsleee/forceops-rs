use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "forceops",
    version,
    about = "By hook or by crook, perform operations on files and directories. If they are in use by a process, kill the process."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Delete files or directories recursively
    #[command(visible_aliases = ["rm", "remove"])]
    Delete {
        /// Files or directories to delete
        #[arg(required = true)]
        files: Vec<String>,

        /// Ignore nonexistent files and arguments
        #[arg(short, long)]
        force: bool,

        /// Do not attempt to elevate if the file can't be deleted
        #[arg(short = 'e', long)]
        disable_elevate: bool,

        /// Delay in ms when retrying to delete a file, after killing processes holding a lock
        #[arg(short = 'd', long, default_value = "50")]
        retry_delay: u64,

        /// Number of retries when deleting a locked file
        #[arg(short = 'n', long, default_value = "10")]
        max_retries: u32,
    },

    /// Uses lock detection to output processes using a file or directory
    List {
        /// File or directory to get the locks of
        file_or_directory: String,
    },
}
