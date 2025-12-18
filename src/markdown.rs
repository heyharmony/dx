use pulldown_cmark::{
    Event as MdEvent, HeadingLevel as MdHeadingLevel, Options as MdOptions, Parser as MdParser,
    Tag as MdTag, TagEnd as MdTagEnd,
};
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};

#[allow(dead_code)]
#[must_use]
pub fn markdown_to_text(input: &str, theme: &super::theme::ThemeTokens) -> Text<'static> {
    let (t, _) = markdown_to_text_with_links(input, theme);
    t
}

#[must_use]
#[allow(clippy::too_many_lines)] // Complex markdown parsing requires extensive logic
pub fn markdown_to_text_with_links(
    input: &str,
    theme: &super::theme::ThemeTokens,
) -> (Text<'static>, Vec<String>) {
    let mut opts = MdOptions::empty();
    opts.insert(MdOptions::ENABLE_STRIKETHROUGH);
    opts.insert(MdOptions::ENABLE_TABLES);
    opts.insert(MdOptions::ENABLE_TASKLISTS);

    let parser = MdParser::new_ext(input, opts);

    let mut text_lines: Vec<Line<'static>> = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut collected_links: Vec<String> = Vec::new();
    let mut current_link: Option<String> = None;

    #[allow(clippy::items_after_statements)] // State struct is logically placed here
    #[derive(Clone, Copy, Default)]
    #[allow(clippy::struct_excessive_bools)] // Parser state flags are justified as bools
    struct State {
        strong: bool,
        emph: bool,
        heading: u32,
        link: bool,
        code_block: bool,
        list_depth: u16,
    }
    let mut state = State::default();

    #[allow(clippy::items_after_statements)] // Helper function logically placed here
    fn push_line(text_lines: &mut Vec<Line<'static>>, current: &mut Vec<Span<'static>>) {
        text_lines.push(Line::from(std::mem::take(current)));
    }

    for ev in parser {
        match ev {
            MdEvent::Start(tag) => match tag {
                MdTag::Heading { level, .. } => {
                    state.heading = match level {
                        MdHeadingLevel::H1 => 1,
                        MdHeadingLevel::H2 => 2,
                        MdHeadingLevel::H3 => 3,
                        MdHeadingLevel::H4 => 4,
                        MdHeadingLevel::H5 => 5,
                        MdHeadingLevel::H6 => 6,
                    };
                }
                MdTag::Emphasis => state.emph = true,
                MdTag::Strong => state.strong = true,
                MdTag::Link { dest_url, .. } => {
                    state.link = true;
                    current_link = Some(dest_url.to_string());
                    current.push(Span::styled(
                        "🔗 ",
                        Style::default()
                            .fg(theme.link)
                            .add_modifier(ratatui::style::Modifier::UNDERLINED),
                    ));
                }
                MdTag::CodeBlock(_kind) => {
                    state.code_block = true;
                    if !current.is_empty() {
                        push_line(&mut text_lines, &mut current);
                    }
                }
                MdTag::List(_) => {
                    state.list_depth = state.list_depth.saturating_add(1);
                }
                MdTag::Item => {
                    let mut bullet = String::new();
                    for _ in 1..state.list_depth {
                        bullet.push_str("  ");
                    }
                    bullet.push_str("- ");
                    current.push(Span::raw(bullet));
                }
                _ => {}
            },
            MdEvent::End(tag) => match tag {
                MdTagEnd::Heading(_) => {
                    // Simulate varying heading sizes via spacing and decoration
                    let level = state.heading;
                    // Compute heading text width before we take() current line
                    let heading_width: usize = current.iter().map(|s| s.content.len()).sum();
                    // Top margin depends on heading level
                    let top_margin = match level {
                        1 => 2,
                        2 | 3 => 1,
                        _ => 0,
                    };
                    for _ in 0..top_margin {
                        text_lines.push(Line::from(""));
                    }
                    // Heading line
                    push_line(&mut text_lines, &mut current);
                    // Underline only for top levels
                    if heading_width > 0 {
                        if level == 1 {
                            let underline = "═".repeat(heading_width);
                            text_lines.push(Line::from(Span::styled(
                                underline,
                                Style::default().fg(theme.rule),
                            )));
                        } else if level == 2 {
                            let underline = "─".repeat(heading_width);
                            text_lines.push(Line::from(Span::styled(
                                underline,
                                Style::default().fg(theme.rule),
                            )));
                        }
                    }
                    // Bottom margin depends on heading level
                    let bottom_margin = match level {
                        1 => 2,
                        2 | 3 => 1,
                        _ => 0,
                    };
                    for _ in 0..bottom_margin {
                        text_lines.push(Line::from(""));
                    }
                    state.heading = 0;
                }
                MdTagEnd::Paragraph => {
                    if !current.is_empty() {
                        push_line(&mut text_lines, &mut current);
                    }
                    text_lines.push(Line::from(""));
                }
                MdTagEnd::Emphasis => state.emph = false,
                MdTagEnd::Strong => state.strong = false,
                MdTagEnd::Link => {
                    state.link = false;
                    if let Some(url) = current_link.take() {
                        let idx = collected_links.len() + 1;
                        collected_links.push(url);
                        current.push(Span::styled(
                            format!(" [{idx}↗]"),
                            Style::default().fg(theme.text_muted),
                        ));
                    }
                }
                MdTagEnd::CodeBlock => {
                    state.code_block = false;
                    push_line(&mut text_lines, &mut current);
                    text_lines.push(Line::from(""));
                }
                MdTagEnd::List(_) => {
                    state.list_depth = state.list_depth.saturating_sub(1);
                }
                MdTagEnd::Item => {
                    push_line(&mut text_lines, &mut current);
                }
                _ => {}
            },
            MdEvent::Text(t) => {
                if state.code_block {
                    for (i, l) in t.split('\n').enumerate() {
                        if i > 0 {
                            push_line(&mut text_lines, &mut current);
                        }
                        current.push(Span::styled(l.to_string(), Style::default().fg(theme.code)));
                    }
                } else {
                    let mut style = Style::default();
                    if state.strong {
                        style = style.add_modifier(ratatui::style::Modifier::BOLD);
                    }
                    if state.emph {
                        style = style.add_modifier(ratatui::style::Modifier::ITALIC);
                    }
                    if state.link {
                        style = style
                            .fg(theme.link)
                            .add_modifier(ratatui::style::Modifier::UNDERLINED);
                    }
                    // Heading styling: stronger emphasis for higher levels
                    if state.heading == 1 {
                        style = style
                            .fg(theme.md_heading1)
                            .add_modifier(ratatui::style::Modifier::BOLD);
                    } else if state.heading == 2 {
                        style = style
                            .fg(theme.md_heading2)
                            .add_modifier(ratatui::style::Modifier::BOLD);
                    } else if state.heading == 3 {
                        style = style.fg(theme.md_heading3);
                    }
                    let mut text = t.to_string();
                    // Uppercase only for top-level headings for stronger contrast
                    if state.heading == 1 || state.heading == 2 {
                        text = text.to_uppercase();
                    }
                    current.push(Span::styled(text, style));
                }
            }
            MdEvent::Code(code) => {
                current.push(Span::styled(
                    code.to_string(),
                    Style::default().fg(theme.code),
                ));
            }
            MdEvent::SoftBreak | MdEvent::HardBreak => {
                push_line(&mut text_lines, &mut current);
            }
            MdEvent::Rule => {
                current.push(Span::styled(
                    "─".repeat(40),
                    Style::default().fg(theme.rule),
                ));
                push_line(&mut text_lines, &mut current);
                text_lines.push(Line::from(""));
            }
            MdEvent::Html(html) => {
                current.push(Span::styled(
                    html.to_string(),
                    Style::default().fg(theme.text_muted),
                ));
            }
            MdEvent::TaskListMarker(done) => {
                current.push(Span::raw(if done {
                    "[x] ".to_string()
                } else {
                    "[ ] ".to_string()
                }));
            }
            _ => {}
        }
    }
    if !current.is_empty() {
        text_lines.push(Line::from(current));
    }
    if !collected_links.is_empty() {
        text_lines.push(Line::from(""));
        text_lines.push(Line::from(Span::styled(
            "Links:",
            Style::default().fg(theme.text_muted),
        )));
        for (i, url) in collected_links.iter().enumerate() {
            let mut spans: Vec<Span> = Vec::new();
            spans.push(Span::styled(
                format!("[{}] ", i + 1),
                Style::default().fg(theme.text_muted),
            ));
            spans.push(Span::styled(
                url.clone(),
                Style::default()
                    .fg(theme.link)
                    .add_modifier(ratatui::style::Modifier::UNDERLINED),
            ));
            text_lines.push(Line::from(spans));
        }
    }
    (Text::from(text_lines), collected_links)
}

// Backwards-compat: tests may call old signature without theme; use builtin dark theme
#[allow(dead_code)]
#[must_use]
pub fn markdown_to_text_with_links_compat(input: &str) -> (Text<'static>, Vec<String>) {
    let theme = super::theme::ThemeTokens::builtin_dark();
    markdown_to_text_with_links(input, &theme)
}
