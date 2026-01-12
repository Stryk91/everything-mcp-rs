# everything-mcp-rs

A fast, zero-dependency deployment MCP (Model Context Protocol) server for [Everything Search](https://www.voidtools.com/) written in Rust.

## Features

- **24 specialized search tools** for Claude Desktop and other MCP clients
- **Zero-dependency deployment** - dynamically loads Everything64.dll at runtime
- **Dual mode** - runs as MCP server or standalone CLI
- **Built with rmcp** - official Rust MCP SDK

## Requirements

- Windows (Everything Search is Windows-only)
- [Everything Search](https://www.voidtools.com/) installed and running
- Everything64.dll available (installed with Everything or in PATH)

## Installation

### Build from source

```bash
cargo build --release
```

The binary will be at `target/release/everything-mcp-rs.exe`

### Claude Desktop Configuration

Add to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "everything": {
      "command": "path/to/everything-mcp-rs.exe"
    }
  }
}
```

## Available Tools

### General Search
- `everything_search` - Full search with wildcards, extensions, paths, regex support
- `everything_status` - Check Everything service status and version

### File Type Searches
- `everything_search_ext` - Search by extension(s)
- `everything_search_audio` - Find audio files (mp3, wav, flac, etc.)
- `everything_search_video` - Find video files (mp4, mkv, avi, etc.)
- `everything_search_image` - Find images (jpg, png, gif, etc.)
- `everything_search_doc` - Find documents (pdf, docx, xlsx, etc.)
- `everything_search_code` - Find source code files
- `everything_search_archive` - Find archives (zip, rar, 7z, etc.)
- `everything_search_exe` - Find executables

### Location-Based
- `everything_search_in_folder` - Search within a specific folder
- `everything_search_folders` - Search for folders only

### Date & Size Filters
- `everything_recent` - Recently modified files
- `everything_search_date_created` - Filter by creation date
- `everything_search_date_modified` - Filter by modification date
- `everything_search_size` - Filter by file size
- `everything_search_large` - Find large files

### Advanced
- `everything_search_empty` - Find empty folders
- `everything_search_hidden` - Find hidden files
- `everything_search_content` - Search file contents (slow)
- `everything_search_regex` - Search with regular expressions
- `everything_find_duplicates` - Find duplicate filenames
- `everything_search_exclude` - Search with exclusions
- `everything_search_or` - Search with OR logic

## CLI Mode

Run directly from command line:

```bash
# Search
everything-mcp-rs search "*.rs" -n 20

# Search by extension
everything-mcp-rs ext "rs,toml" -k "mcp"

# Recent files
everything-mcp-rs recent -d 7 -e "rs"

# Large files
everything-mcp-rs large -s 500mb

# Check status
everything-mcp-rs status
```

## Build Optimization

Release builds are optimized for minimal size:
- LTO enabled
- Single codegen unit
- Symbols stripped
- Size-optimized (`opt-level = "z"`)

## License

MIT

## Author

STRYK
