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

#[derive(Debug, Clone, Copy)]
enum ErrorType {
    DomNesting,
    Hydration,
    TypeError,
    RefError,
    SyntaxError,
    SystemError,
    Storybook,
    RuntimeError,
}

impl ErrorType {
    const ALL: &'static [ErrorType] = &[
        Self::DomNesting,
        Self::Hydration,
        Self::TypeError,
        Self::RefError,
        Self::SyntaxError,
        Self::SystemError,
        Self::Storybook,
        Self::RuntimeError,
    ];

    fn name(&self) -> &'static str {
        match self {
            Self::DomNesting => "DOM_NESTING",
            Self::Hydration => "HYDRATION",
            Self::TypeError => "TYPE_ERROR",
            Self::RefError => "REF_ERROR",
            Self::SyntaxError => "SYNTAX_ERROR",
            Self::SystemError => "SYSTEM_ERROR",
            Self::Storybook => "STORYBOOK",
            Self::RuntimeError => "RUNTIME_ERROR",
        }
    }

    fn color(&self) -> Color {
        match self {
            Self::DomNesting => Color::Yellow,
            Self::Hydration => Color::Magenta,
            Self::Storybook => Color::Cyan,
            _ => Color::Red,
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            Self::DomNesting => "󰅖",
            Self::Hydration => "󰦒",
            Self::Storybook => "󰂺",
            Self::SystemError => "",
            _ => "",
        }
    }

    fn pattern(&self) -> &Regex {
        match self {
            Self::DomNesting => &PATTERNS.dom_nesting,
            Self::Hydration => &PATTERNS.hydration,
            Self::TypeError => &PATTERNS.type_error,
            Self::RefError => &PATTERNS.ref_error,
            Self::SyntaxError => &PATTERNS.syntax_error,
            Self::SystemError => &PATTERNS.system_error,
            Self::Storybook => &PATTERNS.storybook,
            Self::RuntimeError => &PATTERNS.stack_trace,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Patterns (compiled once at startup)
// ─────────────────────────────────────────────────────────────────────────────

static PATTERNS: Lazy<Patterns> = Lazy::new(Patterns::compile);

struct Patterns {
    // Detection
    dom_nesting: Regex,
    hydration: Regex,
    type_error: Regex,
    ref_error: Regex,
    syntax_error: Regex,
    system_error: Regex,
    storybook: Regex,
    stack_trace: Regex,
    // Extraction
    file_location: Regex,
    dom_issue: Regex,
    system_code: Regex,
    storybook_code: Regex,
    user_frame: Regex,
    framework_noise: Regex,
}

impl Patterns {
    fn compile() -> Self {
        let ext = SOURCE_EXTENSIONS;
        Self {
            // Detection patterns
            dom_nesting: re(r"(?i)validateDOMNesting"),
            hydration: re(r"(?i)hydration"),
            type_error: re(r"(?m)^TypeError"),
            ref_error: re(r"(?m)^ReferenceError"),
            syntax_error: re(r"(?m)^SyntaxError"),
            system_error: re(r"ENOENT|EACCES|ECONNREFUSED"),
            storybook: re(r"SB_"),
            stack_trace: re(r"at .* \(.*:\d+:\d+\)|Error:.*at"),
            // Extraction patterns
            file_location: re(&format!(r"[A-Za-z0-9_.-]+\.({ext}):\d+")),
            dom_issue: re(r"<[a-z]+> cannot be a descendant of <[a-z]+>"),
            system_code: re(r"E[A-Z]+:[^\n]*"),
            storybook_code: re(r"SB_[A-Z_]+[^\n]*"),
            user_frame: re(&format!(r"(@|at ).+\.({ext}):\d+")),
            framework_noise: re(r"chunk-|node_modules|storybook_internal|webpack|vite|/internal"),
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
    PATTERNS.file_location.find(input).map(|m| m.as_str().to_string())
}

fn extract_issue(input: &str, error_type: ErrorType) -> Option<String> {
    match error_type {
        ErrorType::DomNesting => extract_by_pattern_or_contains(input, &PATTERNS.dom_issue, "cannot be a descendant"),
        ErrorType::Hydration => find_line_containing(input, &["hydration", "mismatch"]),
        ErrorType::TypeError | ErrorType::RefError | ErrorType::SyntaxError => {
            find_line_starting_with(input, &["TypeError", "ReferenceError", "SyntaxError"])
        }
        ErrorType::SystemError => extract_first_match(input, &PATTERNS.system_code),
        ErrorType::Storybook => extract_first_match_truncated(input, &PATTERNS.storybook_code, 100),
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
    if io::stdin().is_terminal() {
        Clipboard::new()
            .and_then(|mut c| c.get_text())
            .map_err(|_| "Failed to read clipboard")
    } else {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|_| "Failed to read stdin")?;
        Ok(buf)
    }
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
