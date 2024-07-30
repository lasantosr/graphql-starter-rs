use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::{Ansi, AnsiIter, Color, Error, Minifier};

/// A text string that contains an optional style
#[derive(Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "graphql", derive(async_graphql::SimpleObject))]
pub struct StyledText {
    /// The text
    pub text: String,
    /// The optional style
    pub style: Option<TextStyle>,
}

/// The style of a text
#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "graphql", derive(async_graphql::SimpleObject))]
pub struct TextStyle {
    /// The foreground hex color
    pub fg: Option<String>,
    /// The background hex color
    pub bg: Option<String>,
    /// The text effects
    pub effects: TextEffects,
}

/// The effects of a text
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "graphql", derive(async_graphql::SimpleObject))]
pub struct TextEffects {
    /// Bold or increased intensity
    pub bold: bool,
    /// Faint, decreased intensity, or dim
    pub faint: bool,
    /// Italic
    pub italic: bool,
    /// Underlined
    pub underline: bool,
    /// Strike or crossed-out
    pub strikethrough: bool,
}

// From here on, based on https://github.com/Aloso/to-html/blob/main/crates/ansi-to-html/src/html/mod.rs

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum AnsiStyle {
    Bold,
    Faint,
    Italic,
    Underline,
    CrossedOut,
    ForegroundColor(Color),
    BackgroundColor(Color),
}

impl From<&Vec<AnsiStyle>> for TextStyle {
    fn from(styles: &Vec<AnsiStyle>) -> Self {
        let mut ret = Self::default();
        for s in styles {
            match s {
                AnsiStyle::Bold => ret.effects.bold = true,
                AnsiStyle::Faint => ret.effects.faint = true,
                AnsiStyle::Italic => ret.effects.italic = true,
                AnsiStyle::Underline => ret.effects.underline = true,
                AnsiStyle::CrossedOut => ret.effects.strikethrough = true,
                AnsiStyle::ForegroundColor(fg) => ret.fg = Some(fg.to_string()),
                AnsiStyle::BackgroundColor(bg) => ret.bg = Some(bg.to_string()),
            }
        }
        ret
    }
}

#[derive(Debug, Default)]
pub(super) struct AnsiConverter {
    styles: Vec<AnsiStyle>,
    current_text: String,
    result: Vec<StyledText>,
}

impl AnsiConverter {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn consume_ansi_code(&mut self, ansi: Ansi) {
        match ansi {
            Ansi::Noop => {}
            Ansi::Reset => self.clear_style(|_| true),
            Ansi::Bold => self.set_style(AnsiStyle::Bold),
            Ansi::Faint => self.set_style(AnsiStyle::Faint),
            Ansi::Italic => self.set_style(AnsiStyle::Italic),
            Ansi::Underline => self.set_style(AnsiStyle::Underline),
            Ansi::CrossedOut => self.set_style(AnsiStyle::CrossedOut),
            Ansi::BoldOff => self.clear_style(|&s| s == AnsiStyle::Bold),
            Ansi::BoldAndFaintOff => self.clear_style(|&s| s == AnsiStyle::Bold || s == AnsiStyle::Faint),
            Ansi::ItalicOff => self.clear_style(|&s| s == AnsiStyle::Italic),
            Ansi::UnderlineOff => self.clear_style(|&s| s == AnsiStyle::Underline),
            Ansi::CrossedOutOff => self.clear_style(|&s| s == AnsiStyle::CrossedOut),
            Ansi::ForgroundColor(c) => self.set_style(AnsiStyle::ForegroundColor(c)),
            Ansi::DefaultForegroundColor => self.clear_style(|&s| matches!(s, AnsiStyle::ForegroundColor(_))),
            Ansi::BackgroundColor(c) => self.set_style(AnsiStyle::BackgroundColor(c)),
            Ansi::DefaultBackgroundColor => self.clear_style(|&s| matches!(s, AnsiStyle::BackgroundColor(_))),
        }
    }

    pub(super) fn push_str(&mut self, s: &str) {
        self.current_text.push_str(s);
    }

    fn set_style(&mut self, s: AnsiStyle) {
        if !self.styles.contains(&s) {
            self.checkpoint();
            self.styles.push(s);
        }
    }

    fn clear_style(&mut self, mut cond: impl Fn(&AnsiStyle) -> bool) {
        if self.styles.iter().any(&mut cond) {
            self.checkpoint();
            self.styles.retain(|s| !cond(s));
        }
    }

    fn checkpoint(&mut self) {
        if !self.current_text.is_empty() {
            let text = std::mem::take(&mut self.current_text);
            self.result.push(StyledText {
                text,
                style: if self.styles.is_empty() {
                    None
                } else {
                    Some((&self.styles).into())
                },
            })
        }
    }

    pub(super) fn result(mut self) -> Vec<StyledText> {
        self.checkpoint();
        self.result
    }
}

static ANSI_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("\x1b(\\[[0-9;?]*[A-HJKSTfhilmnsu]|\\(B)").unwrap());

/// Convert ANSI sequences to styled text.
pub fn ansi_to_text(mut input: &str) -> Result<Vec<StyledText>, Error> {
    let mut minifier = Minifier::new();

    loop {
        match ANSI_REGEX.find(input) {
            Some(m) => {
                if m.start() > 0 {
                    let (before, after) = input.split_at(m.start());
                    minifier.push_str(before);
                    input = after;
                }

                let len = m.range().len();
                input = &input[len..];

                if !m.as_str().ends_with('m') {
                    continue;
                }

                if len == 3 {
                    minifier.clear_styles();
                    continue;
                }

                let nums = &m.as_str()[2..len - 1];
                let nums = nums.split(';').map(|n| n.parse::<u8>());

                for ansi in AnsiIter::new(nums) {
                    minifier.push_ansi_code(ansi?);
                }
            }
            None => {
                minifier.push_str(input);
                break;
            }
        }
    }
    minifier.push_ansi_code(Ansi::Reset); // make sure all tags are closed

    Ok(minifier.result())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let text = "[2m1970-01-01T00:00:00.000000Z[0m [32m INFO[0m \
                    [2mgraphql_starter::ansi::text::tests[0m[2m:[0m test-event-#1";
        let mut res = ansi_to_text(text).unwrap();

        let date = res.remove(0);
        assert_eq!(
            date,
            StyledText {
                text: "1970-01-01T00:00:00.000000Z".into(),
                style: Some(TextStyle {
                    fg: None,
                    bg: None,
                    effects: TextEffects {
                        bold: false,
                        faint: true,
                        italic: false,
                        underline: false,
                        strikethrough: false,
                    },
                },),
            }
        );

        let whitespace = res.remove(0);
        assert_eq!(
            whitespace,
            StyledText {
                text: " ".into(),
                style: None
            }
        );

        let level = res.remove(0);
        assert_eq!(
            level,
            StyledText {
                text: " INFO".into(),
                style: Some(TextStyle {
                    fg: Some("#0a0".into(),),
                    bg: None,
                    effects: TextEffects {
                        bold: false,
                        faint: false,
                        italic: false,
                        underline: false,
                        strikethrough: false,
                    },
                },),
            }
        );

        let whitespace = res.remove(0);
        assert_eq!(
            whitespace,
            StyledText {
                text: " ".into(),
                style: None
            }
        );

        let location = res.remove(0);
        assert_eq!(
            location,
            StyledText {
                text: "graphql_starter::ansi::text::tests:".into(),
                style: Some(TextStyle {
                    fg: None,
                    bg: None,
                    effects: TextEffects {
                        bold: false,
                        faint: true,
                        italic: false,
                        underline: false,
                        strikethrough: false,
                    },
                },),
            }
        );

        let log = res.remove(0);
        assert_eq!(
            log,
            StyledText {
                text: " test-event-#1".into(),
                style: None
            }
        );
    }
}
