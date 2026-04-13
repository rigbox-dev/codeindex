# codeindex

Semantic code indexing and retrieval for AI agents.

AI coding tools spend tokens and round-trips searching codebases with grep and glob. codeindex pre-indexes your project and returns ranked code **regions** (not just files) with their dependency connections in a single query.

## Quick Start

```bash
# Build from source
cargo install --path crates/cli

# In any project
codeindex init
codeindex index
codeindex query "where is authentication handled"
```

## What It Does

codeindex parses your source code with [tree-sitter](https://tree-sitter.github.io/tree-sitter/), extracts meaningful regions (functions, classes, structs, traits, interfaces, methods), maps their dependencies, and stores everything in a fast local index.

Queries return:
- **Ranked regions** with file paths, line numbers, and signatures
- **Dependency graphs** showing what each region calls, what calls it, and what types it references
- **Optional LLM summaries** describing what each region does

## Supported Languages

| Language | Regions | Dependencies |
|----------|---------|-------------|
| Rust | functions, structs, enums, traits, impl blocks, methods | use imports, function calls, trait impls |
| TypeScript/JavaScript | functions, classes, interfaces, methods | import statements |
| Python | functions, classes, methods | from/import statements |
| Go | functions, methods, structs, interfaces | import declarations |

Adding a language means implementing one trait with two methods. The plugin architecture uses tree-sitter grammars.

## Commands

```bash
codeindex init                          # Initialize index in current project
codeindex index                         # Index (or re-index) the project
codeindex index --incremental           # Only process changed files
codeindex status                        # Show index health

# Querying
codeindex query "auth handler"          # Natural language search
codeindex query ":symbol AuthRequest"   # Direct symbol lookup
codeindex query ":deps src/auth.rs::login"  # Dependency graph
codeindex query ":file src/auth.rs"     # All regions in a file

# Options
codeindex query "..." --top 10          # More results
codeindex query "..." --depth 2         # Deeper dependency expansion
codeindex query "..." --json            # Machine-readable output
codeindex query "..." --format compact  # One line per result
```

## AI Agent Integration

### MCP Server (Claude Code)

codeindex ships as an MCP server for native integration with Claude Code and other MCP-compatible tools.

```json
{
  "mcpServers": {
    "codeindex": {
      "command": "codeindex",
      "args": ["mcp-server"]
    }
  }
}
```

This exposes three tools:
- `codeindex_query` — search the codebase
- `codeindex_deps` — dependency graph for a symbol
- `codeindex_status` — index health check

### CLI (Codex, any agent with shell access)

Any agent that can run shell commands can use:

```bash
codeindex query "where is the database connection pool" --json
```

## How It Works

### Indexing

1. Walks the project (respects `.gitignore`)
2. Parses each file with tree-sitter
3. Extracts regions (functions, classes, etc.) and dependencies (imports, calls, type references)
4. Stores metadata in SQLite, embeddings in an HNSW vector index
5. Optionally generates LLM summaries via Claude Haiku

### Querying

1. Parses the query (natural language vs structured)
2. Runs keyword search (SQLite FTS5) and semantic search (HNSW) in parallel
3. Fuses results with Reciprocal Rank Fusion
4. Expands the dependency graph around top results
5. Optionally re-ranks with Haiku for explanation

### Index Freshness

- **File watcher daemon**: `codeindex watch` monitors for changes with debounced re-indexing
- **Git hooks**: `codeindex init --git-hooks` installs post-checkout/merge/commit hooks

## Architecture

```
codeindex/
├── crates/
│   ├── core/               # Library: storage, indexing, querying, embeddings
│   ├── tree-sitter-langs/  # Language plugins (Rust, TS, Python, Go)
│   ├── cli/                # CLI binary
│   ├── mcp-server/         # MCP server binary
│   └── daemon/             # File watcher + git hooks
```

Core library + thin frontends. The CLI, MCP server, and daemon all share the same on-disk index via file-based locking.

## Configuration

```bash
codeindex config set embedding.provider voyage   # Use Voyage API for embeddings
codeindex config set summary.enabled true        # Enable Haiku summaries
```

Config lives at `.codeindex/config.json`. Works with zero configuration — defaults to a local embedding model with no API keys required.

| Setting | Default | Options |
|---------|---------|---------|
| `embedding.provider` | `local` | `local`, `voyage` |
| `summary.enabled` | `false` | `true`, `false` |
| `summary.model` | `claude-haiku-4-5-20251001` | Any Anthropic model |
| `daemon.debounce_ms` | `500` | Any integer |

## Performance

Tested on a 475-file Go project (rigbox-sidecar):

| Metric | Result |
|--------|--------|
| Full index | 1.2s |
| Regions extracted | 5,593 |
| Dependencies mapped | 2,095 |
| Symbol lookup | <10ms |

## Building

```bash
git clone https://github.com/rigbox-dev/codeindex.git
cd codeindex
cargo build --release
cargo test --workspace
```

The release binary is at `target/release/codeindex`.

## License

MIT
