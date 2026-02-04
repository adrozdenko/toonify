# Changelog

All notable changes to error-toon will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] - 2026-02-05

### Added
- **Multi-error separation**: When input contains multiple errors, they are now automatically split into separate blocks. Each error retains its own stack frames — no more mixing frames from different errors.
- New helper functions: `split_into_error_blocks()`, `is_error_boundary()`, `is_stack_frame_line()`
- Multi-error output formatters for plain, TOON, and colored formats
- 20 new tests for multi-error functionality
- Extended `REACT_KEY` pattern to detect "unique key prop" warning format

### Changed
- `main()` now processes input through multi-error splitting pipeline
- Single-error output remains unchanged for backward compatibility
- Clipboard now shows error count when multiple errors are processed

### Fixed
- Stack frames from different errors no longer get mixed together
- Subsequent errors in console output are no longer lost

## [1.1.5] - 2026-01-15

### Added
- Playwright error detection (`PLAYWRIGHT` type)
- Support for locator, page, and assertion timeout errors
- Detection for `@playwright/test` import errors

## [1.1.4] - 2026-01-10

### Added
- TOON format output (`--toon` flag)
- Tabular array syntax for frames: `frames[N]{fn,loc}:`
- Inline object syntax for stats: `stats{orig,comp,pct}:`

### Changed
- Improved frame parsing with pre-compiled regex patterns

## [1.1.3] - 2026-01-05

### Added
- React key error detection (`REACT_KEY` type)
- Browser console prefix support (e.g., `file.js:42 TypeError:`)

### Fixed
- UTF-8 truncation now handles multi-byte characters safely

## [1.1.0] - 2025-12-20

### Added
- 27 error type detection patterns
- Colored terminal output with icons
- Plain text output (`--plain` flag)
- Automatic clipboard integration
- Smart file location extraction (prefers user code over node_modules)
- Framework noise filtering (React, Webpack, Vite internals)

### Changed
- Initial public release

[1.2.0]: https://github.com/adrozdenko/error-toon/compare/v1.1.5...v1.2.0
[1.1.5]: https://github.com/adrozdenko/error-toon/compare/v1.1.4...v1.1.5
[1.1.4]: https://github.com/adrozdenko/error-toon/compare/v1.1.3...v1.1.4
[1.1.3]: https://github.com/adrozdenko/error-toon/compare/v1.1.0...v1.1.3
[1.1.0]: https://github.com/adrozdenko/error-toon/releases/tag/v1.1.0
