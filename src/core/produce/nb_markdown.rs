use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd, html};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::core::util::append_log_line;
use crate::data::assets::lang::{sanitize_ui_text, t, tf};
use crate::{MainWindow, MarkdownPreviewRow};

const DOCUMENT_STYLE: &str = r#"
html { color-scheme: light; }
* { box-sizing: border-box; }
body {
  max-width: 980px; margin: 0 auto; padding: 48px 32px;
  color: #1f2328; background: #ffffff;
  font: 16px/1.65 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  overflow-wrap: break-word;
}
a { color: #0969da; text-decoration: none; }
a:hover { text-decoration: underline; }
h1, h2, h3, h4, h5, h6 { margin: 1.45em 0 .65em; line-height: 1.25; }
h1, h2 { padding-bottom: .3em; border-bottom: 1px solid #d0d7de; }
h1 { font-size: 2em; } h2 { font-size: 1.5em; } h3 { font-size: 1.25em; }
p, ul, ol, blockquote, table, pre { margin: 0 0 1em; }
ul, ol { padding-left: 2em; }
li + li { margin-top: .25em; }
input[type="checkbox"] { margin: 0 .45em 0 0; vertical-align: middle; }
blockquote { padding: 0 1em; color: #57606a; border-left: .25em solid #d0d7de; }
code, pre { font-family: "Cascadia Code", Consolas, "Courier New", monospace; }
code { padding: .18em .38em; background: #afb8c133; border-radius: 4px; font-size: 85%; }
pre { padding: 16px; overflow: auto; background: #f6f8fa; border-radius: 6px; }
pre code { padding: 0; background: transparent; font-size: inherit; }
table { display: block; max-width: 100%; overflow: auto; border-spacing: 0; border-collapse: collapse; }
th, td { padding: 6px 13px; border: 1px solid #d0d7de; }
th { background: #f6f8fa; font-weight: 600; }
tr:nth-child(2n) { background: #f6f8fa; }
img { max-width: 100%; height: auto; }
hr { height: .25em; margin: 1.5em 0; border: 0; background: #d0d7de; }
"#;

const DARK_DOCUMENT_STYLE: &str = r#"
html { color-scheme: dark; }
body { color: #f0f6fc; background: #0d1117; }
a { color: #58a6ff; } h1, h2 { border-color: #30363d; }
blockquote { color: #8b949e; border-color: #3b434b; }
code, pre, th, tr:nth-child(2n) { background: #161b22; }
th, td { border-color: #30363d; } hr { background: #30363d; }
"#;

#[derive(Clone, Copy)]
enum HtmlTheme {
    Light,
    Dark,
}

impl HtmlTheme {
    fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    fn file_suffix(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    fn document_title_suffix(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    fn exported_status_key(self) -> &'static str {
        match self {
            Self::Light => "markdown.status.light_exported",
            Self::Dark => "markdown.status.dark_exported",
        }
    }

    fn export_done_key(self) -> &'static str {
        match self {
            Self::Light => "markdown.msg.export_light_done",
            Self::Dark => "markdown.msg.export_dark_done",
        }
    }
}

#[derive(Clone)]
struct MarkdownCandidate {
    source_path: PathBuf,
}

#[derive(Clone)]
struct PreviewRow {
    source_name: String,
    light_html_name: String,
    dark_html_name: String,
    status_text: String,
    has_error: bool,
}

#[derive(Clone)]
struct MarkdownState {
    candidates: Vec<MarkdownCandidate>,
    excluded_indices: HashSet<usize>,
}

impl MarkdownState {
    fn visible_indices(&self) -> Vec<usize> {
        self.candidates
            .iter()
            .enumerate()
            .filter_map(|(index, _)| (!self.excluded_indices.contains(&index)).then_some(index))
            .collect()
    }

    fn pending_rows(&self, language_index: i32) -> Vec<PreviewRow> {
        self.visible_indices()
            .into_iter()
            .map(|index| {
                let candidate = &self.candidates[index];
                let (light_html_name, dark_html_name) = output_names_for(&candidate.source_path, 1);
                PreviewRow {
                    source_name: file_name(&candidate.source_path),
                    light_html_name,
                    dark_html_name,
                    status_text: t(language_index, "markdown.status.pending"),
                    has_error: false,
                }
            })
            .collect()
    }

    fn selected_candidates(&self) -> Vec<MarkdownCandidate> {
        self.visible_indices()
            .into_iter()
            .map(|index| self.candidates[index].clone())
            .collect()
    }

    fn exclude_visible_row(&mut self, visible_row_index: usize) -> Option<String> {
        let visible_indices = self.visible_indices();
        let excluded_index = *visible_indices.get(visible_row_index)?;
        let name = file_name(&self.candidates[excluded_index].source_path);
        self.excluded_indices.insert(excluded_index);
        Some(name)
    }
}

fn append_markdown_status_log(ui: &MainWindow, message: &str) {
    ui.set_markdown_status_text(
        append_log_line(ui.get_markdown_status_text().as_ref(), message).into(),
    );
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn output_name_for(source: &Path, number: usize, theme: HtmlTheme) -> String {
    let stem = source
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| std::ffi::OsStr::new("markdown"));
    let stem = stem.to_string_lossy();
    if number == 1 {
        format!("{stem}_{}.html", theme.file_suffix())
    } else {
        format!("{stem}_{number}_{}.html", theme.file_suffix())
    }
}

fn output_names_for(source: &Path, number: usize) -> (String, String) {
    (
        output_name_for(source, number, HtmlTheme::Light),
        output_name_for(source, number, HtmlTheme::Dark),
    )
}

fn collect_markdown_candidates(source_path: &Path) -> Result<Vec<MarkdownCandidate>, String> {
    let mut candidates = Vec::new();

    if source_path.is_dir() {
        let entries = fs::read_dir(source_path).map_err(|error| error.to_string())?;
        for entry in entries {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_file() && is_markdown_file(&path) {
                candidates.push(MarkdownCandidate { source_path: path });
            }
        }
    } else if source_path.is_file() && is_markdown_file(source_path) {
        candidates.push(MarkdownCandidate {
            source_path: source_path.to_path_buf(),
        });
    } else {
        return Err("Input path is not a Markdown file or directory".to_string());
    }

    candidates.sort_by_key(|candidate| file_name(&candidate.source_path).to_lowercase());
    Ok(candidates)
}

fn escaped_html_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn extract_bracket_math(markdown: &str, options: Options) -> (String, Vec<(String, String)>) {
    let mut protected = Vec::new();
    let mut code_block_start = None;
    for (event, range) in Parser::new_ext(markdown, options).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(_)) => code_block_start = Some(range.start),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = code_block_start.take() {
                    protected.push(start..range.end);
                }
            }
            Event::Code(_) => protected.push(range),
            _ => {}
        }
    }

    let mut normalized = String::with_capacity(markdown.len());
    let mut formulas = Vec::new();
    let mut marker_prefix = String::from("NEWBEEMATHPLACEHOLDER");
    while markdown.contains(&marker_prefix) {
        marker_prefix.push('X');
    }
    let mut index = 0;
    while index < markdown.len() {
        let rest = &markdown[index..];
        let (opening, closing, bare, display) = if rest.starts_with("\\[") {
            (2, "\\]", false, true)
        } else if rest.starts_with("\\(") {
            (2, "\\)", false, false)
        } else if rest.starts_with('[') {
            (1, "]", true, true)
        } else {
            (0, "", false, false)
        };

        if opening != 0 {
            if let Some(end_offset) = rest[opening..].find(closing) {
                let content_end = index + opening + end_offset;
                let end = content_end + closing.len();
                let content = &markdown[index + opening..content_end];
                let is_matrix = content.starts_with(char::is_whitespace)
                    && content.ends_with(char::is_whitespace)
                    && content.contains("\\begin{")
                    && content.contains("\\end{");
                if (!bare || is_matrix)
                    && !protected
                        .iter()
                        .any(|range| range.start < end && index < range.end)
                {
                    let marker = format!("{marker_prefix}{}END", formulas.len());
                    normalized.push_str(&marker);
                    formulas.push((marker, math_fragment(content, display)));
                    index = end;
                    continue;
                }
            }
        }

        let character = rest.chars().next().expect("index is inside markdown");
        normalized.push(character);
        index += character.len_utf8();
    }
    (normalized, formulas)
}

fn math_fragment(formula: &str, display: bool) -> String {
    let (opening, closing, class) = if display {
        ("\\[", "\\]", "math-display")
    } else {
        ("\\(", "\\)", "math-inline")
    };
    format!(
        "<span class=\"math {class}\">{opening}{}{closing}</span>",
        escaped_html_text(formula)
    )
}

fn math_html(event: Event<'_>) -> Event<'_> {
    let (formula, display) = match event {
        Event::InlineMath(formula) => (formula, false),
        Event::DisplayMath(formula) => (formula, true),
        other => return other,
    };
    Event::Html(CowStr::Boxed(
        math_fragment(&formula, display).into_boxed_str(),
    ))
}

fn render_html_document(markdown: &str, title: &str, dark_theme: bool) -> String {
    let mut options = Options::empty();
    options.insert(
        Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_SMART_PUNCTUATION
            | Options::ENABLE_HEADING_ATTRIBUTES
            | Options::ENABLE_MATH,
    );

    let (markdown, formulas) = extract_bracket_math(markdown, options);
    let mut body = String::new();
    html::push_html(
        &mut body,
        Parser::new_ext(&markdown, options).map(math_html),
    );
    for (marker, fragment) in formulas {
        body = body.replace(&marker, &fragment);
    }
    let math_script = if body.contains("class=\"math ") {
        "<script defer src=\"https://cdn.jsdelivr.net/npm/mathjax@3.2.2/es5/tex-chtml.js\"></script>\n"
    } else {
        ""
    };

    format!(
        "<!doctype html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<style>{}{}</style>\n{}</head>\n<body>\n{}\n</body>\n</html>\n",
        escaped_html_text(title),
        DOCUMENT_STYLE,
        if dark_theme { DARK_DOCUMENT_STYLE } else { "" },
        math_script,
        body
    )
}

fn make_unique_output_path(output_dir: &Path, source: &Path, theme: HtmlTheme) -> PathBuf {
    let mut number = 1usize;
    loop {
        let destination = output_dir.join(output_name_for(source, number, theme));
        if !destination.exists() {
            return destination;
        }
        number += 1;
    }
}

fn export_markdown_document(
    source: &Path,
    destination: &Path,
    theme: HtmlTheme,
) -> Result<(), String> {
    let markdown = fs::read_to_string(source).map_err(|error| error.to_string())?;
    let title = source
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Markdown".to_string());
    fs::write(
        destination,
        render_html_document(
            &markdown,
            &format!("{title} — {}", theme.document_title_suffix()),
            theme.is_dark(),
        ),
    )
    .map_err(|error| format!("{} HTML: {error}", theme.document_title_suffix()))
}

fn set_preview_rows(ui: &MainWindow, rows: Vec<PreviewRow>) {
    let rows = rows
        .into_iter()
        .map(|row| MarkdownPreviewRow {
            source_name: sanitize_ui_text(&row.source_name).into(),
            light_html_name: sanitize_ui_text(&row.light_html_name).into(),
            dark_html_name: sanitize_ui_text(&row.dark_html_name).into(),
            status_text: sanitize_ui_text(&row.status_text).into(),
            has_error: row.has_error,
        })
        .collect::<Vec<_>>();
    ui.set_markdown_preview_rows(ModelRc::new(VecModel::from(rows)));
}

fn export_selected_markdown(
    ui: &MainWindow,
    markdown_state: &Rc<RefCell<Option<MarkdownState>>>,
    output: &str,
    theme: HtmlTheme,
) {
    let output = output.trim();
    if output.is_empty() {
        append_markdown_status_log(
            ui,
            &t(ui.get_language_index(), "markdown.msg.output_required"),
        );
        return;
    }

    let selected = {
        let state = markdown_state.borrow();
        let Some(state) = state.as_ref() else {
            append_markdown_status_log(ui, &t(ui.get_language_index(), "markdown.msg.scan_first"));
            return;
        };
        state.selected_candidates()
    };
    if selected.is_empty() {
        append_markdown_status_log(
            ui,
            &t(ui.get_language_index(), "markdown.msg.no_selected_items"),
        );
        return;
    }

    let output_dir = PathBuf::from(output);
    if let Err(error) = fs::create_dir_all(&output_dir) {
        append_markdown_status_log(
            ui,
            &tf(
                ui.get_language_index(),
                "markdown.msg.create_output_dir_failed",
                &[("error", &error.to_string())],
            ),
        );
        return;
    }

    let mut rows = Vec::with_capacity(selected.len());
    let mut success_count = 0usize;
    let mut failed_count = 0usize;
    for candidate in selected {
        let source_name = file_name(&candidate.source_path);
        let (mut light_html_name, mut dark_html_name) = output_names_for(&candidate.source_path, 1);
        let destination = make_unique_output_path(&output_dir, &candidate.source_path, theme);
        if theme.is_dark() {
            dark_html_name = file_name(&destination);
        } else {
            light_html_name = file_name(&destination);
        }

        match export_markdown_document(&candidate.source_path, &destination, theme) {
            Ok(()) => {
                success_count += 1;
                rows.push(PreviewRow {
                    source_name,
                    light_html_name,
                    dark_html_name,
                    status_text: t(ui.get_language_index(), theme.exported_status_key()),
                    has_error: false,
                });
            }
            Err(error) => {
                failed_count += 1;
                append_markdown_status_log(
                    ui,
                    &tf(
                        ui.get_language_index(),
                        "markdown.msg.export_item_failed",
                        &[("name", &source_name), ("error", &error)],
                    ),
                );
                rows.push(PreviewRow {
                    source_name,
                    light_html_name,
                    dark_html_name,
                    status_text: t(ui.get_language_index(), "markdown.status.failed"),
                    has_error: true,
                });
            }
        }
    }

    set_preview_rows(ui, rows);
    let success = success_count.to_string();
    let output_dir = output_dir.display().to_string();
    append_markdown_status_log(
        ui,
        &tf(
            ui.get_language_index(),
            theme.export_done_key(),
            &[("count", &success), ("path", &output_dir)],
        ),
    );
    if failed_count > 0 {
        let failed = failed_count.to_string();
        append_markdown_status_log(
            ui,
            &tf(
                ui.get_language_index(),
                "markdown.msg.export_failed_summary",
                &[("count", &failed)],
            ),
        );
    }
}

pub fn setup_markdown_handlers(ui: &MainWindow) {
    let markdown_state: Rc<RefCell<Option<MarkdownState>>> = Rc::new(RefCell::new(None));

    ui.set_markdown_status_text("".into());
    ui.set_markdown_preview_rows(ModelRc::new(VecModel::from(
        Vec::<MarkdownPreviewRow>::new(),
    )));
    append_markdown_status_log(ui, &t(ui.get_language_index(), "markdown.msg.ready"));

    {
        let ui_handle = ui.as_weak();
        let markdown_state = Rc::clone(&markdown_state);
        ui.on_markdown_scan_request(move |source| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let source_path = PathBuf::from(source.as_str().trim());
            if source_path.as_os_str().is_empty() {
                *markdown_state.borrow_mut() = None;
                set_preview_rows(&ui, Vec::new());
                append_markdown_status_log(
                    &ui,
                    &t(ui.get_language_index(), "markdown.msg.input_required"),
                );
                return;
            }

            if !source_path.exists() {
                *markdown_state.borrow_mut() = None;
                set_preview_rows(&ui, Vec::new());
                append_markdown_status_log(
                    &ui,
                    &t(ui.get_language_index(), "markdown.msg.input_invalid"),
                );
                return;
            }
            if source_path.is_file() && !is_markdown_file(&source_path) {
                *markdown_state.borrow_mut() = None;
                set_preview_rows(&ui, Vec::new());
                append_markdown_status_log(
                    &ui,
                    &t(ui.get_language_index(), "markdown.msg.input_not_markdown"),
                );
                return;
            }

            match collect_markdown_candidates(&source_path) {
                Ok(candidates) if candidates.is_empty() => {
                    *markdown_state.borrow_mut() = None;
                    set_preview_rows(&ui, Vec::new());
                    append_markdown_status_log(
                        &ui,
                        &t(ui.get_language_index(), "markdown.msg.no_markdown_files"),
                    );
                }
                Ok(candidates) => {
                    let state = MarkdownState {
                        candidates,
                        excluded_indices: HashSet::new(),
                    };
                    let count = state.candidates.len().to_string();
                    let rows = state.pending_rows(ui.get_language_index());
                    set_preview_rows(&ui, rows);
                    *markdown_state.borrow_mut() = Some(state);
                    append_markdown_status_log(
                        &ui,
                        &tf(
                            ui.get_language_index(),
                            "markdown.msg.scan_success",
                            &[("count", &count)],
                        ),
                    );
                }
                Err(error) => {
                    *markdown_state.borrow_mut() = None;
                    set_preview_rows(&ui, Vec::new());
                    append_markdown_status_log(
                        &ui,
                        &tf(
                            ui.get_language_index(),
                            "markdown.msg.scan_failed",
                            &[("error", &error)],
                        ),
                    );
                }
            }
        });
    }

    {
        let ui_handle = ui.as_weak();
        let markdown_state = Rc::clone(&markdown_state);
        ui.on_markdown_remove_row_request(move |index| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let mut state = markdown_state.borrow_mut();
            let Some(state) = state.as_mut() else {
                append_markdown_status_log(
                    &ui,
                    &t(ui.get_language_index(), "markdown.msg.scan_first"),
                );
                return;
            };
            let Some(name) = state.exclude_visible_row(index as usize) else {
                return;
            };

            set_preview_rows(&ui, state.pending_rows(ui.get_language_index()));
            append_markdown_status_log(
                &ui,
                &tf(
                    ui.get_language_index(),
                    "markdown.msg.row_removed",
                    &[("name", &name)],
                ),
            );
        });
    }

    for theme in [HtmlTheme::Light, HtmlTheme::Dark] {
        let ui_handle = ui.as_weak();
        let markdown_state = Rc::clone(&markdown_state);
        match theme {
            HtmlTheme::Light => ui.on_markdown_export_light_request(move |output| {
                let Some(ui) = ui_handle.upgrade() else {
                    return;
                };
                export_selected_markdown(&ui, &markdown_state, output.as_str(), HtmlTheme::Light);
            }),
            HtmlTheme::Dark => ui.on_markdown_export_dark_request(move |output| {
                let Some(ui) = ui_handle.upgrade() else {
                    return;
                };
                export_selected_markdown(&ui, &markdown_state, output.as_str(), HtmlTheme::Dark);
            }),
        }
    }

    {
        let ui_handle = ui.as_weak();
        let markdown_state = Rc::clone(&markdown_state);
        ui.on_markdown_clear_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            *markdown_state.borrow_mut() = None;
            set_preview_rows(&ui, Vec::new());
            append_markdown_status_log(&ui, &t(ui.get_language_index(), "markdown.msg.cleared"));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::render_html_document;

    #[test]
    fn renders_common_markdown_extensions_in_a_complete_document() {
        let document = render_html_document(
            "# Heading\n\n- [x] Done\n\n| A | B |\n| - | - |\n| 1 | 2 |\n\n~~old~~",
            "Example",
            false,
        );

        assert!(document.starts_with("<!doctype html>"));
        assert!(document.contains("<h1>Heading</h1>"));
        assert!(document.contains("type=\"checkbox\""));
        assert!(document.contains("checked=\"\""));
        assert!(document.contains("<table>"));
        assert!(document.contains("<del>old</del>"));
    }

    #[test]
    fn renders_bracketed_matrix_and_preserves_code_examples() {
        let matrix = r"[ \mathbf{\Omega}(t)= \begin{bmatrix} x_1(t)\\ x_2(t)\\ \vdots\\ x_n(t) \end{bmatrix} ]";
        let markdown = format!("{matrix}\n\n\\[x^2\\]\n\n`{matrix}`\n\n```tex\n{matrix}\n```");
        let document = render_html_document(&markdown, "Matrix", true);

        assert!(document.contains("mathjax@3.2.2"));
        assert!(document.contains("class=\"math math-display\""));
        assert!(document.contains(r"\mathbf{\Omega}(t)="));
        assert!(document.contains(r"\begin{bmatrix} x_1(t)\\ x_2(t)"));
        assert!(document.contains(r"\[x^2\]"));
        assert!(document.contains("<code>[ \\mathbf"));
        assert!(document.contains("<pre><code class=\"language-tex\">[ \\mathbf"));
    }

    #[test]
    fn renders_multiline_display_and_inline_latex_delimiters() {
        let markdown = "\\[\n\\frac{d\\mathbf X}{dt}\n=\n\\mathbf F(\\mathbf X,t,\\boldsymbol{\\theta})\n+\n\\boldsymbol{\\eta}(t)\n\\]\n\n其中 \\(\\mathbf X\\) 表示系统状态。";
        let document = render_html_document(markdown, "Equation", false);

        assert!(document.contains("class=\"math math-display\""));
        assert!(document.contains(r"\frac{d\mathbf X}{dt}"));
        assert!(document.contains(r"\boldsymbol{\eta}(t)"));
        assert!(document.contains("class=\"math math-inline\""));
        assert!(document.contains(r"\(\mathbf X\)"));
    }
}
