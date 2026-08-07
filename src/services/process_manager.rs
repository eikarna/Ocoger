//! Lifecycle supervisor for the `opencode` child process.
// Note: graceful terminate -> 3s timeout -> kill is POSIX-shaped; Windows
// requires a per-platform strategy. See TODO.md §5.
