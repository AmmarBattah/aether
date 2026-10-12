use std::borrow::Cow;
use std::fmt;
use std::ops::Deref;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeSeparator {
    Dot,
    Arrow,
}

impl NoticeSeparator {
    fn glyph(self) -> &'static str {
        match self {
            Self::Dot => "·",
            Self::Arrow => "→",
        }
    }
}

/// Only separators authored by the TUI change with the glyph set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeText {
    text: String,
    separators: Vec<(usize, NoticeSeparator)>,
    detail_payload: Option<String>,
}

impl NoticeText {
    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub(super) fn with_detail_payload(mut self, payload: String) -> Self {
        self.detail_payload = Some(payload);
        self
    }

    pub fn detail_payload(&self) -> Option<&str> {
        self.detail_payload.as_deref()
    }

    pub fn joined(parts: &[&str], separator: NoticeSeparator) -> Self {
        let mut out = Self::from(String::new());
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                out.text.push(' ');
                out.separators.push((out.text.len(), separator));
                out.text.push_str(separator.glyph());
                out.text.push(' ');
            }
            out.text.push_str(part);
        }
        out
    }

    pub fn ascii(&self, dot: &str) -> Cow<'_, str> {
        if self.separators.is_empty() {
            return Cow::Borrowed(&self.text);
        }
        let mut out = String::with_capacity(self.text.len());
        let mut start = 0;
        for &(at, separator) in &self.separators {
            out.push_str(&self.text[start..at]);
            out.push_str(match separator {
                NoticeSeparator::Dot => dot,
                NoticeSeparator::Arrow => "->",
            });
            start = at + separator.glyph().len();
        }
        out.push_str(&self.text[start..]);
        Cow::Owned(out)
    }

    pub(super) fn map_parts(&mut self, map: &mut impl FnMut(&mut String)) {
        let mut out = Self::from(String::new());
        let mut start = 0;
        for &(at, separator) in &self.separators {
            let mut part = self.text[start..at].to_owned();
            map(&mut part);
            out.text.push_str(&part);
            out.separators.push((out.text.len(), separator));
            out.text.push_str(separator.glyph());
            start = at + separator.glyph().len();
        }
        let mut part = self.text[start..].to_owned();
        map(&mut part);
        out.text.push_str(&part);
        out.detail_payload = self.detail_payload.take();
        *self = out;
    }
}

impl From<String> for NoticeText {
    fn from(text: String) -> Self {
        Self {
            text,
            separators: Vec::new(),
            detail_payload: None,
        }
    }
}

impl From<&str> for NoticeText {
    fn from(text: &str) -> Self {
        Self::from(text.to_owned())
    }
}

impl Deref for NoticeText {
    type Target = str;

    fn deref(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for NoticeText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}
