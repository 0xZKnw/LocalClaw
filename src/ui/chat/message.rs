//! Message display components with Markdown rendering

use crate::app::AppState;
use dioxus::prelude::*;

#[derive(Clone, PartialEq, Debug)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

// Convert storage Message to UI Message
impl From<crate::types::message::Message> for Message {
    fn from(msg: crate::types::message::Message) -> Self {
        Message {
            role: match msg.role {
                crate::types::message::Role::User => MessageRole::User,
                crate::types::message::Role::Assistant => MessageRole::Assistant,
                crate::types::message::Role::System => MessageRole::System,
            },
            content: msg.content,
        }
    }
}

// Convert UI Message to storage Message
impl From<Message> for crate::types::message::Message {
    fn from(msg: Message) -> Self {
        crate::types::message::Message::new(
            match msg.role {
                MessageRole::User => crate::types::message::Role::User,
                MessageRole::Assistant => crate::types::message::Role::Assistant,
                MessageRole::System => crate::types::message::Role::System,
            },
            msg.content,
        )
    }
}

// Content parts for parsed message content
#[derive(Clone, PartialEq, Debug)]
enum ContentPart {
    Text(String),
    Thinking(String),          // Completed <think>...</think> block
    ThinkingStreaming(String), // Open <think> block still being generated
}

/// Parse thinking blocks from message content.
/// Supports both <think>...</think> and <thinking>...</thinking> tags.
/// Incomplete tags are rendered as live streaming blocks.
/// Also strips <request>...</request> tags (rendered as normal text).
fn parse_thinking_blocks(content: &str) -> Vec<ContentPart> {
    // First: strip <request>...</request> tags, keeping inner content as normal text
    let cleaned = strip_xml_tags(content, "request");

    let mut parts = Vec::new();
    let mut remaining = cleaned.as_str();

    loop {
        // Find the earliest opening tag: <think> or <thinking>
        let think_pos = remaining.find("<think>");
        let thinking_pos = remaining.find("<thinking>");

        let (start, open_tag, close_tag) = match (think_pos, thinking_pos) {
            (Some(a), Some(b)) => {
                if a <= b {
                    (a, "<think>", "</think>")
                } else {
                    (b, "<thinking>", "</thinking>")
                }
            }
            (Some(a), None) => (a, "<think>", "</think>"),
            (None, Some(b)) => (b, "<thinking>", "</thinking>"),
            (None, None) => break,
        };

        // Text before the tag
        if start > 0 {
            let text = remaining[..start].to_string();
            if !text.trim().is_empty() {
                parts.push(ContentPart::Text(text));
            }
        }

        let open_len = open_tag.len();
        let close_len = close_tag.len();

        if let Some(end_offset) = remaining[start..].find(close_tag) {
            // Closed thinking block
            let think_start = start + open_len;
            let think_end = start + end_offset;
            let think_content = remaining[think_start..think_end].to_string();
            if !think_content.trim().is_empty() {
                parts.push(ContentPart::Thinking(think_content));
            }
            remaining = &remaining[think_end + close_len..];
        } else {
            // STREAMING: open tag without closing -> live thinking block
            let think_start = start + open_len;
            if think_start <= remaining.len() {
                let think_content = remaining[think_start..].to_string();
                parts.push(ContentPart::ThinkingStreaming(think_content));
            }
            remaining = "";
            break;
        }
    }

    if !remaining.is_empty() {
        parts.push(ContentPart::Text(remaining.to_string()));
    }

    if parts.is_empty() {
        parts.push(ContentPart::Text(content.to_string()));
    }

    parts
}

/// Strip XML-like tags, keeping the inner content as plain text.
/// e.g. strip_xml_tags("Hello <request>world</request>!", "request") -> "Hello world!"
fn strip_xml_tags(content: &str, tag: &str) -> String {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let mut result = content.to_string();
    // Remove all occurrences of opening and closing tags
    result = result.replace(&open, "");
    result = result.replace(&close, "");
    result
}

/// Collapsible thinking block component - premium style with left accent border
#[component]
fn ThinkingBlock(content: String) -> Element {
    let app_state = use_context::<AppState>();
    let is_en = app_state.settings.read().language == "en";
    let mut is_expanded = use_signal(|| false);

    let chevron_class = if is_expanded() {
        "thinking-chevron expanded"
    } else {
        "thinking-chevron"
    };

    let content_class = if is_expanded() {
        "thinking-content expanded"
    } else {
        "thinking-content"
    };

    rsx! {
        div { class: "thinking-block my-3",
            div {
                class: "thinking-header",
                onclick: move |_| is_expanded.set(!is_expanded()),

                svg {
                    class: "{chevron_class}",
                    width: "12",
                    height: "12",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2.5",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    polyline { points: "9 18 15 12 9 6" }
                }

                span { if is_en { "Thinking" } else { "Reflexion" } }
            }

            div {
                class: "{content_class}",
                div {
                    class: "text-sm text-[var(--text-secondary)] leading-relaxed px-4 pb-3",
                    MarkdownContent { content: content }
                }
            }
        }
    }
}

/// Streaming thinking block - elegant, subtle with soft animation
#[component]
fn ThinkingBlockStreaming(content: String) -> Element {
    let app_state = use_context::<AppState>();
    let is_en = app_state.settings.read().language == "en";

    let display_content = if content.trim().is_empty() {
        "...".to_string()
    } else {
        content.clone()
    };

    rsx! {
        div { class: "thinking-stream my-2",
            // Header with subtle animated dots
            div {
                class: "thinking-header",
                style: "padding: 0.5rem 0.75rem;",

                div { class: "flex items-center gap-1",
                    div {
                        class: "w-1 h-1 rounded-full animate-pulse",
                        style: "background: var(--accent-primary); opacity: 0.6;"
                    }
                    div {
                        class: "w-1 h-1 rounded-full animate-pulse delay-150",
                        style: "background: var(--accent-primary); opacity: 0.6;"
                    }
                    div {
                        class: "w-1 h-1 rounded-full animate-pulse delay-300",
                        style: "background: var(--accent-primary); opacity: 0.6;"
                    }
                }

                span {
                    class: "text-xs",
                    style: "color: var(--text-tertiary);",
                    if is_en { "Thinking..." } else { "Reflexion en cours..." }
                }
            }

            // Content - more compact
            div {
                class: "px-3 pb-2 max-h-40 overflow-y-auto scrollbar-thin",
                p {
                    class: "text-xs leading-relaxed whitespace-pre-wrap",
                    style: "color: var(--text-secondary);",
                    "{display_content}"
                }
            }
        }
    }
}

/// Markdown content renderer
#[component]
fn MarkdownContent(content: String) -> Element {
    let blocks = parse_markdown_blocks(&content);

    rsx! {
        div { class: "markdown-content space-y-3",
            for block in blocks {
                {render_block(block)}
            }
        }
    }
}

#[derive(Clone, Debug)]
enum MarkdownBlock {
    Paragraph(String),
    Heading(u8, String),
    CodeBlock(String, String), // (language, code)
    MathBlock(String),         // LaTeX math block
    UnorderedList(Vec<String>),
    OrderedList(Vec<String>),
    HorizontalRule,
    Blockquote(String),
    Table(Vec<Vec<String>>, Vec<String>), // (rows, headers)
}

/// Parse a table row into cells
fn parse_table_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

/// Check if a line is a table separator (|---|---|)
fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim().trim_matches('|');
    trimmed.split('|').all(|cell| {
        let c = cell.trim();
        c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ') && c.contains('-')
    })
}

fn parse_markdown_blocks(content: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Empty line
        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        // Math block $$...$$
        if trimmed.starts_with("$$") {
            let first_line_content = trimmed.trim_start_matches('$').trim();
            let mut math_lines = Vec::new();

            if first_line_content.ends_with("$$") {
                // Single line math block
                let math = first_line_content.trim_end_matches('$').trim();
                blocks.push(MarkdownBlock::MathBlock(math.to_string()));
                i += 1;
                continue;
            }

            if !first_line_content.is_empty() {
                math_lines.push(first_line_content.to_string());
            }
            i += 1;
            while i < lines.len() {
                let l = lines[i];
                if l.trim().contains("$$") {
                    let before_end = l.trim().trim_end_matches('$').trim();
                    if !before_end.is_empty() {
                        math_lines.push(before_end.to_string());
                    }
                    i += 1;
                    break;
                }
                math_lines.push(l.to_string());
                i += 1;
            }
            blocks.push(MarkdownBlock::MathBlock(math_lines.join("\n")));
            continue;
        }

        // Code block ```
        if trimmed.starts_with("```") {
            let lang = trimmed.trim_start_matches('`').to_string();
            let mut code_lines = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim().starts_with("```") {
                code_lines.push(lines[i]);
                i += 1;
            }
            blocks.push(MarkdownBlock::CodeBlock(lang, code_lines.join("\n")));
            i += 1;
            continue;
        }

        // Horizontal rule
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            blocks.push(MarkdownBlock::HorizontalRule);
            i += 1;
            continue;
        }

        // Heading
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            if level <= 6 {
                let text = trimmed.trim_start_matches('#').trim().to_string();
                blocks.push(MarkdownBlock::Heading(level as u8, text));
                i += 1;
                continue;
            }
        }

        // Blockquote
        if trimmed.starts_with('>') {
            let mut quote_lines = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('>') {
                quote_lines.push(lines[i].trim().trim_start_matches('>').trim());
                i += 1;
            }
            blocks.push(MarkdownBlock::Blockquote(quote_lines.join("\n")));
            continue;
        }

        // Table (lines starting with |)
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let mut table_lines: Vec<&str> = Vec::new();
            while i < lines.len() {
                let l = lines[i].trim();
                if l.starts_with('|') && l.ends_with('|') {
                    table_lines.push(l);
                    i += 1;
                } else {
                    break;
                }
            }

            if table_lines.len() >= 2 {
                // Parse header row
                let headers: Vec<String> = parse_table_row(table_lines[0]);

                // Skip separator row (|---|---|)
                let data_start = if table_lines.len() > 1 && is_table_separator(table_lines[1]) {
                    2
                } else {
                    1
                };

                // Parse data rows
                let rows: Vec<Vec<String>> = table_lines[data_start..]
                    .iter()
                    .map(|line| parse_table_row(line))
                    .collect();

                blocks.push(MarkdownBlock::Table(rows, headers));
            }
            continue;
        }

        // Unordered list
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("• ") {
            let mut items = Vec::new();
            while i < lines.len() {
                let l = lines[i].trim();
                if l.starts_with("- ") || l.starts_with("* ") || l.starts_with("• ") {
                    items.push(l[2..].to_string());
                    i += 1;
                } else if l.is_empty() || l.starts_with('#') || l.starts_with("```") {
                    break;
                } else {
                    // Continuation of previous item
                    if let Some(last) = items.last_mut() {
                        last.push(' ');
                        last.push_str(l);
                    }
                    i += 1;
                }
            }
            blocks.push(MarkdownBlock::UnorderedList(items));
            continue;
        }

        // Ordered list
        if trimmed
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
            && trimmed.contains(". ")
        {
            let mut items = Vec::new();
            while i < lines.len() {
                let l = lines[i].trim();
                if let Some(pos) = l.find(". ") {
                    if l[..pos].chars().all(|c| c.is_ascii_digit()) {
                        items.push(l[pos + 2..].to_string());
                        i += 1;
                        continue;
                    }
                }
                if l.is_empty() || l.starts_with('#') || l.starts_with("```") {
                    break;
                }
                // Continuation
                if let Some(last) = items.last_mut() {
                    last.push(' ');
                    last.push_str(l);
                }
                i += 1;
            }
            if !items.is_empty() {
                blocks.push(MarkdownBlock::OrderedList(items));
                continue;
            }
        }

        // Regular paragraph - collect until empty line or special block
        let mut para_lines = Vec::new();
        while i < lines.len() {
            let l = lines[i];
            let t = l.trim();
            if t.is_empty()
                || t.starts_with('#')
                || t.starts_with("```")
                || t.starts_with("---")
                || t.starts_with("- ")
                || t.starts_with("* ")
                || t.starts_with("> ")
            {
                break;
            }
            para_lines.push(l);
            i += 1;
        }
        if !para_lines.is_empty() {
            blocks.push(MarkdownBlock::Paragraph(para_lines.join("\n")));
        }
    }

    blocks
}

fn render_block(block: MarkdownBlock) -> Element {
    match block {
        MarkdownBlock::Paragraph(text) => rsx! {
            p { class: "text-[var(--text-primary)] leading-[1.75]",
                {render_inline(&text)}
            }
        },
        MarkdownBlock::Heading(level, text) => {
            let class = match level {
                1 => "text-2xl font-bold text-[var(--text-primary)] mt-6 mb-3",
                2 => "text-xl font-semibold text-[var(--text-primary)] mt-5 mb-2",
                3 => "text-lg font-semibold text-[var(--text-primary)] mt-4 mb-2",
                4 => "text-base font-semibold text-[var(--text-primary)] mt-3 mb-1",
                _ => "text-sm font-semibold text-[var(--text-primary)] mt-2 mb-1",
            };
            rsx! {
                div { class: "{class}",
                    {render_inline(&text)}
                }
            }
        }
        MarkdownBlock::CodeBlock(lang, code) => rsx! {
            div { class: "my-3 rounded-xl overflow-hidden border border-[var(--border-subtle)]",
                style: "background: #121110;",
                if !lang.is_empty() {
                    div { class: "code-header",
                        span { "{lang}" }
                    }
                }
                pre { class: "p-4 overflow-x-auto",
                    code { class: "text-sm font-mono leading-relaxed",
                        style: "color: #E8E2DB;",
                        "{code}"
                    }
                }
            }
        },
        MarkdownBlock::UnorderedList(items) => rsx! {
            ul { class: "space-y-1.5 pl-1",
                for item in items {
                    li { class: "flex items-start gap-2 text-[var(--text-primary)]",
                        span { class: "text-[var(--accent-primary)] mt-2 text-xs", "•" }
                        span { class: "leading-[1.75] flex-1",
                            {render_inline(&item)}
                        }
                    }
                }
            }
        },
        MarkdownBlock::OrderedList(items) => rsx! {
            ol { class: "space-y-1.5 pl-1",
                for (idx, item) in items.iter().enumerate() {
                    li { class: "flex items-start gap-2 text-[var(--text-primary)]",
                        span { class: "text-[var(--accent-primary)] font-medium text-sm min-w-[1.25rem]", "{idx + 1}." }
                        span { class: "leading-[1.75] flex-1",
                            {render_inline(item)}
                        }
                    }
                }
            }
        },
        MarkdownBlock::MathBlock(math) => rsx! {
            div { class: "my-4 p-4 rounded-xl bg-[var(--bg-tertiary)]/50 border border-[var(--border-subtle)] overflow-x-auto",
                pre { class: "font-mono text-sm text-[var(--accent-primary)] text-center whitespace-pre-wrap",
                    "{math}"
                }
            }
        },
        MarkdownBlock::HorizontalRule => rsx! {
            hr { class: "border-none h-px bg-[var(--border-subtle)] my-6" }
        },
        MarkdownBlock::Blockquote(text) => rsx! {
            blockquote { class: "border-l-3 border-[var(--accent-primary)] pl-4 py-2 my-3 bg-[var(--bg-tertiary)]/30 rounded-r-lg",
                p { class: "text-[var(--text-secondary)] italic leading-relaxed",
                    {render_inline(&text)}
                }
            }
        },
        MarkdownBlock::Table(rows, headers) => rsx! {
            div { class: "my-4 overflow-x-auto rounded-xl border border-[var(--border-subtle)]",
                table { class: "w-full text-sm",
                    thead { class: "bg-[var(--bg-tertiary)]",
                        tr {
                            for header in headers.iter() {
                                th {
                                    class: "px-4 py-3 text-left font-semibold text-[var(--text-primary)] border-b border-[var(--border-subtle)]",
                                    {render_inline(header)}
                                }
                            }
                        }
                    }
                    tbody {
                        for (row_idx, row) in rows.iter().enumerate() {
                            tr {
                                class: if row_idx % 2 == 0 { "bg-[var(--bg-secondary)]" } else { "bg-[var(--bg-primary)]" },
                                for cell in row.iter() {
                                    td {
                                        class: "px-4 py-2.5 text-[var(--text-secondary)] border-b border-[var(--border-subtle)]/50",
                                        {render_inline(cell)}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    }
}

/// Render inline markdown (bold, italic, code, links, etc.)
fn render_inline(text: &str) -> Element {
    let segments = parse_inline_markdown(text);

    rsx! {
        {segments.into_iter().map(|seg| render_segment(seg))}
    }
}

#[derive(Clone, Debug)]
enum InlineSegment {
    Text(String),
    Bold(String),
    Italic(String),
    BoldItalic(String),
    Code(String),
    Link(String, String), // (text, url)
    InlineMath(String),
}

fn parse_inline_markdown(text: &str) -> Vec<InlineSegment> {
    let mut segments = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut current_text = String::new();

    while i < chars.len() {
        // Inline code `...`
        if chars[i] == '`' && !matches!(chars.get(i + 1), Some('`')) {
            if let Some(close_offset) = chars[i + 1..].iter().position(|&c| c == '`') {
                if !current_text.is_empty() {
                    segments.push(InlineSegment::Text(current_text.clone()));
                    current_text.clear();
                }
                let start = i + 1;
                let end = i + 1 + close_offset;
                let code: String = chars[start..end].iter().collect();
                segments.push(InlineSegment::Code(code));
                i = end + 1;
                continue;
            } else {
                // Unclosed backtick, treat as normal text
                current_text.push('`');
                i += 1;
                continue;
            }
        }

        // Inline math $...$
        if chars[i] == '$' && !matches!(chars.get(i + 1), Some('$')) {
            if let Some(close_offset) = chars[i + 1..].iter().position(|&c| c == '$') {
                if !current_text.is_empty() {
                    segments.push(InlineSegment::Text(current_text.clone()));
                    current_text.clear();
                }
                let start = i + 1;
                let end = i + 1 + close_offset;
                let math: String = chars[start..end].iter().collect();
                segments.push(InlineSegment::InlineMath(math));
                i = end + 1;
                continue;
            } else {
                // Unclosed dollar, treat as normal text
                current_text.push('$');
                i += 1;
                continue;
            }
        }

        // Bold+Italic ***...***
        if i + 2 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' && chars[i + 2] == '*' {
            if !current_text.is_empty() {
                segments.push(InlineSegment::Text(current_text.clone()));
                current_text.clear();
            }
            let start = i + 3;
            i += 3;
            while i + 2 < chars.len()
                && !(chars[i] == '*' && chars[i + 1] == '*' && chars[i + 2] == '*')
            {
                i += 1;
            }
            let content: String = chars[start..i].iter().collect();
            segments.push(InlineSegment::BoldItalic(content));
            i += 3;
            continue;
        }

        // Bold **...**
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if !current_text.is_empty() {
                segments.push(InlineSegment::Text(current_text.clone()));
                current_text.clear();
            }
            let start = i + 2;
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '*') {
                i += 1;
            }
            let content: String = chars[start..i].iter().collect();
            segments.push(InlineSegment::Bold(content));
            i += 2;
            continue;
        }

        // Italic *...*
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] != '*' && chars[i + 1] != ' ' {
            if !current_text.is_empty() {
                segments.push(InlineSegment::Text(current_text.clone()));
                current_text.clear();
            }
            let start = i + 1;
            i += 1;
            while i < chars.len() && chars[i] != '*' {
                i += 1;
            }
            let content: String = chars[start..i].iter().collect();
            if !content.is_empty() && !content.starts_with(' ') && !content.ends_with(' ') {
                segments.push(InlineSegment::Italic(content));
                i += 1;
                continue;
            } else {
                // Not valid italic, treat as text
                current_text.push('*');
                current_text.push_str(&content);
                if i < chars.len() {
                    current_text.push('*');
                    i += 1;
                }
                continue;
            }
        }

        // Link [text](url)
        if chars[i] == '[' {
            let bracket_start = i;
            i += 1;
            let text_start = i;
            while i < chars.len() && chars[i] != ']' {
                i += 1;
            }
            if i < chars.len() && i + 1 < chars.len() && chars[i + 1] == '(' {
                let link_text: String = chars[text_start..i].iter().collect();
                i += 2; // skip ](
                let url_start = i;
                while i < chars.len() && chars[i] != ')' {
                    i += 1;
                }
                if i < chars.len() {
                    let url: String = chars[url_start..i].iter().collect();
                    if !current_text.is_empty() {
                        segments.push(InlineSegment::Text(current_text.clone()));
                        current_text.clear();
                    }
                    segments.push(InlineSegment::Link(link_text, url));
                    i += 1;
                    continue;
                }
            }
            // Not a valid link, backtrack
            i = bracket_start;
        }

        current_text.push(chars[i]);
        i += 1;
    }

    if !current_text.is_empty() {
        segments.push(InlineSegment::Text(current_text));
    }

    segments
}

fn render_segment(segment: InlineSegment) -> Element {
    match segment {
        InlineSegment::Text(text) => rsx! { "{text}" },
        InlineSegment::Bold(text) => rsx! {
            strong { class: "font-semibold text-[var(--text-primary)]", "{text}" }
        },
        InlineSegment::Italic(text) => rsx! {
            em { class: "italic", "{text}" }
        },
        InlineSegment::BoldItalic(text) => rsx! {
            strong { class: "font-semibold italic text-[var(--text-primary)]", "{text}" }
        },
        InlineSegment::Code(code) => rsx! {
            code { class: "px-1.5 py-0.5 rounded-md bg-[var(--bg-tertiary)] text-[var(--accent-primary)] font-mono text-[0.9em]", "{code}" }
        },
        InlineSegment::Link(text, url) => rsx! {
            a {
                href: "{url}",
                target: "_blank",
                rel: "noopener noreferrer",
                class: "text-[var(--accent-primary)] hover:underline",
                "{text}"
            }
        },
        InlineSegment::InlineMath(math) => rsx! {
            code { class: "px-1.5 py-0.5 rounded-md bg-[var(--accent-primary)]/10 text-[var(--accent-primary)] font-mono text-[0.9em] italic", "{math}" }
        },
    }
}

/// Check if content is a tool-related message
fn is_tool_message(content: &str) -> Option<ToolMessageType> {
    let trimmed = content.trim();

    // Detect by leading emoji
    if trimmed.starts_with("🔧") || trimmed.starts_with("Utilisation de l'outil") {
        return Some(ToolMessageType::InProgress);
    }
    if trimmed.starts_with('⏳') || trimmed.starts_with("Autorisation requise") {
        return Some(ToolMessageType::PermissionRequired);
    }
    if trimmed.starts_with('🚫')
        || trimmed.starts_with("Permission refusée")
        || trimmed.starts_with("Demande d'autorisation expirée")
    {
        return Some(ToolMessageType::PermissionDenied);
    }
    if trimmed.starts_with('✅') {
        return Some(ToolMessageType::Result);
    }
    if trimmed.starts_with("Résultat de `") {
        return Some(ToolMessageType::Result);
    }
    if trimmed.starts_with('❌') || trimmed.starts_with("Erreur pendant l'outil") {
        return Some(ToolMessageType::Error);
    }
    if trimmed.starts_with("Outil introuvable") {
        return Some(ToolMessageType::NotFound);
    }
    if trimmed.starts_with('⏱') {
        return Some(ToolMessageType::PermissionDenied);
    }
    None
}

#[derive(Clone, PartialEq, Debug)]
enum ToolMessageType {
    InProgress,
    PermissionRequired,
    PermissionDenied,
    Result,
    Error,
    NotFound,
}

/// Extract tool name from message content (looks for `tool_name` pattern)
fn extract_tool_name(content: &str) -> Option<String> {
    if let Some(start) = content.find('`') {
        if let Some(end) = content[start + 1..].find('`') {
            return Some(content[start + 1..start + 1 + end].to_string());
        }
    }
    None
}

/// Extract detail text after the tool name section
fn extract_detail(content: &str) -> Option<String> {
    // For results: "✅ `tool` (Xs): detail text" -> extract detail text
    // For permissions: "⏳ Autorisation ... Cible: detail" -> extract Cible value
    if let Some(pos) = content.find("Cible:") {
        let after = content[pos + 6..].trim();
        if !after.is_empty() {
            return Some(after.to_string());
        }
    }
    // For results with colon after the parenthesis: "✅ `tool` (Xs): the detail"
    if content.starts_with('✅') {
        if let Some(paren_close) = content.find("):") {
            let after = content[paren_close + 2..].trim();
            if !after.is_empty() {
                return Some(after.to_string());
            }
        }
    }
    None
}

/// Extract duration string like "(5.3s)" from content
fn extract_duration(content: &str) -> Option<String> {
    if let Some(start) = content.find('(') {
        if let Some(end) = content[start..].find("s)") {
            let dur = &content[start + 1..start + end];
            // Check it looks like a number
            if dur.parse::<f64>().is_ok() {
                return Some(format!("{}s", dur));
            }
        }
    }
    None
}

/// Extract permission level from content like "(Réseau)" or "(Écriture fichier)"
#[allow(dead_code)]
fn extract_permission_level(content: &str) -> Option<String> {
    // Look for pattern after tool name: `tool` (Level)
    if let Some(backtick_end) = content.rfind('`') {
        let after = &content[backtick_end + 1..];
        if let Some(paren_start) = after.find('(') {
            if let Some(paren_end) = after[paren_start..].find(')') {
                let level = &after[paren_start + 1..paren_start + paren_end];
                if !level.is_empty()
                    && !level.contains("itération")
                    && level.parse::<f64>().is_err()
                {
                    return Some(level.to_string());
                }
            }
        }
    }
    None
}

/// Premium tool status card component - ultra minimal design
#[component]
fn ToolCard(message_type: ToolMessageType, content: String) -> Element {
    let tool_name = extract_tool_name(&content).unwrap_or_else(|| "tool".to_string());
    let detail = extract_detail(&content);
    let duration = extract_duration(&content);

    // Minimal accent colors using CSS variables
    let (accent_var, status_icon) = match message_type {
        ToolMessageType::InProgress => ("var(--accent-primary)", "●"),
        ToolMessageType::PermissionRequired => ("var(--warning)", "◐"),
        ToolMessageType::PermissionDenied => ("var(--error)", "○"),
        ToolMessageType::Result => ("var(--success)", "●"),
        ToolMessageType::Error => ("var(--error)", "●"),
        ToolMessageType::NotFound => ("var(--warning)", "○"),
    };

    let show_spinner = message_type == ToolMessageType::InProgress;
    let is_success = message_type == ToolMessageType::Result;
    let is_error =
        message_type == ToolMessageType::Error || message_type == ToolMessageType::PermissionDenied;

    // Compute duration style outside rsx for type inference
    let duration_style = if is_success {
        "color: var(--success);"
    } else if is_error {
        "color: var(--error);"
    } else {
        "color: var(--text-tertiary);"
    };

    rsx! {
        div {
            class: "animate-fade-in",
            style: "margin: 0.35rem 0;",

            // Ultra-minimal single line
            div {
                class: "flex items-center gap-2",
                style: format!(
                    "padding: 0.4rem 0.5rem; border-left: 2px solid {}; background: linear-gradient(90deg, rgba(42,107,124,0.03) 0%, transparent 100%); border-radius: 0 8px 8px 0;",
                    accent_var
                ),

                // Status indicator - dot or spinner
                if show_spinner {
                    div {
                        class: "flex items-center gap-0.5",
                        div {
                            class: "w-1 h-1 rounded-full animate-pulse",
                            style: format!("background: {};", accent_var)
                        }
                        div {
                            class: "w-1 h-1 rounded-full animate-pulse delay-100",
                            style: format!("background: {};", accent_var)
                        }
                        div {
                            class: "w-1 h-1 rounded-full animate-pulse delay-200",
                            style: format!("background: {};", accent_var)
                        }
                    }
                } else {
                    span {
                        class: "text-[8px]",
                        style: format!("color: {}; opacity: 0.8;", accent_var),
                        "{status_icon}"
                    }
                }

                // Tool name - clean monospace
                span {
                    class: "font-mono text-xs font-medium",
                    style: format!("color: {};", accent_var),
                    "{tool_name}"
                }

                // Detail or result summary
                if let Some(ref d) = detail {
                    span {
                        class: "text-xs truncate flex-1",
                        style: "color: var(--text-secondary); max-width: 300px;",
                        "{d}"
                    }
                }

                // Right side - duration only (no verbose labels)
                div { class: "flex-1" } // spacer

                if let Some(ref dur) = duration {
                    span {
                        class: "font-mono text-[10px]",
                        style: "{duration_style}",
                        "{dur}"
                    }
                }
            }
        }
    }
}

#[component]
pub fn MessageBubble(message: Message) -> Element {
    let is_user = message.role == MessageRole::User;

    // Check if this is a tool-related message
    if !is_user {
        if let Some(tool_type) = is_tool_message(&message.content) {
            return rsx! {
                div { class: "message-layout",
                    ToolCard {
                        message_type: tool_type,
                        content: message.content.clone()
                    }
                }
            };
        }
    }

    let content_parts = if !is_user {
        parse_thinking_blocks(&message.content)
    } else {
        vec![ContentPart::Text(message.content.clone())]
    };

    if is_user {
        // User message — right-aligned, accent-tinted glass
        rsx! {
            div { class: "message-layout animate-fade-in-up",
                div { class: "flex justify-end mb-4",
                    div {
                        class: "message-user px-4 py-3 max-w-[85%]",
                        div {
                            class: "text-[15px] leading-relaxed text-[var(--text-primary)]",
                            "{message.content}"
                        }
                    }
                }
            }
        }
    } else {
        // Assistant message — with small avatar, no bubble
        rsx! {
            div { class: "message-layout animate-fade-in-up",
                div { class: "flex items-start gap-3 mb-4",
                    // LocalClaw avatar — small circle with gradient
                    div {
                        class: "flex-shrink-0 w-6 h-6 rounded-full flex items-center justify-center mt-1",
                        style: "background: var(--accent-primary); box-shadow: 0 4px 12px -4px var(--accent-glow);",
                        svg {
                            class: "w-3 h-3",
                            style: "color: #F2EDE7;",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2.5",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
                        }
                    }

                    // Content
                    div {
                        class: "flex-1 min-w-0",
                        for part in content_parts {
                            match part {
                                ContentPart::Thinking(text) => rsx! {
                                    ThinkingBlock { content: text }
                                },
                                ContentPart::ThinkingStreaming(text) => rsx! {
                                    ThinkingBlockStreaming { content: text }
                                },
                                ContentPart::Text(text) => rsx! {
                                    MarkdownContent { content: text }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}
