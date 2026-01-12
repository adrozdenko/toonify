# toonify

**Compress verbose browser errors before pasting to LLMs. Save 70-90% tokens.**

A fast, single-binary CLI tool written in Rust with beautiful terminal output.

```
╭─────────────────────────────────────────╮
│ 󰅖 DOM_NESTING                           │
├─────────────────────────────────────────┤
│  iOS-SafeArea-Guide.mdx:79             │
│  <p> cannot be descendant of <p>       │
│ frames:                                 │
│   _createMdxContent @ Guide.mdx:79      │
│   MDXContent @ Guide.mdx:1118           │
├─────────────────────────────────────────┤
│ 📦 4521c → 198c (95% saved)             │
╰─────────────────────────────────────────╯
```

## Install

### Cargo (recommended)

```bash
cargo install --git https://github.com/adrozdenko/toonify
```

### From releases

Download the binary for your platform from [Releases](https://github.com/adrozdenko/toonify/releases).

### Build locally

```bash
git clone https://github.com/adrozdenko/toonify
cd toonify
cargo build --release
./target/release/toonify --help
```

## Usage

```bash
# Copy error to clipboard, then:
toonify           # read clipboard, show colored output
toonify -c        # also copy result back to clipboard
toonify -p        # plain output (no colors)

# Or pipe:
pbpaste | toonify       # macOS
xclip -o | toonify      # Linux
cat error.txt | toonify
```

### Workflow

1. Copy error from browser console
2. Run `toonify -c`
3. Paste compressed result to your LLM

## Supported Errors

| Type | Color | Examples |
|------|-------|----------|
| **DOM_NESTING** | Yellow | `<p>` inside `<p>` |
| **HYDRATION** | Magenta | Server/client mismatch |
| **TYPE_ERROR** | Red | `undefined is not a function` |
| **REF_ERROR** | Red | `x is not defined` |
| **SYNTAX_ERROR** | Red | Unexpected token |
| **SYSTEM_ERROR** | Red | ENOENT, ECONNREFUSED |
| **STORYBOOK** | Cyan | SB_* error codes |
| **RUNTIME_ERROR** | Red | Generic stack traces |

## Why toonify?

LLMs charge per token. A typical browser error is 4000+ characters of noise:

```
Without toonify:  4000 chars → LLM reads 4000 chars → $$$
With toonify:      400 chars → LLM reads  400 chars → $
```

The LLM has already consumed tokens by the time it reads your error. Compress **before** you paste.

## Features

- **Smart detection** - Identifies error type automatically
- **Noise removal** - Strips React/Webpack/Vite internals
- **Colored output** - Error-type-specific colors and icons
- **Auto plain mode** - Detects pipes, disables colors
- **Cross-platform clipboard** - macOS, Linux, Windows

## Options

```
-c, --copy     Copy compressed result to clipboard
-p, --plain    Force plain output (no colors)
-h, --help     Show help
-V, --version  Show version
```

## Why Rust?

| | Benefit |
|---|---------|
| **Single binary** | No runtime dependencies |
| **Fast startup** | ~1ms cold start |
| **Small size** | ~1.3MB stripped |
| **Cross-platform** | macOS, Linux, Windows |
| **Reliable** | No "undefined is not a function" |

## Contributing

PRs welcome! Ideas:

- [ ] More error patterns (Vue, Angular, Svelte)
- [ ] Homebrew formula
- [ ] GitHub Actions releases
- [ ] VS Code extension

## License

MIT
