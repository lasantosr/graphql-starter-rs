use std::num::ParseIntError;

use super::{AnsiColor, Error};

/// Iterator that consumes a sequence of numbers and emits ANSI escape sequences.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub(super) struct AnsiIter<T> {
    inner: T,
}

impl<T> AnsiIter<T>
where
    T: Iterator<Item = Result<u8, ParseIntError>>,
{
    pub(super) fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T> Iterator for AnsiIter<T>
where
    T: Iterator<Item = Result<u8, ParseIntError>>,
{
    type Item = Result<Ansi, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(Ok(code)) => Some(iter_next(code, &mut self.inner)),
            Some(Err(e)) => Some(Err(e.into())),
            None => None,
        }
    }
}

fn iter_next<I>(code: u8, iter: I) -> Result<Ansi, Error>
where
    I: Iterator<Item = Result<u8, ParseIntError>>,
{
    Ok(match code {
        0 => Ansi::Reset,
        1 => Ansi::Bold,
        2 => Ansi::Faint,
        3 => Ansi::Italic,
        4 => Ansi::Underline,
        5..=8 => Ansi::Noop,
        9 => Ansi::CrossedOut,
        10..=19 => Ansi::Noop,
        20 => Ansi::Noop,
        21 => Ansi::BoldOff,
        22 => Ansi::BoldAndFaintOff,
        23 => Ansi::ItalicOff,
        24 => Ansi::UnderlineOff,
        25..=28 => Ansi::Noop,
        29 => Ansi::CrossedOutOff,
        30..=37 => Ansi::ForgroundColor(AnsiColor::parse_4bit(code - 30)?),
        38 => Ansi::ForgroundColor(AnsiColor::parse_8bit_or_rgb(iter)?),
        39 => Ansi::DefaultForegroundColor,
        40..=47 => Ansi::BackgroundColor(AnsiColor::parse_4bit(code - 40)?),
        48 => Ansi::BackgroundColor(AnsiColor::parse_8bit_or_rgb(iter)?),
        49 => Ansi::DefaultBackgroundColor,
        50..=55 => Ansi::Noop,
        58..=59 => Ansi::Noop,
        60..=65 => Ansi::Noop,
        73..=74 => Ansi::Noop,
        90..=97 => Ansi::ForgroundColor(AnsiColor::parse_4bit_bright(code - 90)?),
        100..=107 => Ansi::BackgroundColor(AnsiColor::parse_4bit_bright(code - 100)?),
        _ => {
            return Err(Error::InvalidAnsi {
                msg: format!("Unexpected code {}", code),
            });
        }
    })
}

/// An enum encoding all supported ANSI escape codes.
///
/// See [this reference](https://stackoverflow.com/questions/4842424/list-of-ansi-color-escape-sequences).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum Ansi {
    /// Unsupported code, ignored
    Noop,

    Reset,
    Bold,
    Faint,
    Italic,
    Underline,
    // SlowBlink,
    // RapidBlink,
    // ReverseVideo,
    // Conceal,
    CrossedOut,
    // DefaultFont,
    // AlternateFont,
    // Fraktur,
    BoldOff,
    BoldAndFaintOff,
    ItalicOff,
    UnderlineOff,
    // BlinkOff,
    // InverseOff,
    // ConcealOff,
    CrossedOutOff,
    ForgroundColor(AnsiColor),
    DefaultForegroundColor,
    BackgroundColor(AnsiColor),
    DefaultBackgroundColor,
    // Framed,
    // Encircled,
    // Overlined,
    // FramedAndEncircledOff,
    // OverlinedOff,
    // IdeogramUnderline,
    // IdeogramDoubleUnderline,
    // IdeogramOverline,
    // IdeogramDoubleOverline,
    // IdeogramStressMarking,
    // IdeogramAttributesOff,
}
