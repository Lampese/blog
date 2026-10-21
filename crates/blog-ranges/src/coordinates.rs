//! Text ranges carry their coordinate unit until an external API needs numbers.
use crate::AnchorError;
use serde::{Deserialize, Serialize};
use std::{marker::PhantomData, ops::Range};

/// A range whose offsets count `U` units. Binding it to text validates bounds.
///
/// Different coordinate units cannot be passed interchangeably.
/// ```compile_fail
/// use blog_ranges::{Scalar, TextRange, Utf16};
/// let characters = TextRange::<Scalar>::new(0..2);
/// let browser: TextRange<Utf16> = characters;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRange<U> {
    offsets: Range<usize>,
    unit: PhantomData<U>,
}

impl<U> TextRange<U> {
    pub fn new(offsets: Range<usize>) -> Self {
        Self {
            offsets,
            unit: PhantomData,
        }
    }

    pub fn start(&self) -> usize {
        self.offsets.start
    }

    pub fn end(&self) -> usize {
        self.offsets.end
    }

    /// Leave the typed domain at a library or serialization boundary.
    pub fn offsets(&self) -> Range<usize> {
        self.offsets.clone()
    }

    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }
}

impl<U: Unit> TextRange<U> {
    fn bytes(&self, text: &str) -> Result<Range<usize>, AnchorError> {
        if self.start() > self.end() {
            return Err(AnchorError::InvalidRange);
        }
        Ok(U::byte_offset(text, self.start())?..U::byte_offset(text, self.end())?)
    }

    pub fn convert<V: Unit>(&self, text: &str) -> Result<TextRange<V>, AnchorError> {
        let bytes = self.bytes(text)?;
        Ok(TextRange::new(
            V::count(&text[..bytes.start])..V::count(&text[..bytes.end]),
        ))
    }

    pub fn slice<'a>(&self, text: &'a str) -> Result<&'a str, AnchorError> {
        Ok(&text[self.bytes(text)?])
    }
}

/// Wire ranges always keep the existing `{ start, end }` representation.
impl<U> Serialize for TextRange<U> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        WireRange {
            start: self.start(),
            end: self.end(),
        }
        .serialize(serializer)
    }
}

impl<'de, U> Deserialize<'de> for TextRange<U> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let range = WireRange::deserialize(deserializer)?;
        Ok(Self::new(range.start..range.end))
    }
}

#[derive(Serialize, Deserialize)]
struct WireRange {
    start: usize,
    end: usize,
}

mod sealed {
    pub trait Sealed {}
}

/// The three coordinate systems used by source, CRDT, and browser APIs.
pub trait Unit: sealed::Sealed {
    fn byte_offset(text: &str, offset: usize) -> Result<usize, AnchorError>;
    fn count(text: &str) -> usize;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Byte {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scalar {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Utf16 {}

impl sealed::Sealed for Byte {}
impl sealed::Sealed for Scalar {}
impl sealed::Sealed for Utf16 {}

impl Unit for Byte {
    fn byte_offset(text: &str, offset: usize) -> Result<usize, AnchorError> {
        text.is_char_boundary(offset)
            .then_some(offset)
            .ok_or(AnchorError::InvalidRange)
    }

    fn count(text: &str) -> usize {
        text.len()
    }
}

impl Unit for Scalar {
    fn byte_offset(text: &str, offset: usize) -> Result<usize, AnchorError> {
        text.char_indices()
            .map(|(byte, _)| byte)
            .chain(std::iter::once(text.len()))
            .nth(offset)
            .ok_or(AnchorError::InvalidRange)
    }

    fn count(text: &str) -> usize {
        text.chars().count()
    }
}

impl Unit for Utf16 {
    fn byte_offset(text: &str, offset: usize) -> Result<usize, AnchorError> {
        let mut units = 0;
        for (byte, ch) in text.char_indices() {
            if units == offset {
                return Ok(byte);
            }
            units += ch.len_utf16();
            if units > offset {
                return Err(AnchorError::InvalidUtf16Offset);
            }
        }
        if units == offset {
            Ok(text.len())
        } else {
            Err(AnchorError::InvalidUtf16Offset)
        }
    }

    fn count(text: &str) -> usize {
        text.encode_utf16().count()
    }
}
