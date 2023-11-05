//! This module helps with ANSI strings

// These modules comes from https://github.com/Aloso/to-html/blob/main/crates/ansi-to-html
crate::using! {
    ansi,
    color
}

crate::using! {
    pub text,
    minifier
}

pub use ansi_to_html::Error;

fn invalid_ansi(s: &'static str) -> impl Fn() -> Error {
    move || Error::InvalidAnsi { msg: s.to_string() }
}

/// Represents an ANSI string.
///
/// They can be created using [From]: `AnsiString::from("ansi string")`
pub struct AnsiString(String);
impl From<String> for AnsiString {
    fn from(value: String) -> Self {
        Self(value)
    }
}
impl From<&str> for AnsiString {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}
impl From<AnsiString> for String {
    fn from(value: AnsiString) -> Self {
        value.0
    }
}

impl AnsiString {
    pub fn as_ansi(&self) -> &str {
        &self.0
    }

    pub fn as_html(&self) -> Result<String, Error> {
        ansi_to_html::convert(&self.0, true, true)
    }

    pub fn as_plaintext(&self) -> String {
        strip_ansi_escapes::strip_str(&self.0)
    }

    pub fn as_styled_text(&self) -> Result<Vec<StyledText>, Error> {
        ansi_to_text(&self.0)
    }
}

#[cfg(feature = "graphql")]
#[async_graphql::Object]
/// Represents an ANSI string
impl AnsiString {
    /// ANSI representation of the string
    async fn ansi(&self) -> &str {
        self.as_ansi()
    }

    /// HTML representation of the string
    async fn html(&self) -> crate::error::GraphQLResult<String> {
        use crate::error::MapToErr;
        Ok(self.as_html().map_to_internal_err("Invalid ansi string")?)
    }

    /// Plain text representation of the string
    async fn plain(&self) -> String {
        self.as_plaintext()
    }

    /// Styled text representation of the string
    async fn text(&self) -> crate::error::GraphQLResult<Vec<StyledText>> {
        use crate::error::MapToErr;
        Ok(self.as_styled_text().map_to_internal_err("Invalid ansi string")?)
    }
}
