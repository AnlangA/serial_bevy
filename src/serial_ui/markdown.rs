//! Markdown parsing and rendering helpers for the LLM conversation panel.

use bevy_egui::egui;
use egui::text::{LayoutJob, TextFormat};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

type MarkdownEvents<'a> = std::iter::Peekable<Parser<'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarkdownDocument {
    blocks: Vec<MarkdownBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MarkdownBlock {
    Paragraph(Vec<MarkdownInline>),
    Heading {
        level: u8,
        content: Vec<MarkdownInline>,
    },
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    Quote(Vec<MarkdownBlock>),
    List {
        ordered: bool,
        start: Option<u64>,
        items: Vec<Vec<MarkdownBlock>>,
    },
    Rule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MarkdownInline {
    Text(String),
    Code(String),
    LineBreak,
    Emphasis(Vec<MarkdownInline>),
    Strong(Vec<MarkdownInline>),
    Strikethrough(Vec<MarkdownInline>),
    Link {
        label: Vec<MarkdownInline>,
        destination: String,
    },
}

#[derive(Clone, Copy)]
enum BlockTerminator {
    Document,
    BlockQuote,
    Item,
}

#[derive(Clone, Copy)]
enum InlineTerminator {
    Paragraph,
    Heading(HeadingLevel),
    Emphasis,
    Strong,
    Strikethrough,
    Link,
}

#[derive(Clone, Copy)]
struct InlineStyle {
    font_size: f32,
    emphasis: bool,
    strong: bool,
    strikethrough: bool,
    code: bool,
    link: bool,
}

impl InlineStyle {
    const fn new(font_size: f32) -> Self {
        Self {
            font_size,
            emphasis: false,
            strong: false,
            strikethrough: false,
            code: false,
            link: false,
        }
    }
}

/// Renders Markdown content into the provided egui container.
pub fn render_markdown(
    ui: &mut egui::Ui,
    content: &str,
    default_color: egui::Color32,
    visuals: &egui::Visuals,
) {
    let document = parse_markdown(content);
    render_blocks(ui, &document.blocks, default_color, visuals);
}

fn parse_markdown(content: &str) -> MarkdownDocument {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut events = Parser::new_ext(content, options).peekable();
    MarkdownDocument {
        blocks: parse_blocks(&mut events, BlockTerminator::Document),
    }
}

fn parse_blocks<'a>(
    events: &mut MarkdownEvents<'a>,
    terminator: BlockTerminator,
) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();

    while let Some(event) = events.next() {
        match event {
            Event::End(end) if is_block_terminator(&end, terminator) => break,
            Event::Start(tag) => push_block_from_tag(events, &mut blocks, tag),
            Event::Rule => blocks.push(MarkdownBlock::Rule),
            Event::Text(text) => {
                push_nonempty_paragraph(&mut blocks, vec![MarkdownInline::Text(text.into_string())])
            }
            Event::Code(code) => {
                push_nonempty_paragraph(
                    &mut blocks,
                    vec![MarkdownInline::Code(code.into_string())],
                );
            }
            Event::SoftBreak | Event::HardBreak => {
                push_nonempty_paragraph(&mut blocks, vec![MarkdownInline::LineBreak]);
            }
            Event::TaskListMarker(checked) => push_nonempty_paragraph(
                &mut blocks,
                vec![MarkdownInline::Text(if checked {
                    "[x] ".into()
                } else {
                    "[ ] ".into()
                })],
            ),
            Event::Html(text) | Event::InlineHtml(text) => {
                push_nonempty_paragraph(&mut blocks, vec![MarkdownInline::Text(text.into_string())])
            }
            _ => {}
        }
    }

    blocks
}

fn push_block_from_tag<'a>(
    events: &mut MarkdownEvents<'a>,
    blocks: &mut Vec<MarkdownBlock>,
    tag: Tag<'a>,
) {
    match tag {
        Tag::Paragraph => blocks.push(MarkdownBlock::Paragraph(parse_inlines(
            events,
            InlineTerminator::Paragraph,
        ))),
        Tag::Heading { level, .. } => blocks.push(MarkdownBlock::Heading {
            level: heading_level(level),
            content: parse_inlines(events, InlineTerminator::Heading(level)),
        }),
        Tag::CodeBlock(kind) => blocks.push(parse_code_block(events, kind)),
        Tag::List(start) => blocks.push(parse_list(events, start)),
        Tag::BlockQuote(_) => blocks.push(MarkdownBlock::Quote(parse_blocks(
            events,
            BlockTerminator::BlockQuote,
        ))),
        _ => {}
    }
}

fn parse_code_block<'a>(events: &mut MarkdownEvents<'a>, kind: CodeBlockKind<'a>) -> MarkdownBlock {
    let language = match kind {
        CodeBlockKind::Fenced(language) if !language.is_empty() => Some(language.into_string()),
        _ => None,
    };

    let mut code = String::new();
    for event in events.by_ref() {
        match event {
            Event::End(TagEnd::CodeBlock) => break,
            Event::Text(text) | Event::Code(text) | Event::Html(text) | Event::InlineHtml(text) => {
                code.push_str(&text)
            }
            Event::SoftBreak | Event::HardBreak => code.push('\n'),
            _ => {}
        }
    }

    MarkdownBlock::CodeBlock {
        language,
        code: code.trim_end_matches('\n').to_string(),
    }
}

#[allow(clippy::while_let_on_iterator)]
fn parse_list<'a>(events: &mut MarkdownEvents<'a>, start: Option<u64>) -> MarkdownBlock {
    let mut items = Vec::new();

    while let Some(event) = events.next() {
        match event {
            Event::Start(Tag::Item) => items.push(parse_blocks(events, BlockTerminator::Item)),
            Event::End(TagEnd::List(_)) => break,
            _ => {}
        }
    }

    MarkdownBlock::List {
        ordered: start.is_some(),
        start,
        items,
    }
}

fn parse_inlines<'a>(
    events: &mut MarkdownEvents<'a>,
    terminator: InlineTerminator,
) -> Vec<MarkdownInline> {
    let mut inlines = Vec::new();

    while let Some(event) = events.next() {
        match event {
            Event::End(end) if is_inline_terminator(&end, terminator) => break,
            Event::Text(text) => inlines.push(MarkdownInline::Text(text.into_string())),
            Event::Code(code) => inlines.push(MarkdownInline::Code(code.into_string())),
            Event::SoftBreak | Event::HardBreak => inlines.push(MarkdownInline::LineBreak),
            Event::TaskListMarker(checked) => inlines.push(MarkdownInline::Text(if checked {
                "[x] ".into()
            } else {
                "[ ] ".into()
            })),
            Event::Html(text) | Event::InlineHtml(text) => {
                inlines.push(MarkdownInline::Text(text.into_string()));
            }
            Event::Start(Tag::Emphasis) => inlines.push(MarkdownInline::Emphasis(parse_inlines(
                events,
                InlineTerminator::Emphasis,
            ))),
            Event::Start(Tag::Strong) => inlines.push(MarkdownInline::Strong(parse_inlines(
                events,
                InlineTerminator::Strong,
            ))),
            Event::Start(Tag::Strikethrough) => inlines.push(MarkdownInline::Strikethrough(
                parse_inlines(events, InlineTerminator::Strikethrough),
            )),
            Event::Start(Tag::Link { dest_url, .. }) => {
                inlines.push(MarkdownInline::Link {
                    label: parse_inlines(events, InlineTerminator::Link),
                    destination: dest_url.into_string(),
                });
            }
            _ => {}
        }
    }

    inlines
}

fn is_block_terminator(end: &TagEnd, terminator: BlockTerminator) -> bool {
    matches!(
        (terminator, end),
        (BlockTerminator::BlockQuote, TagEnd::BlockQuote(_))
            | (BlockTerminator::Item, TagEnd::Item)
    )
}

fn is_inline_terminator(end: &TagEnd, terminator: InlineTerminator) -> bool {
    match terminator {
        InlineTerminator::Paragraph => matches!(end, TagEnd::Paragraph),
        InlineTerminator::Heading(level) => {
            matches!(end, TagEnd::Heading(end_level) if *end_level == level)
        }
        InlineTerminator::Emphasis => matches!(end, TagEnd::Emphasis),
        InlineTerminator::Strong => matches!(end, TagEnd::Strong),
        InlineTerminator::Strikethrough => matches!(end, TagEnd::Strikethrough),
        InlineTerminator::Link => matches!(end, TagEnd::Link),
    }
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn push_nonempty_paragraph(blocks: &mut Vec<MarkdownBlock>, inlines: Vec<MarkdownInline>) {
    if !inlines.is_empty() {
        blocks.push(MarkdownBlock::Paragraph(inlines));
    }
}

fn render_blocks(
    ui: &mut egui::Ui,
    blocks: &[MarkdownBlock],
    default_color: egui::Color32,
    visuals: &egui::Visuals,
) {
    for (index, block) in blocks.iter().enumerate() {
        match block {
            MarkdownBlock::Paragraph(inlines) => {
                render_paragraph(ui, inlines, default_color, visuals, 14.0);
            }
            MarkdownBlock::Heading { level, content } => {
                let font_size = match level {
                    1 => 22.0,
                    2 => 20.0,
                    3 => 18.0,
                    4 => 16.0,
                    5 => 15.0,
                    _ => 14.5,
                };
                let job = build_layout_job(content, default_color, visuals, font_size);
                ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Wrap));
            }
            MarkdownBlock::CodeBlock { language, code } => {
                let code_bg = if visuals.dark_mode {
                    egui::Color32::from_rgb(24, 24, 32)
                } else {
                    egui::Color32::from_rgb(236, 238, 242)
                };

                egui::Frame::new()
                    .fill(code_bg)
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        if let Some(language) = language {
                            ui.label(egui::RichText::new(language).small().weak().monospace());
                            ui.add_space(4.0);
                        }
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(code)
                                    .monospace()
                                    .color(default_color)
                                    .size(13.0),
                            )
                            .wrap_mode(egui::TextWrapMode::Wrap),
                        );
                    });
            }
            MarkdownBlock::Quote(quoted) => {
                let stroke_color = if visuals.dark_mode {
                    egui::Color32::from_gray(96)
                } else {
                    egui::Color32::from_gray(180)
                };
                let fill_color = if visuals.dark_mode {
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 10)
                } else {
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 8)
                };

                egui::Frame::new()
                    .fill(fill_color)
                    .stroke(egui::Stroke::new(1.0_f32, stroke_color))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| render_blocks(ui, quoted, default_color, visuals));
            }
            MarkdownBlock::List {
                ordered,
                start,
                items,
            } => {
                let mut number = start.unwrap_or(1);
                for item in items {
                    let marker = if *ordered {
                        let current = format!("{number}.");
                        number += 1;
                        current
                    } else {
                        "\u{2022}".to_string()
                    };

                    ui.horizontal_top(|ui| {
                        ui.label(egui::RichText::new(marker).color(default_color).strong());
                        ui.vertical(|ui| render_blocks(ui, item, default_color, visuals));
                    });
                }
            }
            MarkdownBlock::Rule => {
                ui.separator();
            }
        }

        if index + 1 < blocks.len() {
            ui.add_space(4.0);
        }
    }
}

fn render_paragraph(
    ui: &mut egui::Ui,
    inlines: &[MarkdownInline],
    default_color: egui::Color32,
    visuals: &egui::Visuals,
    font_size: f32,
) {
    let job = build_layout_job(inlines, default_color, visuals, font_size);
    ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Wrap));
}

fn build_layout_job(
    inlines: &[MarkdownInline],
    default_color: egui::Color32,
    visuals: &egui::Visuals,
    font_size: f32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    append_inlines(
        &mut job,
        inlines,
        InlineStyle::new(font_size),
        default_color,
        visuals,
    );
    job
}

fn append_inlines(
    job: &mut LayoutJob,
    inlines: &[MarkdownInline],
    style: InlineStyle,
    default_color: egui::Color32,
    visuals: &egui::Visuals,
) {
    for inline in inlines {
        match inline {
            MarkdownInline::Text(text) => append_text(job, text, style, default_color, visuals),
            MarkdownInline::Code(code) => append_text(
                job,
                code,
                InlineStyle {
                    code: true,
                    ..style
                },
                default_color,
                visuals,
            ),
            MarkdownInline::LineBreak => append_text(job, "\n", style, default_color, visuals),
            MarkdownInline::Emphasis(children) => append_inlines(
                job,
                children,
                InlineStyle {
                    emphasis: true,
                    ..style
                },
                default_color,
                visuals,
            ),
            MarkdownInline::Strong(children) => append_inlines(
                job,
                children,
                InlineStyle {
                    strong: true,
                    ..style
                },
                default_color,
                visuals,
            ),
            MarkdownInline::Strikethrough(children) => append_inlines(
                job,
                children,
                InlineStyle {
                    strikethrough: true,
                    ..style
                },
                default_color,
                visuals,
            ),
            MarkdownInline::Link { label, destination } => {
                append_inlines(
                    job,
                    label,
                    InlineStyle {
                        link: true,
                        ..style
                    },
                    default_color,
                    visuals,
                );
                if !destination.is_empty() {
                    append_text(
                        job,
                        &format!(" ({destination})"),
                        InlineStyle {
                            link: true,
                            ..style
                        },
                        default_color,
                        visuals,
                    );
                }
            }
        }
    }
}

fn append_text(
    job: &mut LayoutJob,
    text: &str,
    style: InlineStyle,
    default_color: egui::Color32,
    visuals: &egui::Visuals,
) {
    if text.is_empty() {
        return;
    }

    let mut font_size = style.font_size;
    if style.strong {
        font_size += 0.5;
    }

    let color = if style.link {
        if visuals.dark_mode {
            egui::Color32::from_rgb(125, 190, 255)
        } else {
            egui::Color32::from_rgb(0, 102, 204)
        }
    } else {
        default_color
    };

    let background = if style.code {
        if visuals.dark_mode {
            egui::Color32::from_rgb(38, 38, 46)
        } else {
            egui::Color32::from_rgb(238, 240, 244)
        }
    } else {
        egui::Color32::TRANSPARENT
    };

    job.append(
        text,
        0.0,
        TextFormat {
            font_id: if style.code {
                egui::FontId::monospace(font_size)
            } else {
                egui::FontId::proportional(font_size)
            },
            color,
            italics: style.emphasis,
            underline: if style.link {
                egui::Stroke::new(1.0_f32, color)
            } else {
                egui::Stroke::NONE
            },
            strikethrough: if style.strikethrough {
                egui::Stroke::new(1.0_f32, color)
            } else {
                egui::Stroke::NONE
            },
            background,
            ..Default::default()
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_heading_list_and_code_block() {
        let doc = parse_markdown("# Title\n\n- one\n- two\n\n```rust\nfn main() {}\n```");

        assert!(matches!(
            doc.blocks.first(),
            Some(MarkdownBlock::Heading { level: 1, .. })
        ));
        assert!(matches!(
            doc.blocks.get(1),
            Some(MarkdownBlock::List { ordered: false, .. })
        ));
        assert!(matches!(
            doc.blocks.get(2),
            Some(MarkdownBlock::CodeBlock { language: Some(language), .. }) if language == "rust"
        ));
    }

    #[test]
    fn test_parse_inline_styles() {
        let doc = parse_markdown("Hello **world** and `code`");
        let MarkdownBlock::Paragraph(inlines) = &doc.blocks[0] else {
            panic!("expected paragraph block");
        };

        assert!(
            inlines
                .iter()
                .any(|inline| matches!(inline, MarkdownInline::Strong(_)))
        );
        assert!(
            inlines
                .iter()
                .any(|inline| matches!(inline, MarkdownInline::Code(text) if text == "code"))
        );
    }

    #[test]
    fn test_parse_block_quote() {
        let doc = parse_markdown("> quoted");
        assert!(matches!(doc.blocks.first(), Some(MarkdownBlock::Quote(_))));
    }
}
