# Rerere (Reuse Recorded Resolution) Design for Jujutsu

## Implementation Status

✅ **IMPLEMENTED** - The rerere feature is now available in Jujutsu!

Enable it with:
```toml
[rerere]
enabled = true
```

## Overview

This document describes the design and implementation of a rerere-like feature in Jujutsu. Rerere allows jj to record how merge conflicts were resolved and automatically apply the same resolution when the same conflict appears again.

## Background

Jujutsu already has excellent conflict handling:
- Conflicts are stored as first-class objects in commits
- Conflict resolutions automatically propagate to descendant commits during rebasing
- The algebraic conflict simplification ensures clean conflict representations

However, jj lacks cross-branch resolution memory. When you encounter the same conflict in different rebasing contexts (e.g., when merging the same changes through different paths), you must resolve it manually each time.

## Goals

1. Record conflict resolutions automatically when users resolve conflicts
2. Automatically apply recorded resolutions when the same conflict appears
3. Integrate seamlessly with jj's existing conflict model
4. Support jj's multi-way conflicts (not just 3-way)
5. Minimal configuration and user intervention

## Non-Goals

1. Complex CLI interface (git's `rerere forget`, `rerere clear`, etc.)
2. Auto-staging of resolved files (jj handles working copy differently than git)
3. Partial conflict matching (only exact matches)
4. AST-level or semantic merging (better handled by specialized tools like [Mergiraf](https://mergiraf.org/))

## Implementation Status

✅ **This feature has been implemented and is available when `rerere.enabled = true` is set in the configuration.**

## Implementation

### Storage

Resolution cache is stored in `.jj/repo/resolution_cache/` with the following structure:

```
.jj/repo/resolution_cache/
├── <conflict-hash>/
│   ├── conflict      # The normalized conflict content
│   └── resolution    # The recorded resolution
└── ...
```

- Uses BLAKE2b-512 for hashing (consistent with jj's other content-addressed storage)
- Stores raw content, not serialized data structures
- No metadata files initially (can be added later if needed)
- **Atomic writes**: Both directory creation and file updates use atomic operations to prevent corruption during concurrent access

### Conflict Normalization

Before hashing, conflicts are normalized to ensure the same logical conflict produces the same hash:

1. Write a header with the number of conflict sides
2. Sort conflict sides deterministically (by content)
3. Path-independent hashing (same conflict in different files gets same ID)
4. Each side is wrapped with `SIDE_START` and `SIDE_END` markers

Example normalized format:
```
CONFLICT:3
SIDE_START
base content
SIDE_END
SIDE_START
left content
SIDE_END
SIDE_START
right content
SIDE_END
```

### Resolution Recording

When a conflict is resolved:

1. During conflict resolution in `update_conflict_from_materialized_value`
2. Calculate the normalized conflict hash
3. Store the conflict content and resolution in the cache
4. Continue with normal jj operations

### Resolution Replay

During operations that may create conflicts (rebase, merge):

1. When a conflict is detected in `merge_tree_values_with_cache`
2. Check if a resolution exists in the cache
3. If found, apply the resolution automatically
4. The file is automatically marked as resolved

### Integration Points

1. **`lib/src/conflicts.rs`**: Records resolutions when conflicts are resolved
2. **`lib/src/rewrite.rs`**: Attaches resolution cache to trees during rebasing operations
3. **`lib/src/merged_tree.rs`**:
   - `MergedTree` struct contains optional resolution cache
   - `resolve()` method uses internal cache if present
   - `with_resolution_cache()` method sets the cache
4. **`lib/src/repo.rs`**: Manages ResolutionCache lifecycle and configuration

### Configuration

Single configuration option:
```toml
[rerere]
enabled = false  # Default to false
```

### Core Data Structures

```rust
// lib/src/resolution_cache.rs
pub struct ResolutionCache {
    store_path: PathBuf,
    enabled: bool,
}

pub struct ConflictId([u8; 64]); // BLAKE2b-512 hash

/// Result of a resolution cache operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionCacheResult {
    /// A new conflict preimage was recorded.
    RecordedPreimage,
    /// A resolution was recorded.
    RecordedResolution,
    /// A cached resolution was applied.
    AppliedCachedResolution,
    /// No action was taken.
    NoAction,
}

impl ResolutionCache {
    pub fn record_resolution(
        &self,
        path: &RepoPath,
        conflict: &MaterializedFileConflictValue,
        resolution: &[u8],
    ) -> BackendResult<()>;

    pub fn get_resolution(
        &self,
        path: &RepoPath,
        conflict: &MaterializedFileConflictValue,
    ) -> BackendResult<Option<Vec<u8>>>;

    pub fn get_resolution_for_content(
        &self,
        path: &RepoPath,
        conflict_content: &Merge<BString>,
    ) -> BackendResult<Option<Vec<u8>>>;

    pub fn clear(&self) -> io::Result<()>;

    pub fn is_enabled(&self) -> bool;
}

// lib/src/merged_tree.rs
pub struct MergedTree {
    trees: Merge<Tree>,
    resolution_cache_stats: ResolutionCacheStats,
    resolution_cache: Option<Arc<ResolutionCache>>,
}

impl MergedTree {
    /// Set the resolution cache to use for conflict resolution
    pub fn with_resolution_cache(mut self, resolution_cache: Arc<ResolutionCache>) -> Self;

    /// Resolve conflicts using the internal resolution cache if set
    pub fn resolve(&self) -> BackendResult<MergedTree>;
}
```

## Current Implementation Details

### Completed Features

1. **Core Resolution Cache** (`lib/src/resolution_cache.rs`)
   - Conflict normalization with consistent BLAKE2b-512 hashing
   - Storage and retrieval of conflict resolutions
   - Path-independent conflict identification
   - Support for multi-way conflicts (not just 3-way)
   - Garbage collection support with time-based expiration
   - `ResolutionCacheResult` enum for tracking operation outcomes

2. **Integration Points**
   - **Conflict Recording**: Integrated into `update_conflict_from_materialized_value` in `lib/src/conflicts.rs`
   - **Conflict Resolution**: Added `check_for_cached_resolution` in `lib/src/conflicts.rs`
   - **Tree Merging**:
     - `MergedTree` struct contains optional resolution cache field
     - Trees carry their resolution cache through operations
     - `resolve()` method uses internal cache automatically
     - `with_resolution_cache()` method allows setting cache on trees
   - **Rebase Operations**: Trees are constructed with resolution cache in `lib/src/rewrite.rs`
   - **Repository**: Added `resolution_cache()` method to `Repo` trait
   - **Garbage Collection**: Integrated into `jj util gc` command in `cli/src/commands/util/gc.rs`

3. **Configuration**
   - Single setting: `rerere.enabled = true/false` (defaults to false)
   - Lazy initialization of resolution cache only when enabled
   - No additional configuration options (intentionally simple)

4. **User Feedback**
   - Resolution cache statistics are displayed after operations that may apply or record resolutions
   - Commands that display statistics: `rebase`, `new`, `squash`, `resolve`
   - Statistics show number of applied cached resolutions and newly recorded resolutions
   - Feedback is implemented via `print_resolution_cache_stats` in `cli_util.rs`

5. **Testing**
   - Comprehensive unit tests in `resolution_cache.rs`
   - Integration test in `test_new_command.rs` demonstrating cross-branch resolution reuse
   - GC integration test in `test_util_command.rs`

### How It Works

1. **Recording**: When a conflict is resolved (file no longer has conflict markers), the resolution is automatically recorded
2. **Replaying**: When the same conflict appears again (even in different files or branches), the cached resolution is automatically applied
3. **Normalization**: Conflicts are normalized by sorting sides deterministically, ensuring order-independent matching

## Usage Example

To enable rerere in your jujutsu repository:

```toml
# In your config file (~/.jjconfig.toml or .jj/repo/config.toml)
[rerere]
enabled = true
```

Once enabled, jujutsu will automatically:
1. Record resolutions when you resolve conflicts
2. Apply cached resolutions when the same conflict appears again
3. Display statistics about recorded and applied resolutions

Example workflow:
```bash
# Create a merge with conflicts
jj new branch1 branch2
# Output: "Warning: There are unresolved conflicts at these paths:..."

# Resolve the conflicts manually
# ... edit files to resolve conflicts ...

# Commit the resolution (automatically recorded)
jj commit -m "Resolved merge"
# Output: "Recorded 1 new conflict resolutions"

# Later, when the same conflict appears in a different context
jj rebase -b some_commit -d other_commit
# Output: "Applied 1 cached conflict resolutions"
# The conflict is automatically resolved!
```

### Commands that Display Resolution Cache Statistics

The following commands will show resolution cache statistics when relevant:
- `jj new` - When creating merge commits
- `jj rebase` - When rebasing commits
- `jj resolve` - When resolving conflicts
- `jj squash` - When squashing commits

Statistics are only shown when resolutions are actually recorded or applied.

## Testing

The implementation includes comprehensive tests for:
- **Core functionality**:
  - Conflict normalization consistency
  - Order independence of conflict sides
  - Multi-way conflict support (5+ sides)
  - Empty side handling
  - Path independence
  - Resolution recording and retrieval
  - Disabled cache behavior
- **Special cases**:
  - Binary file conflicts
  - Executable bit conflicts
  - Empty file conflicts
  - Mixed text/binary conflicts
- **Complex scenarios**:
  - Rename/move conflicts
  - File vs directory conflicts
  - Multiple interdependent conflicts
  - Nested conflicts (conflicts within conflicts)
  - Copy detection scenarios
- **System behavior**:
  - Concurrent access handling
  - Cross-platform path normalization
  - Integration with external merge tools
  - Garbage collection with time-based expiration

## Implementation Decisions

### Simplified API Design
The implementation attaches the resolution cache to the MergedTree object rather than passing it as a parameter:
- `tree.with_resolution_cache(cache).resolve()` - Resolution cache is set on the tree object
- `MergedTree` struct contains an optional `resolution_cache` field
- Internal functions like `merge_trees()` still accept the cache as a parameter for flexibility
- The cache is automatically propagated through tree operations

This design:
- Reduces the need to pass resolution cache through multiple function calls
- Allows trees to carry their resolution context with them
- Makes the API cleaner at call sites
- Maintains backward compatibility (trees without cache work normally)

### Atomic Storage Operations
The resolution cache uses atomic operations to ensure data integrity during concurrent access:
- **Directory creation**: Uses temporary directories with atomic rename to prevent partial writes
- **File updates**: Uses the existing `persist_content_addressed_temp_file` function for atomic file operations
- **Race condition handling**: When multiple processes try to create the same conflict directory, the implementation gracefully handles the race by updating only the resolution file


## Current Limitations

1. **Exact matching**: Only identical conflicts (after normalization) are matched
2. **No conflict context**: Resolution matching ignores surrounding file context
3. **Single resolution per conflict**: No support for multiple valid resolutions
4. **No cross-repository sharing**: Resolution cache is local to each repository
5. **File conflicts only**: Rerere currently only supports regular file conflicts, not symlink conflicts, directory/file conflicts, or other special file types

## Garbage Collection

Resolution cache entries are cleaned up through `jj util gc`, following jujutsu's existing GC patterns:

- **Manual cleanup**: Run `jj util gc` to remove old resolutions (default: older than 14 days)
- **Immediate cleanup**: Use `jj util gc --expire=now` to remove all resolutions
- **Time-based**: Uses file modification time; resolutions are touched when used to prevent premature removal
- **Integrated**: Part of the overall GC process alongside operation and object cleanup

No automatic garbage collection occurs - users must explicitly run `jj util gc`.

## Alternatives Considered

1. **Storing resolutions in commits**: Would complicate the commit model and make history less portable
2. **Using git's rerere directly**: Would only work with git backend, not jj's native model
3. **Fuzzy matching**: Too complex for initial implementation, could lead to incorrect resolutions
4. **AST-level merging**: Better handled by specialized tools like Mergiraf

## Future Extensions

1. **Automatic garbage collection** - Run GC automatically based on cache size or age thresholds
2. **Smart conflict matching** - Understanding moved code blocks and semantic changes
3. **Statistics and debugging commands** - `jj debug rerere-stats`, `jj debug rerere-list`, etc.
