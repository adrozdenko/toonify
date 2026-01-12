use arboard::Clipboard;
use clap::Parser;
use colored::*;
use once_cell::sync::Lazy;
use regex::Regex;
use std::io::{self, IsTerminal, Read};

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const BOX_WIDTH: usize = 43;
const CONTENT_WIDTH: usize = BOX_WIDTH - 4; // Account for "│ " and " │"
const TRUNCATE_WIDTH: usize = CONTENT_WIDTH - 3; // Account for "..."
const SOURCE_EXTENSIONS: &str = r"mdx|tsx|jsx|ts|js|vue|svelte";

// ─────────────────────────────────────────────────────────────────────────────
// CLI
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "toonify", version, about = "Compress verbose browser errors for LLM consumption")]
struct Args {
    /// Copy result to clipboard
    #[arg(short, long)]
    copy: bool,

    /// Plain output (no colors)
    #[arg(short, long)]
    plain: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Error Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum ErrorType {
    // DOM/React errors
    DomNesting,
    Hydration,
    ReactMinified,
    InvalidHook,
    // JavaScript errors
    TypeError,
    RefError,
    SyntaxError,
    RangeError,
    UriError,
    EvalError,
    // Network errors
    CorsError,
    NetworkError,
    HttpError,
    WebSocketError,
    // Security errors
    CspError,
    SecurityError,
    MixedContent,
    // Build tool errors
    Storybook,
    NextJs,
    ModuleNotFound,
    // System/Node errors
    SystemError,
    // Promise errors
    UnhandledRejection,
    // Browser API errors
    MediaError,
    IndexedDbError,
    ServiceWorker,
    // Deprecation/warnings
    Deprecation,
    // Catch-all
    RuntimeError,
}

impl ErrorType {
    // Order matters! More specific patterns first, RuntimeError (catch-all) last
    const ALL: &'static [ErrorType] = &[
        // DOM/React (most specific first)
        Self::DomNesting,
        Self::Hydration,
        Self::ReactMinified,
        Self::InvalidHook,
        // Security errors (MixedContent before SecurityError - more specific)
        Self::CorsError,
        Self::CspError,
        Self::MixedContent,      // Must come before SecurityError (contains "insecure")
        Self::SecurityError,
        // Browser APIs (ServiceWorker before HttpError - may contain status codes)
        Self::ServiceWorker,
        Self::MediaError,
        Self::IndexedDbError,
        // Network errors
        Self::NetworkError,
        Self::WebSocketError,
        Self::HttpError,         // After ServiceWorker (SW errors may contain HTTP codes)
        // Build tools
        Self::Storybook,
        Self::NextJs,
        Self::ModuleNotFound,
        // Promise errors (before JS errors - may contain TypeError text)
        Self::UnhandledRejection,
        // JavaScript errors
        Self::TypeError,
        Self::RefError,
        Self::SyntaxError,
        Self::RangeError,
        Self::UriError,
        Self::EvalError,
        // System errors
        Self::SystemError,
        // Warnings
        Self::Deprecation,
        // Catch-all (must be last)
        Self::RuntimeError,
    ];

    fn name(&self) -> &'static str {
        match self {
            Self::DomNesting => "DOM_NESTING",
            Self::Hydration => "HYDRATION",
            Self::ReactMinified => "REACT_MINIFIED",
            Self::InvalidHook => "INVALID_HOOK",
            Self::TypeError => "TYPE_ERROR",
            Self::RefError => "REF_ERROR",
            Self::SyntaxError => "SYNTAX_ERROR",
            Self::RangeError => "RANGE_ERROR",
            Self::UriError => "URI_ERROR",
            Self::EvalError => "EVAL_ERROR",
            Self::CorsError => "CORS_ERROR",
            Self::NetworkError => "NETWORK_ERROR",
            Self::HttpError => "HTTP_ERROR",
            Self::WebSocketError => "WEBSOCKET_ERROR",
            Self::CspError => "CSP_ERROR",
            Self::SecurityError => "SECURITY_ERROR",
            Self::MixedContent => "MIXED_CONTENT",
            Self::Storybook => "STORYBOOK",
            Self::NextJs => "NEXTJS",
            Self::ModuleNotFound => "MODULE_NOT_FOUND",
            Self::SystemError => "SYSTEM_ERROR",
            Self::UnhandledRejection => "UNHANDLED_REJECTION",
            Self::MediaError => "MEDIA_ERROR",
            Self::IndexedDbError => "INDEXEDDB_ERROR",
            Self::ServiceWorker => "SERVICE_WORKER",
            Self::Deprecation => "DEPRECATION",
            Self::RuntimeError => "RUNTIME_ERROR",
        }
    }

    fn color(&self) -> Color {
        match self {
            // Warnings (yellow)
            Self::DomNesting | Self::Deprecation => Color::Yellow,
            // React/Hydration (magenta)
            Self::Hydration | Self::ReactMinified | Self::InvalidHook => Color::Magenta,
            // Build tools (cyan)
            Self::Storybook | Self::NextJs | Self::ModuleNotFound => Color::Cyan,
            // Network (blue)
            Self::NetworkError | Self::HttpError | Self::WebSocketError => Color::Blue,
            // Security (bright red)
            Self::CorsError | Self::CspError | Self::SecurityError | Self::MixedContent => Color::BrightRed,
            // All other errors (red)
            _ => Color::Red,
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            Self::DomNesting => "󰅖",
            Self::Hydration | Self::ReactMinified | Self::InvalidHook => "󰜈",
            Self::Storybook => "󰂺",
            Self::NextJs => "󰔶",
            Self::CorsError | Self::CspError | Self::SecurityError | Self::MixedContent => "󰒃",
            Self::NetworkError | Self::HttpError => "󰖟",
            Self::WebSocketError => "󱄙",
            Self::ModuleNotFound => "󰏗",
            Self::SystemError => "",
            Self::UnhandledRejection => "󰜺",
            Self::MediaError => "󰎁",
            Self::IndexedDbError => "󰆼",
            Self::ServiceWorker => "󰖟",
            Self::Deprecation => "󰀦",
            _ => "",
        }
    }

    fn pattern(&self) -> &Regex {
        match self {
            Self::DomNesting => &PATTERNS.dom_nesting,
            Self::Hydration => &PATTERNS.hydration,
            Self::ReactMinified => &PATTERNS.react_minified,
            Self::InvalidHook => &PATTERNS.invalid_hook,
            Self::TypeError => &PATTERNS.type_error,
            Self::RefError => &PATTERNS.ref_error,
            Self::SyntaxError => &PATTERNS.syntax_error,
            Self::RangeError => &PATTERNS.range_error,
            Self::UriError => &PATTERNS.uri_error,
            Self::EvalError => &PATTERNS.eval_error,
            Self::CorsError => &PATTERNS.cors_error,
            Self::NetworkError => &PATTERNS.network_error,
            Self::HttpError => &PATTERNS.http_error,
            Self::WebSocketError => &PATTERNS.websocket_error,
            Self::CspError => &PATTERNS.csp_error,
            Self::SecurityError => &PATTERNS.security_error,
            Self::MixedContent => &PATTERNS.mixed_content,
            Self::Storybook => &PATTERNS.storybook,
            Self::NextJs => &PATTERNS.nextjs,
            Self::ModuleNotFound => &PATTERNS.module_not_found,
            Self::SystemError => &PATTERNS.system_error,
            Self::UnhandledRejection => &PATTERNS.unhandled_rejection,
            Self::MediaError => &PATTERNS.media_error,
            Self::IndexedDbError => &PATTERNS.indexeddb_error,
            Self::ServiceWorker => &PATTERNS.service_worker,
            Self::Deprecation => &PATTERNS.deprecation,
            Self::RuntimeError => &PATTERNS.stack_trace,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Patterns (compiled once at startup)
// ─────────────────────────────────────────────────────────────────────────────

static PATTERNS: Lazy<Patterns> = Lazy::new(Patterns::compile);

struct Patterns {
    // Detection - DOM/React
    dom_nesting: Regex,
    hydration: Regex,
    react_minified: Regex,
    invalid_hook: Regex,
    // Detection - JavaScript errors
    type_error: Regex,
    ref_error: Regex,
    syntax_error: Regex,
    range_error: Regex,
    uri_error: Regex,
    eval_error: Regex,
    // Detection - Network
    cors_error: Regex,
    network_error: Regex,
    http_error: Regex,
    websocket_error: Regex,
    // Detection - Security
    csp_error: Regex,
    security_error: Regex,
    mixed_content: Regex,
    // Detection - Build tools
    storybook: Regex,
    nextjs: Regex,
    module_not_found: Regex,
    // Detection - System
    system_error: Regex,
    // Detection - Promise
    unhandled_rejection: Regex,
    // Detection - Browser APIs
    media_error: Regex,
    indexeddb_error: Regex,
    service_worker: Regex,
    // Detection - Warnings
    deprecation: Regex,
    // Detection - Catch-all
    stack_trace: Regex,
    // Extraction patterns
    file_location: Regex,
    dom_issue: Regex,
    system_code: Regex,
    storybook_code: Regex,
    nextjs_code: Regex,
    http_status: Regex,
    user_frame: Regex,
    framework_noise: Regex,
}

impl Patterns {
    fn compile() -> Self {
        let ext = SOURCE_EXTENSIONS;
        Self {
            // Detection - DOM/React
            dom_nesting: re(r"(?i)validateDOMNesting"),
            hydration: re(r"(?i)hydrat(ion|e|ing).*(?:failed|mismatch|error)"),
            react_minified: re(r"Minified React error #\d+|react\.production\.min\.js"),
            invalid_hook: re(r"(?i)Invalid hook call|Rules of Hooks|rendered more hooks"),

            // Detection - JavaScript errors
            type_error: re(r"(?m)^TypeError:|Uncaught TypeError"),
            ref_error: re(r"(?m)^ReferenceError:|Uncaught ReferenceError"),
            syntax_error: re(r"(?m)^SyntaxError:|Uncaught SyntaxError"),
            range_error: re(r"(?m)^RangeError:|Uncaught RangeError"),
            uri_error: re(r"(?m)^URIError:|Uncaught URIError"),
            eval_error: re(r"(?m)^EvalError:|Uncaught EvalError"),

            // Detection - Network
            cors_error: re(r"(?i)CORS|Access-Control-Allow-Origin|blocked by CORS|cross-origin"),
            network_error: re(r"(?i)Failed to fetch|NetworkError|net::ERR_|NS_ERROR_|fetch.*failed"),
            // HTTP errors: "GET /api 404" or "status: 500" but NOT "bundle.js:45892"
            http_error: re(r"(?i)\b(GET|POST|PUT|DELETE|PATCH)\s+\S+\s+[45]\d{2}\b|status[:\s]+[45]\d{2}\b|\b[45]\d{2}\s+(Not Found|Internal Server|Bad Request|Unauthorized|Forbidden)"),
            websocket_error: re(r"(?i)WebSocket.*(?:error|failed|closed)|ws://.*error|wss://.*error"),

            // Detection - Security
            csp_error: re(r"(?i)Content-Security-Policy|CSP|blocked.*policy|violat.*directive"),
            security_error: re(r"(?i)SecurityError|security.*violation|insecure|blocked.*security"),
            mixed_content: re(r"(?i)Mixed Content|blocked.*insecure|http://.*https://"),

            // Detection - Build tools
            storybook: re(r"SB_"),
            nextjs: re(r"(?i)NEXT_|getServerSideProps|getStaticProps|NextJS|next/"),
            module_not_found: re(r"(?i)Module not found|Cannot find module|Cannot resolve|ModuleNotFoundError"),

            // Detection - System
            system_error: re(r"ENOENT|EACCES|ECONNREFUSED|ETIMEDOUT|EADDRINUSE|EPERM"),

            // Detection - Promise
            unhandled_rejection: re(r"(?i)Unhandled.*rejection|UnhandledPromiseRejection|promise.*reject"),

            // Detection - Browser APIs
            media_error: re(r"(?i)MediaError|NotSupportedError.*media|play\(\).*failed|autoplay.*blocked"),
            indexeddb_error: re(r"(?i)IndexedDB|IDBDatabase|QuotaExceededError|VersionError"),
            service_worker: re(r"(?i)ServiceWorker|service.*worker.*(?:error|failed)|SW.*(?:error|failed)"),

            // Detection - Warnings
            deprecation: re(r"(?i)deprecated|deprecation|will be removed|no longer supported"),

            // Detection - Catch-all (must be very generic)
            stack_trace: re(r"at .* \(.*:\d+:\d+\)|Error:.*\n.*at\s"),

            // Extraction patterns
            file_location: re(&format!(r"[A-Za-z0-9_.-]+\.({ext}):\d+")),
            dom_issue: re(r"<[a-z]+> cannot (?:appear as a |be a )?descendant of <[a-z]+>"),
            system_code: re(r"E[A-Z]+:[^\n]*"),
            storybook_code: re(r"SB_[A-Z_]+[^\n]*"),
            nextjs_code: re(r"NEXT_[A-Z_]+|(?:getServerSideProps|getStaticProps)[^\n]*error"),
            http_status: re(r"\b[45]\d{2}\b"),
            user_frame: re(&format!(r"(@|at ).+\.({ext}):\d+")),
            framework_noise: re(r"chunk-|node_modules|storybook_internal|webpack|vite|/internal|react-dom"),
        }
    }
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("Invalid regex pattern")
}

// ─────────────────────────────────────────────────────────────────────────────
// Detection
// ─────────────────────────────────────────────────────────────────────────────

fn detect_error_type(input: &str) -> Option<ErrorType> {
    ErrorType::ALL
        .iter()
        .find(|t| t.pattern().is_match(input))
        .copied()
}

// ─────────────────────────────────────────────────────────────────────────────
// Extraction
// ─────────────────────────────────────────────────────────────────────────────

fn extract_file_location(input: &str) -> Option<String> {
    // Prefer user code over node_modules/framework files
    let all_matches: Vec<_> = PATTERNS.file_location.find_iter(input).collect();

    // First try to find a user file (not in node_modules)
    for m in &all_matches {
        // Get the full line containing this match
        let line_start = input[..m.start()].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line_end = input[m.end()..].find('\n').map(|i| m.end() + i).unwrap_or(input.len());
        let full_line = &input[line_start..line_end];

        // Skip node_modules - user code is in src/, pages/, components/, etc.
        if !full_line.contains("node_modules") {
            return Some(m.as_str().to_string());
        }
    }

    // Fallback to first match
    all_matches.first().map(|m| m.as_str().to_string())
}

fn extract_issue(input: &str, error_type: ErrorType) -> Option<String> {
    match error_type {
        // DOM/React errors
        ErrorType::DomNesting => extract_by_pattern_or_contains(input, &PATTERNS.dom_issue, "descendant"),
        ErrorType::Hydration => find_line_containing(input, &["hydration", "mismatch", "server", "client"]),
        ErrorType::ReactMinified => find_line_containing(input, &["Minified React error", "react.production"]),
        ErrorType::InvalidHook => find_line_containing(input, &["Invalid hook", "Rules of Hooks", "rendered more hooks"]),

        // JavaScript errors
        ErrorType::TypeError => find_line_starting_with(input, &["TypeError:", "Uncaught TypeError"]),
        ErrorType::RefError => find_line_starting_with(input, &["ReferenceError:", "Uncaught ReferenceError"]),
        ErrorType::SyntaxError => find_line_starting_with(input, &["SyntaxError:", "Uncaught SyntaxError"]),
        ErrorType::RangeError => find_line_starting_with(input, &["RangeError:", "Uncaught RangeError"]),
        ErrorType::UriError => find_line_starting_with(input, &["URIError:", "Uncaught URIError"]),
        ErrorType::EvalError => find_line_starting_with(input, &["EvalError:", "Uncaught EvalError"]),

        // Network errors
        ErrorType::CorsError => find_line_containing(input, &["CORS", "Access-Control", "cross-origin", "blocked"]),
        ErrorType::NetworkError => find_line_containing(input, &["Failed to fetch", "NetworkError", "net::ERR_", "fetch"]),
        ErrorType::HttpError => extract_first_match(input, &PATTERNS.http_status)
            .and_then(|status| find_line_containing(input, &[&status])),
        ErrorType::WebSocketError => find_line_containing(input, &["WebSocket", "ws://", "wss://"]),

        // Security errors
        ErrorType::CspError => find_line_containing(input, &["Content-Security-Policy", "CSP", "directive", "violated"]),
        ErrorType::SecurityError => find_line_containing(input, &["SecurityError", "security", "blocked"]),
        ErrorType::MixedContent => find_line_containing(input, &["Mixed Content", "insecure", "http://"]),

        // Build tools
        ErrorType::Storybook => extract_first_match_truncated(input, &PATTERNS.storybook_code, 100),
        ErrorType::NextJs => extract_first_match_truncated(input, &PATTERNS.nextjs_code, 100)
            .or_else(|| find_line_containing(input, &["NEXT_", "getServerSideProps", "getStaticProps"])),
        ErrorType::ModuleNotFound => find_line_containing(input, &["Module not found", "Cannot find module", "Cannot resolve"]),

        // System errors
        ErrorType::SystemError => extract_first_match(input, &PATTERNS.system_code),

        // Promise errors
        ErrorType::UnhandledRejection => find_line_containing(input, &["Unhandled", "rejection", "promise"]),

        // Browser API errors
        ErrorType::MediaError => find_line_containing(input, &["MediaError", "play()", "autoplay", "media"]),
        ErrorType::IndexedDbError => find_line_containing(input, &["IndexedDB", "IDBDatabase", "QuotaExceeded"]),
        ErrorType::ServiceWorker => find_line_containing(input, &["ServiceWorker", "service worker", "SW"]),

        // Warnings
        ErrorType::Deprecation => find_line_containing(input, &["deprecated", "deprecation", "will be removed"]),

        // Catch-all
        ErrorType::RuntimeError => input.lines().next().map(str::to_string),
    }
}

fn extract_user_frames(input: &str) -> Vec<String> {
    input
        .lines()
        .filter(|line| PATTERNS.user_frame.is_match(line) && !PATTERNS.framework_noise.is_match(line))
        .take(3)
        .map(|s| s.trim().to_string())
        .collect()
}

// Extraction helpers
fn extract_by_pattern_or_contains(input: &str, pattern: &Regex, fallback_contains: &str) -> Option<String> {
    pattern
        .find(input)
        .map(|m| m.as_str().to_string())
        .or_else(|| find_line_containing(input, &[fallback_contains]))
}

fn extract_first_match(input: &str, pattern: &Regex) -> Option<String> {
    pattern.find(input).map(|m| m.as_str().to_string())
}

fn extract_first_match_truncated(input: &str, pattern: &Regex, max_len: usize) -> Option<String> {
    pattern.find(input).map(|m| truncate(m.as_str(), max_len))
}

fn find_line_containing(input: &str, needles: &[&str]) -> Option<String> {
    input
        .lines()
        .find(|line| {
            let lower = line.to_lowercase();
            needles.iter().any(|n| lower.contains(&n.to_lowercase()))
        })
        .map(|s| s.trim().to_string())
}

fn find_line_starting_with(input: &str, prefixes: &[&str]) -> Option<String> {
    input
        .lines()
        .find(|line| prefixes.iter().any(|p| line.starts_with(p)))
        .map(str::to_string)
}

// ─────────────────────────────────────────────────────────────────────────────
// Output Model
// ─────────────────────────────────────────────────────────────────────────────

struct ToonifiedError {
    error_type: ErrorType,
    file_location: Option<String>,
    issue: Option<String>,
    frames: Vec<String>,
    original_len: usize,
}

impl ToonifiedError {
    fn new(input: &str, error_type: ErrorType) -> Self {
        Self {
            error_type,
            file_location: extract_file_location(input),
            issue: extract_issue(input, error_type),
            frames: extract_user_frames(input),
            original_len: input.len(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Plain Formatter
// ─────────────────────────────────────────────────────────────────────────────

impl ToonifiedError {
    fn format_plain(&self) -> String {
        let mut lines = vec![format!("type: {}", self.error_type.name())];

        if let Some(ref loc) = self.file_location {
            lines.push(format!("file: {}", loc));
        }

        if let Some(ref issue) = self.issue {
            lines.push(format!("issue: {}", issue));
        }

        if !self.frames.is_empty() {
            lines.push("frames:".to_string());
            for frame in &self.frames {
                lines.push(format!("  {}", frame));
            }
        }

        // Calculate stats without recursion
        let content = lines.join("\n");
        let stats_overhead = 40; // Approximate size of stats line
        let compressed_len = content.len() + stats_overhead;
        let savings = if self.original_len > compressed_len {
            ((self.original_len - compressed_len) * 100) / self.original_len
        } else {
            0
        };

        lines.push(String::new());
        lines.push("---".to_string());
        lines.push(format!("compressed: {}c → {}c ({}% saved)", self.original_len, compressed_len, savings));

        lines.join("\n")
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Colored Formatter
// ─────────────────────────────────────────────────────────────────────────────

impl ToonifiedError {
    fn format_colored(&self) -> String {
        let color = self.error_type.color();
        let mut box_lines = BoxBuilder::new(color);

        box_lines.header(&format!("{} {}", self.error_type.icon(), self.error_type.name()));

        if let Some(ref loc) = self.file_location {
            box_lines.row(&format!(" {}", loc), Color::White);
        }

        if let Some(ref issue) = self.issue {
            box_lines.row(&format!(" {}", truncate(issue, TRUNCATE_WIDTH)), Color::Yellow);
        }

        if !self.frames.is_empty() {
            box_lines.row("frames:", Color::BrightBlack);
            for frame in &self.frames {
                box_lines.row(&format!("  {}", truncate(frame, TRUNCATE_WIDTH - 2)), Color::Cyan);
            }
        }

        // Calculate stats (use plain format length)
        let plain_len = self.format_plain().len();
        let savings = if self.original_len > plain_len {
            ((self.original_len - plain_len) * 100) / self.original_len
        } else {
            0
        };

        box_lines.separator();
        box_lines.row(&format!("📦 {}c → {}c ({}% saved)", self.original_len, plain_len, savings), Color::Green);

        box_lines.build()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Box Drawing Helper
// ─────────────────────────────────────────────────────────────────────────────

struct BoxBuilder {
    lines: Vec<String>,
    color: Color,
}

impl BoxBuilder {
    fn new(color: Color) -> Self {
        Self { lines: vec![], color }
    }

    fn header(&mut self, text: &str) {
        self.lines.push(self.horizontal_line('╭', '╮'));
        self.lines.push(self.content_line(text, self.color, true));
        self.lines.push(self.horizontal_line('├', '┤'));
    }

    fn row(&mut self, text: &str, content_color: Color) {
        self.lines.push(self.content_line(text, content_color, false));
    }

    fn separator(&mut self) {
        self.lines.push(self.horizontal_line('├', '┤'));
    }

    fn build(mut self) -> String {
        self.lines.push(self.horizontal_line('╰', '╯'));
        self.lines.join("\n")
    }

    fn horizontal_line(&self, left: char, right: char) -> String {
        format!(
            "{}{}{}",
            left.to_string().color(self.color),
            "─".repeat(BOX_WIDTH - 2).color(self.color),
            right.to_string().color(self.color)
        )
    }

    fn content_line(&self, text: &str, content_color: Color, bold: bool) -> String {
        let content = if bold {
            text.color(content_color).bold().to_string()
        } else {
            text.color(content_color).to_string()
        };

        let visible_len = text.chars().count();
        let padding = CONTENT_WIDTH.saturating_sub(visible_len);

        format!(
            "{} {}{} {}",
            "│".color(self.color),
            content,
            " ".repeat(padding),
            "│".color(self.color)
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Utilities
// ─────────────────────────────────────────────────────────────────────────────

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Input
// ─────────────────────────────────────────────────────────────────────────────

fn read_input() -> Result<String, &'static str> {
    // If piped, read from stdin
    if !io::stdin().is_terminal() {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|_| "Failed to read stdin")?;
        return Ok(buf);
    }

    // Try clipboard first
    if let Ok(mut clipboard) = Clipboard::new() {
        if let Ok(text) = clipboard.get_text() {
            if !text.trim().is_empty() {
                return Ok(text);
            }
        }
    }

    // Clipboard empty - wait for user to paste
    eprintln!("{}", "Clipboard empty. Paste error below, then press Ctrl+D:".yellow());
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .map_err(|_| "Failed to read stdin")?;
    Ok(buf)
}

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

fn main() {
    let args = Args::parse();

    if args.plain || !io::stdout().is_terminal() {
        colored::control::set_override(false);
    }

    let input = match read_input() {
        Ok(s) if s.trim().is_empty() => exit_with_error("No input. Copy an error to clipboard or pipe it in."),
        Ok(s) => s,
        Err(e) => exit_with_error(e),
    };

    let error_type = match detect_error_type(&input) {
        Some(t) => t,
        None => {
            eprintln!("{}", "Not a recognizable error. Passing through.".yellow());
            println!("{}", input);
            return;
        }
    };

    let result = ToonifiedError::new(&input, error_type);
    let plain_output = result.format_plain();

    // Display
    if args.plain || !io::stdout().is_terminal() {
        println!("{}", plain_output);
    } else {
        println!("{}", result.format_colored());
    }

    // Copy to clipboard
    if args.copy {
        if let Ok(mut clipboard) = Clipboard::new() {
            if clipboard.set_text(&plain_output).is_ok() {
                eprintln!("{}", "✓ Copied to clipboard!".green().bold());
            }
        }
    }
}

fn exit_with_error(msg: &str) -> ! {
    eprintln!("{} {}", "Error:".red().bold(), msg);
    std::process::exit(1)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─────────────────────────────────────────────────────────────────────────
    // Error Type Detection Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn detects_dom_nesting_error() {
        let input = "Warning: validateDOMNesting(...): <p> cannot appear as a descendant of <p>.";
        let result = detect_error_type(input);
        assert!(matches!(result, Some(ErrorType::DomNesting)));
    }

    #[test]
    fn detects_dom_nesting_case_insensitive() {
        let input = "Warning: VALIDATEDOMNESTING(...): error";
        let result = detect_error_type(input);
        assert!(matches!(result, Some(ErrorType::DomNesting)));
    }

    #[test]
    fn detects_hydration_error() {
        let input = "Uncaught Error: Hydration failed because the initial UI does not match.";
        let result = detect_error_type(input);
        assert!(matches!(result, Some(ErrorType::Hydration)));
    }

    #[test]
    fn detects_type_error() {
        let input = "TypeError: Cannot read properties of undefined (reading 'map')";
        let result = detect_error_type(input);
        assert!(matches!(result, Some(ErrorType::TypeError)));
    }

    #[test]
    fn detects_reference_error() {
        let input = "ReferenceError: myVariable is not defined";
        let result = detect_error_type(input);
        assert!(matches!(result, Some(ErrorType::RefError)));
    }

    #[test]
    fn detects_syntax_error() {
        let input = "SyntaxError: Unexpected token '<'";
        let result = detect_error_type(input);
        assert!(matches!(result, Some(ErrorType::SyntaxError)));
    }

    #[test]
    fn detects_system_error_enoent() {
        let input = "Error: ENOENT: no such file or directory, open '/path/to/file'";
        let result = detect_error_type(input);
        assert!(matches!(result, Some(ErrorType::SystemError)));
    }

    #[test]
    fn detects_system_error_econnrefused() {
        let input = "Error: connect ECONNREFUSED 127.0.0.1:3000";
        let result = detect_error_type(input);
        assert!(matches!(result, Some(ErrorType::SystemError)));
    }

    #[test]
    fn detects_storybook_error() {
        let input = "SB_PREVIEW_API_UNDEFINED: The preview API is not available.";
        let result = detect_error_type(input);
        assert!(matches!(result, Some(ErrorType::Storybook)));
    }

    #[test]
    fn detects_runtime_error_with_stack_trace() {
        let input = "Error: Something went wrong\n    at MyComponent (App.tsx:25:10)";
        let result = detect_error_type(input);
        // Should match RuntimeError due to stack trace pattern
        assert!(result.is_some());
    }

    #[test]
    fn returns_none_for_unrecognized_input() {
        let input = "This is just some random text without any error patterns.";
        let result = detect_error_type(input);
        assert!(result.is_none());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // New Error Type Detection Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn detects_react_minified_error() {
        let input = "Minified React error #185; visit https://reactjs.org/docs/error-decoder.html";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::ReactMinified));
    }

    #[test]
    fn detects_invalid_hook_error() {
        let input = "Invalid hook call. Hooks can only be called inside of the body of a function component.";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::InvalidHook));
    }

    #[test]
    fn detects_cors_error() {
        let input = "Access to XMLHttpRequest at 'https://api.example.com' from origin 'http://localhost:3000' has been blocked by CORS policy";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::CorsError));
    }

    #[test]
    fn detects_network_error() {
        let input = "TypeError: Failed to fetch";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::NetworkError));
    }

    #[test]
    fn detects_network_error_chrome() {
        let input = "net::ERR_CONNECTION_REFUSED";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::NetworkError));
    }

    #[test]
    fn detects_http_error() {
        let input = "GET https://api.example.com/users 404 (Not Found)";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::HttpError));
    }

    #[test]
    fn detects_websocket_error() {
        let input = "WebSocket connection to 'wss://example.com/socket' failed";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::WebSocketError));
    }

    #[test]
    fn detects_csp_error() {
        let input = "Refused to execute inline script because it violates the following Content-Security-Policy directive";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::CspError));
    }

    #[test]
    fn detects_mixed_content_error() {
        let input = "Mixed Content: The page at 'https://example.com' was loaded over HTTPS, but requested an insecure resource";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::MixedContent));
    }

    #[test]
    fn detects_nextjs_error() {
        let input = "Error: getServerSideProps should return an object";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::NextJs));
    }

    #[test]
    fn detects_module_not_found_error() {
        let input = "Module not found: Can't resolve './components/Button' in '/app/src'";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::ModuleNotFound));
    }

    #[test]
    fn detects_range_error() {
        let input = "RangeError: Maximum call stack size exceeded";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::RangeError));
    }

    #[test]
    fn detects_unhandled_rejection() {
        let input = "Unhandled Promise Rejection: TypeError: Cannot read property 'x' of undefined";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::UnhandledRejection));
    }

    #[test]
    fn detects_media_error() {
        let input = "DOMException: play() failed because the user didn't interact with the document first";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::MediaError));
    }

    #[test]
    fn detects_indexeddb_error() {
        let input = "QuotaExceededError: The IndexedDB quota has been exceeded";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::IndexedDbError));
    }

    #[test]
    fn detects_service_worker_error() {
        let input = "ServiceWorker registration failed: A bad HTTP response code (404) was received";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::ServiceWorker));
    }

    #[test]
    fn detects_deprecation_warning() {
        let input = "Warning: componentWillMount has been renamed, and is not recommended for use. This method will be deprecated in a future version.";
        let result = detect_error_type(input);
        assert_eq!(result, Some(ErrorType::Deprecation));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // File Location Extraction Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn extracts_simple_file_location() {
        let input = "Error at MyComponent.tsx:42";
        let result = extract_file_location(input);
        assert_eq!(result, Some("MyComponent.tsx:42".to_string()));
    }

    #[test]
    fn prefers_user_code_over_node_modules() {
        let input = r#"
    at CardContent (webpack-internal:///./node_modules/@mui/material/CardContent.js:82:35)
    at Dashboard (webpack-internal:///./src/pages/Dashboard.tsx:45:23)
    at App (webpack-internal:///./node_modules/react-router/index.js:100:5)
"#;
        let result = extract_file_location(input);
        assert_eq!(result, Some("Dashboard.tsx:45".to_string()));
    }

    #[test]
    fn falls_back_to_node_modules_if_no_user_code() {
        let input = r#"
    at CardContent (webpack-internal:///./node_modules/@mui/material/CardContent.js:82:35)
    at Container (webpack-internal:///./node_modules/@mui/material/Container.js:55:12)
"#;
        let result = extract_file_location(input);
        assert_eq!(result, Some("CardContent.js:82".to_string()));
    }

    #[test]
    fn extracts_mdx_file_location() {
        let input = "Error in iOS-SafeArea-Guide.mdx:79";
        let result = extract_file_location(input);
        assert_eq!(result, Some("iOS-SafeArea-Guide.mdx:79".to_string()));
    }

    #[test]
    fn extracts_vue_file_location() {
        let input = "Error at MyComponent.vue:123";
        let result = extract_file_location(input);
        assert_eq!(result, Some("MyComponent.vue:123".to_string()));
    }

    #[test]
    fn extracts_svelte_file_location() {
        let input = "Error at App.svelte:42";
        let result = extract_file_location(input);
        assert_eq!(result, Some("App.svelte:42".to_string()));
    }

    #[test]
    fn returns_none_when_no_file_location() {
        let input = "Some error without file reference";
        let result = extract_file_location(input);
        assert!(result.is_none());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Issue Extraction Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn extracts_dom_nesting_issue_appear_as() {
        let input = "Warning: validateDOMNesting(...): <p> cannot appear as a descendant of <p>.";
        let result = extract_issue(input, ErrorType::DomNesting);
        assert_eq!(result, Some("<p> cannot appear as a descendant of <p>".to_string()));
    }

    #[test]
    fn extracts_dom_nesting_issue_be_a() {
        let input = "In HTML, <div> cannot be a descendant of <p>.";
        let result = extract_issue(input, ErrorType::DomNesting);
        assert_eq!(result, Some("<div> cannot be a descendant of <p>".to_string()));
    }

    #[test]
    fn extracts_hydration_issue() {
        let input = "Uncaught Error: Hydration failed because the initial UI does not match.";
        let result = extract_issue(input, ErrorType::Hydration);
        assert_eq!(result, Some("Uncaught Error: Hydration failed because the initial UI does not match.".to_string()));
    }

    #[test]
    fn extracts_type_error_issue() {
        let input = "TypeError: Cannot read properties of undefined (reading 'map')\n    at Array.map";
        let result = extract_issue(input, ErrorType::TypeError);
        assert_eq!(result, Some("TypeError: Cannot read properties of undefined (reading 'map')".to_string()));
    }

    #[test]
    fn extracts_system_error_code() {
        let input = "Error: ENOENT: no such file or directory, open '/path/to/file'";
        let result = extract_issue(input, ErrorType::SystemError);
        assert_eq!(result, Some("ENOENT: no such file or directory, open '/path/to/file'".to_string()));
    }

    #[test]
    fn extracts_storybook_code() {
        let input = "SB_PREVIEW_API_UNDEFINED: The preview API is not available.";
        let result = extract_issue(input, ErrorType::Storybook);
        assert!(result.unwrap().starts_with("SB_PREVIEW_API_UNDEFINED"));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // User Frame Extraction Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn extracts_user_frames_filters_node_modules() {
        let input = r#"
    at CardContent (webpack-internal:///./node_modules/@mui/material/CardContent.js:82:35)
    at Dashboard (./src/pages/Dashboard.tsx:45:23)
    at App (./src/App.tsx:18:42)
    at Router (webpack-internal:///./node_modules/react-router/index.js:100:5)
"#;
        let result = extract_user_frames(input);
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("Dashboard.tsx"));
        assert!(result[1].contains("App.tsx"));
    }

    #[test]
    fn limits_user_frames_to_three() {
        let input = r#"
    at Component1 (./src/Component1.tsx:10:5)
    at Component2 (./src/Component2.tsx:20:5)
    at Component3 (./src/Component3.tsx:30:5)
    at Component4 (./src/Component4.tsx:40:5)
    at Component5 (./src/Component5.tsx:50:5)
"#;
        let result = extract_user_frames(input);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn returns_empty_when_no_user_frames() {
        let input = "Some error without stack trace";
        let result = extract_user_frames(input);
        assert!(result.is_empty());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Output Format Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn plain_format_includes_type() {
        let input = "TypeError: test error";
        let result = ToonifiedError::new(input, ErrorType::TypeError);
        let output = result.format_plain();
        assert!(output.contains("type: TYPE_ERROR"));
    }

    #[test]
    fn plain_format_includes_file_when_present() {
        let input = "Error at MyComponent.tsx:42";
        let error_type = detect_error_type(input).unwrap_or(ErrorType::RuntimeError);
        let result = ToonifiedError::new(input, error_type);
        let output = result.format_plain();
        assert!(output.contains("file: MyComponent.tsx:42"));
    }

    #[test]
    fn plain_format_includes_compression_stats() {
        let input = "TypeError: test error with some extra content to make it longer";
        let result = ToonifiedError::new(input, ErrorType::TypeError);
        let output = result.format_plain();
        assert!(output.contains("compressed:"));
        assert!(output.contains("saved)"));
    }

    #[test]
    fn plain_format_omits_file_when_none() {
        let input = "TypeError: test error";
        let result = ToonifiedError::new(input, ErrorType::TypeError);
        let output = result.format_plain();
        assert!(!output.contains("file:"));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Utility Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn truncate_short_string_unchanged() {
        let result = truncate("short", 10);
        assert_eq!(result, "short");
    }

    #[test]
    fn truncate_long_string_with_ellipsis() {
        let result = truncate("this is a very long string", 10);
        assert_eq!(result, "this is...");
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        let result = truncate("exactly10!", 10);
        assert_eq!(result, "exactly10!");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Integration Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn full_dom_nesting_error_processing() {
        let input = r#"Warning: validateDOMNesting(...): <p> cannot appear as a descendant of <p>.
    at p
    at CardContent (webpack-internal:///./node_modules/@mui/material/CardContent.js:82:35)
    at Dashboard (webpack-internal:///./src/pages/Dashboard.tsx:45:23)
    at App (webpack-internal:///./src/App.tsx:18:42)"#;

        let error_type = detect_error_type(input).unwrap();
        assert!(matches!(error_type, ErrorType::DomNesting));

        let result = ToonifiedError::new(input, error_type);
        assert_eq!(result.file_location, Some("Dashboard.tsx:45".to_string()));
        assert_eq!(result.issue, Some("<p> cannot appear as a descendant of <p>".to_string()));

        let output = result.format_plain();
        assert!(output.contains("type: DOM_NESTING"));
        assert!(output.contains("file: Dashboard.tsx:45"));
        assert!(output.contains("<p> cannot appear as a descendant of <p>"));
    }

    #[test]
    fn full_hydration_error_processing() {
        let input = r#"Uncaught Error: Hydration failed because the initial UI does not match what was rendered on the server.
    at throwOnHydrationMismatch (webpack-internal:///./node_modules/react-dom/index.js:12507:9)
    at BlogPost (webpack-internal:///./src/components/BlogPost.tsx:23:18)
    at Layout (webpack-internal:///./src/components/Layout.tsx:45:12)"#;

        let error_type = detect_error_type(input).unwrap();
        assert!(matches!(error_type, ErrorType::Hydration));

        let result = ToonifiedError::new(input, error_type);
        assert_eq!(result.file_location, Some("BlogPost.tsx:23".to_string()));
        assert!(result.issue.as_ref().unwrap().contains("Hydration failed"));
    }

    #[test]
    fn compression_ratio_calculated_correctly() {
        // Use a realistic verbose error (like from browser console)
        let long_input = format!(
            "TypeError: Cannot read properties of undefined\n{}",
            (0..50).map(|i| format!("    at function{} (webpack-internal:///./node_modules/react/index.js:{}:10)", i, i * 100))
                   .collect::<Vec<_>>()
                   .join("\n")
        );

        let result = ToonifiedError::new(&long_input, ErrorType::TypeError);
        let output = result.format_plain();

        // Output should be much shorter than input for verbose stack traces
        assert!(output.len() < long_input.len(), "Output ({}) should be shorter than input ({})", output.len(), long_input.len());
        assert!(output.contains("saved)"));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Error Type Properties Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn error_type_names_are_uppercase() {
        for error_type in ErrorType::ALL {
            let name = error_type.name();
            assert_eq!(name, name.to_uppercase(), "Error type name should be uppercase: {}", name);
        }
    }

    #[test]
    fn all_error_types_have_patterns() {
        for error_type in ErrorType::ALL {
            // Just ensure the pattern can be accessed without panic
            let _ = error_type.pattern();
        }
    }

    #[test]
    fn error_types_icons_are_valid() {
        // Some error types have icons, others use a default
        // Just ensure the icon method doesn't panic
        for error_type in ErrorType::ALL {
            let _ = error_type.icon();
        }

        // Verify specific types have custom icons
        assert!(!ErrorType::DomNesting.icon().is_empty());
        assert!(!ErrorType::Hydration.icon().is_empty());
        assert!(!ErrorType::Storybook.icon().is_empty());
    }
}
