//! Syntax highlighting for the diff: syntect, with two-face's syntaxes (about 200
//! languages) and Catppuccin's colors (`assets/themes`): Mocha's on a dark background,
//! Latte's on a light one.
//!
//! It takes a while on big files (about 0.35 s per 5000 lines), so
//! it's meant to run once per diff, off the UI thread.

use std::io::Cursor;
use std::ops::Range;
use std::sync::LazyLock;

use iced::Color;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

use crate::git::{Diff, DiffLine};

/// The "newlines" set expects each line to end in `\n`, which some syntaxes need to
/// match line ends correctly.
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);

static MOCHA: LazyLock<Theme> =
    LazyLock::new(|| load(include_bytes!("../assets/themes/Catppuccin-Mocha.tmTheme")));
static LATTE: LazyLock<Theme> =
    LazyLock::new(|| load(include_bytes!("../assets/themes/Catppuccin-Latte.tmTheme")));

fn load(theme: &[u8]) -> Theme {
    ThemeSet::load_from_reader(&mut Cursor::new(theme)).expect("the bundled theme parses")
}

/// Which Catppuccin theme colors the code: the one made for the app's background
/// (`Colors::light`). Each is parsed the first time it's used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    Mocha,
    Latte,
}

impl Flavor {
    pub fn for_background(light: bool) -> Flavor {
        if light { Flavor::Latte } else { Flavor::Mocha }
    }

    fn theme(self) -> &'static Theme {
        match self {
            Flavor::Mocha => &MOCHA,
            Flavor::Latte => &LATTE,
        }
    }
}

/// Lines longer than this (in bytes) are left uncolored, like VS Code does, so a
/// minified file can't stall the highlighter. Skipping one doesn't break the lines
/// after it: the highlighter just carries on as if it weren't there.
const MAX_LINE_LENGTH: usize = 20_000;

/// One line's colors: byte ranges of the line's text, in order and touching, each with
/// its color. Empty for a line left uncolored.
pub type LineColors = Vec<(Range<usize>, Color)>;

/// The colors of every line of `diff` in `flavor`'s theme, in the same order as
/// `diff.lines`, or `None` if `path` isn't in a language we highlight.
///
/// Each side is highlighted as the whole file it is, so a line gets the colors it has
/// in its own file (say, inside a comment that a removed line opened): removed lines
/// from the old file (unchanged + removed lines), the rest from the new file (unchanged
/// + added lines).
pub fn diff_colors(path: &str, diff: &Diff, flavor: Flavor) -> Option<Vec<LineColors>> {
    let syntax = syntax_for(path, first_line(diff))?;
    let theme = flavor.theme();

    let mut colors = vec![LineColors::new(); diff.lines.len()];
    // The old file first, but only up to its last removed line: the lines after it are
    // all unchanged ones. The new file's pass then overwrites the unchanged lines, so
    // only the removed ones keep the old file's colors.
    let old_end = diff
        .lines
        .iter()
        .rposition(|line| matches!(line, DiffLine::Removed { .. }))
        .map_or(0, |last| last + 1);
    let old_side = &diff.lines[..old_end];
    highlight_side(syntax, theme, old_side, DiffLine::old_number, &mut colors);
    highlight_side(
        syntax,
        theme,
        &diff.lines,
        DiffLine::new_number,
        &mut colors,
    );
    Some(colors)
}

/// Whether `diff_colors` would color `diff` of the file at `path`: whether it's in a
/// language we highlight. Quick, unlike `diff_colors`.
pub fn highlights(path: &str, diff: &Diff) -> bool {
    syntax_for(path, first_line(diff)).is_some()
}

/// What the language is told from when the file name doesn't tell it (a shebang).
fn first_line(diff: &Diff) -> &str {
    diff.lines.first().map_or("", DiffLine::text)
}

/// The language of the file at `path` (a git path, `/`-separated): by its extension,
/// else its whole name (`Dockerfile`, `Makefile`), else its first line (a shebang).
/// `None` for plain text.
fn syntax_for(path: &str, first_line: &str) -> Option<&'static SyntaxReference> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let extension = name.rsplit_once('.').map(|(_, extension)| extension);

    extension
        .and_then(|extension| SYNTAXES.find_syntax_by_extension(extension))
        .or_else(|| SYNTAXES.find_syntax_by_extension(name))
        .or_else(|| SYNTAXES.find_syntax_by_first_line(first_line))
        .filter(|syntax| syntax.name != "Plain Text")
}

/// Highlights one side of the diff as one file: the `lines` that have a `number` on
/// that side, in order. Each line's colors go at its index in `colors`.
fn highlight_side(
    syntax: &SyntaxReference,
    theme: &Theme,
    lines: &[DiffLine],
    number: fn(&DiffLine) -> Option<usize>,
    colors: &mut [LineColors],
) {
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut buffer = String::new();

    for (line, line_colors) in lines.iter().zip(colors) {
        if number(line).is_some() {
            *line_colors = highlight_line(&mut highlighter, &mut buffer, line.text());
        }
    }
}

/// The next line's colors. `buffer` is only there to be reused between lines.
fn highlight_line(highlighter: &mut HighlightLines, buffer: &mut String, line: &str) -> LineColors {
    if line.len() > MAX_LINE_LENGTH {
        return LineColors::new();
    }
    buffer.clear();
    buffer.push_str(line);
    buffer.push('\n');
    // An error leaves this line uncolored; the highlighter carries on with the next.
    let Ok(pieces) = highlighter.highlight_line(buffer, &SYNTAXES) else {
        return LineColors::new();
    };

    let mut colors = LineColors::new();
    let mut start = 0;
    for (style, piece) in pieces {
        // The last piece ends with the `\n` added above, which isn't part of the line.
        let end = (start + piece.len()).min(line.len());
        let color = to_color(style.foreground);
        // Neighbours of the same color merge, so there are fewer spans to draw.
        match colors.last_mut() {
            Some((range, last)) if *last == color => range.end = end,
            _ if start < end => colors.push((start..end, color)),
            _ => {}
        }
        start = end;
    }
    colors
}

fn to_color(color: syntect::highlighting::Color) -> Color {
    Color::from_rgba8(color.r, color.g, color.b, f32::from(color.a) / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catppuccin Mocha's comment and keyword colors.
    const COMMENT: Color = Color::from_rgb8(0x93, 0x99, 0xb2);
    const KEYWORD: Color = Color::from_rgb8(0xcb, 0xa6, 0xf7);

    fn language(path: &str, first_line: &str) -> Option<&'static str> {
        syntax_for(path, first_line).map(|syntax| syntax.name.as_str())
    }

    /// A diff of added lines, as for a new file.
    fn new_file(lines: &[&str]) -> Diff {
        Diff {
            lines: lines
                .iter()
                .enumerate()
                .map(|(i, text)| DiffLine::Added {
                    new: i + 1,
                    text: text.to_string(),
                })
                .collect(),
            binary: false,
        }
    }

    fn color_at(colors: &LineColors, byte: usize) -> Option<Color> {
        colors
            .iter()
            .find(|(range, _)| range.contains(&byte))
            .map(|(_, color)| *color)
    }

    #[test]
    fn test_language_comes_from_the_name_then_the_first_line() {
        assert_eq!(language("src/main.rs", ""), Some("Rust"));
        assert_eq!(language("web/App.tsx", ""), Some("TypeScriptReact"));
        assert_eq!(language("Cargo.toml", ""), Some("TOML"));
        assert_eq!(language("docker/Dockerfile", ""), Some("Dockerfile"));
        assert_eq!(
            language("bin/deploy", "#!/bin/bash"),
            Some("Bourne Again Shell (bash)")
        );
        assert_eq!(language("notes.txt", ""), None);
        assert_eq!(language("LICENSE", "MIT License"), None);
    }

    #[test]
    fn test_unknown_languages_are_not_highlighted() {
        assert!(diff_colors("notes.txt", &new_file(&["let a = 1;"]), Flavor::Mocha).is_none());
    }

    #[test]
    fn test_latte_colors_for_a_light_background() {
        let colors =
            diff_colors("x.rs", &new_file(&["let a = 1; // a"]), Flavor::Latte).expect("Rust");
        // Latte's mauve and overlay2, the darker counterparts of Mocha's.
        assert_eq!(
            color_at(&colors[0], 0),
            Some(Color::from_rgb8(0x88, 0x39, 0xef))
        );
        assert_eq!(
            color_at(&colors[0], 12),
            Some(Color::from_rgb8(0x7c, 0x7f, 0x93))
        );
        assert_eq!(Flavor::for_background(true), Flavor::Latte);
        assert_eq!(Flavor::for_background(false), Flavor::Mocha);
    }

    #[test]
    fn test_each_side_is_highlighted_as_its_own_file() {
        let diff = Diff {
            lines: vec![
                DiffLine::Context {
                    old: 1,
                    new: 1,
                    text: "let a = 1;".to_string(),
                },
                DiffLine::Removed {
                    old: 2,
                    text: "/* old".to_string(),
                },
                DiffLine::Added {
                    new: 2,
                    text: "let b = 2;".to_string(),
                },
                DiffLine::Context {
                    old: 3,
                    new: 3,
                    text: "end */".to_string(),
                },
            ],
            binary: false,
        };
        let colors = diff_colors("x.rs", &diff, Flavor::Mocha).expect("Rust");

        assert_eq!(colors.len(), 4);
        // "old", in the comment the removed line opens.
        assert_eq!(color_at(&colors[1], 3), Some(COMMENT));
        // "let": the new file has no comment.
        assert_eq!(color_at(&colors[2], 0), Some(KEYWORD));
        // "end": in a comment in the old file only, and unchanged lines are drawn
        // with the new file's colors.
        assert_ne!(color_at(&colors[3], 0), Some(COMMENT));
    }

    #[test]
    fn test_ranges_cover_each_line_exactly() {
        let lines = ["fn main() {", "\tlet s = \"héllo\"; // ✓", "", "}\r"];
        let colors = diff_colors("x.rs", &new_file(&lines), Flavor::Mocha).expect("Rust");

        for (line, colors) in lines.iter().zip(&colors) {
            let mut end = 0;
            for (range, _) in colors {
                assert_eq!(range.start, end, "{line:?}");
                assert!(range.start < range.end, "{line:?}");
                end = range.end;
            }
            assert_eq!(end, line.len(), "{line:?}");
        }
    }

    #[test]
    fn test_long_lines_are_left_uncolored_without_breaking_the_rest() {
        let long = "x".repeat(MAX_LINE_LENGTH + 1);
        let lines = new_file(&["/* a", &long, "*/ let b = 2;"]);
        let colors = diff_colors("x.rs", &lines, Flavor::Mocha).expect("Rust");

        assert!(colors[1].is_empty());
        // Still inside the comment the first line opened, which the third one closes.
        assert_eq!(color_at(&colors[2], 0), Some(COMMENT));
        assert_eq!(color_at(&colors[2], 3), Some(KEYWORD));
    }
}
