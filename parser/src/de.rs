use crate::outline2::Outline2;
use serde::{
    de::{self, Visitor},
    Deserialize, Serialize,
};
use std::error;
use std::fmt::{self, Write};
use std::iter::FromIterator;
use std::str::FromStr;

type Result<T> = std::result::Result<T, Error>;

// Outline slicing:
// &str of first headline, &child of first item, &[] of rest of otl
//
// None vs empty vs full string handling?
// Serialization doesn't care about none vs empty/

// Hm, tokens, first child, the rest

struct Deserializer<'de> {
    head: &'de str,
    body: &'de Outline2,
    is_inline_seq: bool,
}

impl<'de> From<&'de Outline2> for Deserializer<'de> {
    fn from(outline: &'de Outline2) -> Self {
        Deserializer {
            head: "",
            body: outline,
            is_inline_seq: false,
        }
    }
}

impl<'de> From<&'de (Option<String>, Outline2)> for Deserializer<'de> {
    fn from((head, body): &'de (Option<String>, Outline2)) -> Self {
        let head = head.as_ref().map_or("", |s| s.as_str());
        Deserializer {
            head,
            body,
            is_inline_seq: false,
        }
    }
}

pub fn from_outline<'de, T>(outline: &'de Outline2) -> Result<T>
where
    T: de::Deserialize<'de>,
{
    let mut deserializer = Deserializer::from(outline);

    let ret = T::deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(ret)
}

// TODO: Robust tokenizer, ditch the old stuff
// We can mutate the headline slice now, can simplify things.
impl<'de> Deserializer<'de> {
    fn parse_next_token(&self) -> Option<(&'de str, &'de str)> {
        // Eat initial whitespace.
        let token_start = self
            .head
            .chars()
            .take_while(|c| c.is_whitespace())
            .map(|c| c.len_utf8())
            .sum();

        let token_end = token_start
            + &self.head[token_start..]
                .chars()
                .take_while(|c| !c.is_whitespace())
                .map(|c| c.len_utf8())
                .sum();

        if token_end > token_start {
            Some((&self.head[token_start..token_end], &self.head[token_end..]))
        } else {
            None
        }
    }

    /// Get next whitespace-separated token and advance deserializer.
    fn next_token(&'_ mut self) -> Option<&'_ str> {
        if let Some((token, rest)) = self.parse_next_token() {
            self.head = rest;
            Some(token)
        } else if self.body.len() == 1 && !self.is_inline_seq {
            // There was no token on headline, but the rest of the outline
            // looks like it's just one line. (And we're not parsing an inline
            // sequence so we haven't just run out of items to parse)
            //
            // Do a switcheroo and make the single body line into the new
            // headline.
            if let Some(ref s) = self.body[0].0 {
                self.head = s.as_str();
            } else {
                self.head = "";
            }
            self.body = &self.body[0].1;
            self.next_token()
        } else {
            None
        }
    }

    /// Parse next token into given type if possible.
    fn parse_next<T: FromStr>(&mut self) -> Result<T> {
        if let Some(tok) = self.next_token() {
            if let Ok(val) = tok.parse() {
                return Ok(val);
            }
        }
        Err(Error::default())
    }

    fn set_fully_consumed(&mut self) {
        self.head = "";

        // Hacky way to get us an empty outline that's in the self.body memory
        // block.
        while !self.body.is_empty() {
            self.body = &self.body[0].1;
        }
    }

    fn headline_is_empty(&self) -> bool {
        !self.head.chars().any(|c| !c.is_whitespace())
    }

    fn parse_string(&mut self) -> Result<String> {
        if !self.headline_is_empty() {
            if self.is_inline_seq {
                // If currently in sequence, strings are whitespace-separated
                self.parse_next()
            } else {
                // Otherwise read to the end of headline
                let ret = self.head.to_string();
                self.head = "";
                Ok(ret)
            }
        } else if !self.body.is_empty() {
            // Headline is empty, read body as paragraph
            let body = Outline2::from_iter(self.body.iter().cloned());
            let mut ret = format!("{}", body);
            // Remove the trailing newline so inline and outline single-line
            // string stay equal.
            if ret.ends_with('\n') {
                ret.pop();
            }
            self.set_fully_consumed();
            Ok(ret)
        } else {
            // XXX: Should we return an empty string here?
            Err(Error::default())
        }
    }

    /// Check that all data has been consumed.
    fn end(&self) -> Result<()> {
        if !self.body.is_empty() || !self.head.is_empty() {
            return Err(Error::default());
        }
        Ok(())
    }

    fn is_line(&self) -> bool {
        !self.headline_is_empty() && self.body.is_empty()
    }

    fn is_paragraph(&self) -> bool {
        self.headline_is_empty() && !self.body.is_empty()
    }

    fn is_section(&self) -> bool {
        !self.headline_is_empty() && !self.body.is_empty()
    }
}

impl<'a, 'de> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    // This is limited since the data format is not self-describing.
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_str(visitor)
    }

    // Primitive types just use the default FromStr behavior

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if !self.is_inline_seq && self.end().is_ok() {
            // XXX: Okay so this is way hacky, but
            // We're treating booleans as flags, mostly in struct context.
            // Assume that default struct has bools set to false,
            // so if you write a struct and just have
            //   Struct
            //     flag1:
            // That can be a concise way of saying "flag 1 is set".
            // So we parse empty input as "true" when expecting a bool.
            //
            // This may be a bad idea.
            visitor.visit_bool(true)
        } else {
            visitor.visit_bool(self.parse_next()?)
        }
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_i8(self.parse_next()?)
    }

    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_i16(self.parse_next()?)
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_i32(self.parse_next()?)
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_i64(self.parse_next()?)
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u8(self.parse_next()?)
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u16(self.parse_next()?)
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u32(self.parse_next()?)
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u64(self.parse_next()?)
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_f32(self.parse_next()?)
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_f64(self.parse_next()?)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if let Some(token) = self.next_token() {
            if token.chars().count() == 1 {
                return visitor.visit_char(token.chars().next().unwrap());
            }
        }
        return Err(Error::default());
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(&self.parse_string()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // XXX: No way currently to express an explicit None in data.
        // Options are expected to be used in structs and by omitting the whole struct field from
        // the literal.
        //
        // Maybe a dedicated 'nil' literal could be introduced if we really need this?
        visitor.visit_some(self)
    }

    fn deserialize_unit<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!();
    }

    // Unit struct means a named value containing no data.
    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!();
    }

    // As is done here, serializers are encouraged to treat newtype structs as
    // insignificant wrappers around the data they contain. That means not
    // parsing anything other than the contained value.
    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    // Deserialization of compound types like sequences and maps happens by
    // passing the visitor an "Access" object that gives it the ability to
    // iterate through the data contained in the sequence.
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.is_inline_seq {
            // Double nesting detected
            return Err(Error::default());
        }

        let seq = if !self.headline_is_empty() {
            self.is_inline_seq = true;
            Sequence {
                de: self,
                cursor: Cursor::Inline,
            }
        } else {
            Sequence {
                de: self,
                cursor: Cursor::Child(0, 0),
            }
        };

        let ret = visitor.visit_seq(seq);
        if !self.is_inline_seq {
            // Ate all the children
            self.set_fully_consumed();
        }

        self.is_inline_seq = false;
        ret
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    // Much like `deserialize_seq` but calls the visitors `visit_map` method
    // with a `MapAccess` implementation, rather than the visitor's `visit_seq`
    // method with a `SeqAccess` implementation.
    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // XXX: Repetition shared with deserialize_seq, factor out?
        if self.is_inline_seq {
            // Double nesting detected
            return Err(Error::default());
        }

        let seq = if !self.headline_is_empty() {
            self.is_inline_seq = true;
            Sequence {
                de: self,
                cursor: Cursor::Inline,
            }
        } else {
            Sequence {
                de: self,
                cursor: Cursor::Child(0, 0),
            }
        };

        let ret = visitor.visit_map(seq);
        if !self.is_inline_seq {
            // Ate all the children
            self.set_fully_consumed();
        }

        self.is_inline_seq = false;
        ret
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // TODO: Expect colons after attribute names.
        // TODO: Parse hyphens to underscores in attribute names.
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // TODO: Enum parsing
        unimplemented!()
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}

/// Cursor for progressing through a map or a seq.
enum Cursor {
    /// Cursor for inline data, use deserializer's position
    Inline,
    /// Cursor for vertical data, parameters are nth child, kth offset in headline
    Child(usize, usize),
}

/// Sequence accessor for items in a single line.
///
/// Uses whitespace as separator, string values in an inline list cannot have whitespace.
struct Sequence<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    cursor: Cursor,
}

impl<'a, 'de> de::SeqAccess<'de> for Sequence<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.cursor {
            Cursor::Inline => {
                if self.de.headline_is_empty() {
                    Ok(None)
                } else {
                    seed.deserialize(&mut *self.de).map(Some)
                }
            }
            Cursor::Child(n, offset) => {
                if n >= self.de.body.len() {
                    Ok(None)
                } else {
                    let mut child_de = Deserializer::from(&self.de.body[n]);
                    self.cursor = Cursor::Child(n + 1, 0);
                    seed.deserialize(&mut child_de).map(Some)
                }
            }
        }
    }
}

impl<'a, 'de> de::MapAccess<'de> for Sequence<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: de::DeserializeSeed<'de>,
    {
        match self.cursor {
            Cursor::Inline => {
                if self.de.headline_is_empty() {
                    Ok(None)
                } else {
                    seed.deserialize(&mut *self.de).map(Some)
                }
            }
            Cursor::Child(n, offset) => {
                if n >= self.de.body.len() {
                    Ok(None)
                } else {
                    let mut child_de = Deserializer::from(&self.de.body[n]);
                    child_de.is_inline_seq = true;
                    child_de.head = &child_de.head[offset..];

                    let ret = seed.deserialize(&mut child_de).map(Some);
                    // Save parse offset from key
                    // XXX: keys must always be inline values
                    self.cursor = Cursor::Child(n, 0);
                    ret
                }
            }
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: de::DeserializeSeed<'de>,
    {
        //self.cursor = Cursor::Child(n + 1);
        match self.cursor {
            Cursor::Inline => seed.deserialize(&mut *self.de),
            Cursor::Child(n, offset) => {
                // TODO: Figure this out!!!
                // Need to apply parse after key is taken
                let mut child_de = Deserializer::from(&self.de.body[n]);
                child_de.head = &child_de.head[offset..];

                self.cursor = Cursor::Child(n + 1, 0);
                let ret = seed.deserialize(&mut child_de);
                child_de.end()?;
                ret
            }
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Error(String);

impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Error {
        Error(format!("{}", msg))
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
