<h1 align="center">error-toon</h1>

<p align="center">
  <strong>Compress verbose browser errors for LLMs. Save 70-90% tokens.</strong>
</p>

<p align="center">
  <a href="https://github.com/adrozdenko/error-toon/actions"><img src="https://img.shields.io/github/actions/workflow/status/anthropics/error-toon/ci.yml?style=flat-square" alt="Build Status"></a>
  <a href="https://crates.io/crates/error-toon"><img src="https://img.shields.io/crates/v/error-toon.svg?style=flat-square" alt="Crates.io"></a>
  <a href="https://github.com/adrozdenko/error-toon/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square" alt="License"></a>
</p>

---

## The Problem

When you copy a browser console error and paste it to an LLM, you're wasting tokens on noise:

```
Warning: validateDOMNesting(...): <p> cannot appear as a descendant of <p>.
    at p
    at MDXContent (http://localhost:6006/node_modules/.cache/sb-vite/deps/chunk-THCTKSJA.js?v=a1b2c3d4:12754:23)
    at MDXProvider (http://localhost:6006/node_modules/.cache/sb-vite/deps/chunk-THCTKSJA.js?v=a1b2c3d4:12629:3)
    at DocsContainer (http://localhost:6006/node_modules/.cache/sb-vite/deps/chunk-QIBLKSSA.js?v=a1b2c3d4:24567:3)
    at ErrorBoundary (http://localhost:6006/node_modules/.cache/sb-vite/deps/chunk-QIBLKSSA.js?v=a1b2c3d4:24123:5)
    at Docs (http://localhost:6006/node_modules/.cache/sb-vite/deps/chunk-QIBLKSSA.js?v=a1b2c3d4:24892:3)
    ... [30 more lines of framework internals]
```

**4,000+ characters** of webpack paths, React internals, and cache hashes — all consumed as tokens before your actual question.

## The Solution

```bash
error-toon
```

```
╭───────────────────────────────────────╮
│ 󰅖 DOM_NESTING                         │
├───────────────────────────────────────┤
│  Guide.mdx:79                         │
│  <p> cannot be descendant of <p>      │
│ frames:                               │
│   MDXContent @ Guide.mdx:79           │
│   DocsContainer @ chunk-QIBLKSSA:24   │
├───────────────────────────────────────┤
│ 📦 4521c → 198c (95% saved)           │
╰───────────────────────────────────────╯
```

**198 characters.** The LLM gets exactly what it needs: error type, location, message, and relevant stack frames.

---

## Quick Start

### Install

```bash
# Cargo (recommended)
cargo install error-toon

# Or build from source
git clone https://github.com/adrozdenko/error-toon
cd error-toon && cargo install --path .
```

### Use

```bash
# 1. Copy an error from your browser console
# 2. Run error-toon
error-toon       # Compresses clipboard, auto-copies result back

# 3. Paste to your LLM — done!
```

---

## Output Formats

### Colored (default)

Beautiful terminal output with error-type colors and icons:

```bash
error-toon
```

### Plain Text

For piping or scripts:

```bash
error-toon --plain
```

```
type: DOM_NESTING
file: Guide.mdx:79
issue: <p> cannot appear as a descendant of <p>
frames:
  MDXContent @ Guide.mdx:79

---
compressed: 4521c → 198c (95% saved)
```

### TOON Format

[TOON (Token-Oriented Object Notation)](https://github.com/toon-format/toon) — optimized for LLM parsing:

```bash
error-toon --toon
```

```
type: DOM_NESTING
file: Guide.mdx:79
issue: <p> cannot appear as a descendant of <p>
frames[1]{fn,loc}:
  MDXContent,Guide.mdx:79
stats{orig,comp,pct}: 4521,198,95
```

TOON uses tabular arrays (`frames[N]{fields}:`) and inline objects (`stats{fields}:`) for maximum token efficiency.

---

## Supported Error Types

error-toon automatically detects and categorizes **26 error types**:

| Category | Types | Example |
|----------|-------|---------|
| **React/DOM** | `DOM_NESTING`, `HYDRATION`, `INVALID_HOOK`, `REACT_MINIFIED` | `<p>` inside `<p>`, server/client mismatch |
| **JavaScript** | `TYPE_ERROR`, `REF_ERROR`, `SYNTAX_ERROR`, `RANGE_ERROR` | `undefined is not a function` |
| **Network** | `CORS_ERROR`, `HTTP_ERROR`, `NETWORK_ERROR`, `WEBSOCKET_ERROR` | CORS blocked, 404/500 responses |
| **Security** | `CSP_ERROR`, `SECURITY_ERROR`, `MIXED_CONTENT` | Content Security Policy violations |
| **Build Tools** | `STORYBOOK`, `NEXTJS`, `MODULE_NOT_FOUND` | `SB_*` codes, build failures |
| **Testing** | `PLAYWRIGHT` | Timeout, locator errors, assertions |
| **System** | `SYSTEM_ERROR`, `SERVICE_WORKER`, `INDEXEDDB_ERROR` | `ENOENT`, `ECONNREFUSED` |

Each type has optimized extraction rules to capture the most relevant information.

---

## How It Works

```
┌─────────────────────────────────────────────────────────────────┐
│                     Browser Console Error                        │
│  4000+ chars of webpack paths, React internals, cache hashes    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                         error-toon                                  │
│  1. Detect error type (25 patterns)                              │
│  2. Extract file location (prefers user code)                    │
│  3. Extract error message                                        │
│  4. Filter stack frames (removes framework noise)                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Compressed Output                            │
│  ~200 chars: type, file, issue, relevant frames                  │
└─────────────────────────────────────────────────────────────────┘
```

**Key optimizations:**
- **Smart file detection** — Finds your code, not `node_modules`
- **Framework noise filter** — Removes React, Webpack, Vite internals
- **Context-aware extraction** — Different logic per error type

---

## CLI Reference

```
error-toon [OPTIONS]

Options:
      --no-copy  Don't copy result to clipboard (copies by default)
  -p, --plain    Plain text output (no colors)
  -t, --toon     TOON format output (token-optimized)
  -h, --help     Print help
  -V, --version  Print version
```

### Input Methods

```bash
# Clipboard (default)
error-toon                      # Reads from clipboard, copies result back

# Pipe
pbpaste | error-toon            # macOS
xclip -o | error-toon           # Linux
cat error.log | error-toon      # File

# Interactive
error-toon                      # If clipboard empty, prompts for paste
```

### Common Workflows

```bash
# Quick compress (auto-copies result to clipboard)
error-toon

# Compress to TOON format for Claude/GPT
error-toon -t

# Use in scripts (no auto-copy when piped)
ERROR=$(pbpaste | error-toon -p)
```

---

## Why Rust?

| | |
|---|---|
| **Single binary** | No Node.js, Python, or runtime dependencies |
| **Fast startup** | ~1ms cold start (no JIT warmup) |
| **Small size** | 1.3MB stripped binary |
| **Cross-platform** | macOS, Linux, Windows |
| **Reliable** | Strong typing catches bugs at compile time |

---

## Contributing

PRs welcome! Ideas:

- [ ] More error patterns (Vue, Angular, Svelte, Cypress, Jest)
- [ ] Homebrew formula
- [ ] GitHub Actions releases
- [ ] VS Code extension

### Development

```bash
# Run tests
cargo test

# Run with sample input
echo "TypeError: foo is not a function" | cargo run

# Build release
cargo build --release
```

---

## License

MIT

---

<p align="center">
  <sub>Built for developers who talk to LLMs</sub>
</p>
