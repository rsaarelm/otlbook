use crate::outline::Outline;
use serde::{
    de::{self, Visitor},
    Deserialize, Serialize,
};
use std::error;
use std::fmt::{self, Write};
use std::str::FromStr;

type Result<T> = std::result::Result<T, Error>;

pub struct Deserializer<'de> {
    outline: &'de Outline,
    offset: usize,
    is_inline_seq: bool,
}

pub fn from_outline<'de, T: de::Deserialize<'de>>(outline: &'de Outline) -> Result<T> {
    let mut deserializer = Deserializer {
        outline,
        offset: 0,
        is_inline_seq: false,
    };

    let ret = T::deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(ret)
}

impl<'de> Deserializer<'de> {
    fn next_token_end(&self) -> Option<usize> {
        if let Some(headline) = &self.outline.headline {
            let s = &headline[self.offset..];
            if s.is_empty() {
                return None;
            }
            // Make sure you consume at least one character if there's trailing space.
            // XXX: Might want to eat all trailing space and whatever non-space comes after that.
            let first_char_width = s.chars().next().unwrap().len_utf8();
            let offset = s[first_char_width..]
                .find(' ')
                .map(|x| x + first_char_width)
                .unwrap_or(s.len());
            Some(self.offset + offset)
        } else {
            None
        }
    }

    /// Return whether this line is a sequence-separating comma and normalize it in case it's an
    /// escaped literal comma.
    ///
    /// ",," becomes a literal ",", ",,," a literal ",," and so on.
    fn normalize_comma(&mut self) -> bool {
        if self.outline.headline == Some(",".to_string()) {
            self.offset = 1;
            return true;
        }
        // Check for escaped comma
        if let Some(headline) = &self.outline.headline {
            if headline.chars().all(|c| c == ',') && self.offset == 0 {
                self.offset = 1;
            }
        }
        false
    }

    fn next_char(&mut self) -> Result<char> {
        if let Some(headline) = &self.outline.headline {
            if let Some(c) = &headline[self.offset..].chars().next() {
                self.offset += c.len_utf8();
                return Ok(*c);
            }
        }
        Err(Error::default())
    }

    fn headline_len(&self) -> Option<usize> {
        self.outline.headline.as_ref().map(|s| s.len())
    }

    fn peek_token(&self) -> Option<&str> {
        if let (Some(headline), Some(token_end)) = (&self.outline.headline, self.next_token_end()) {
            Some(&headline[self.offset..token_end])
        } else {
            None
        }
    }

    fn next_token(&mut self) -> Option<&str> {
        if let (Some(headline), Some(token_end)) = (&self.outline.headline, self.next_token_end()) {
            let ret = Some(&headline[self.offset..token_end]);
            self.offset = token_end;
            // Skip the one space
            let _ = self.next_char();
            ret
        } else {
            None
        }
    }

    fn parse_next<T: FromStr>(&mut self) -> Result<T> {
        if let Some(tok) = self.peek_token() {
            if let Ok(val) = tok.parse() {
                self.next_token();
                return Ok(val);
            }
        }
        Err(Error::default())
    }

    fn headline_tail(&self) -> Option<&str> {
        if let Some(headline) = &self.outline.headline {
            if self.offset < headline.len() {
                return Some(&headline[self.offset..]);
            }
        }
        None
    }

    fn set_fully_consumed(&mut self) {
        while !self.outline.children.is_empty() {
            let last_idx = self.outline.children.len() - 1;
            self.outline = &self.outline.children[last_idx];
        }
        self.offset = self.headline_len().unwrap_or(0);
    }

    fn parse_string(&mut self) -> Result<String> {
        if let Some(tail) = self.headline_tail() {
            if self.is_inline_seq {
                // If currently in sequence, strings are whitespace-separated
                self.parse_next()
            } else {
                // Otherwise string is to the end of input
                let new_offset = self.headline_len().unwrap_or(0);
                let ret: String = tail.into();
                self.offset = new_offset;
                Ok(ret)
            }
        } else if !self.outline.children.is_empty() {
            // No more headline left, read children as the string literal.
            let mut ret = String::new();
            for c in &self.outline.children {
                let _ = write!(&mut ret, "{}", c);
            }
            self.set_fully_consumed();
            Ok(ret)
        } else {
            Err(Error::default())
        }
    }

    /// Check that all data has been consumed.
    fn end(&self) -> Result<()> {
        if !self.outline.children.is_empty() {
            return Err(Error::default());
        }
        if self.headline_tail().is_some() {
            return Err(Error::default());
        }
        Ok(())
    }
}

impl<'a, 'de> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    // This is limited since the data format is not self-describing. End of data is parsed as unit
    // (this allows the pattern of using Option<()> for flag values), otherwise data is treated as
    // a string.
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if self.end().is_ok() {
            visitor.visit_unit()
        } else {
            self.deserialize_str(visitor)
        }
    }

    // Primitive types just use the default FromStr behavior

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_bool(self.parse_next()?)
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
        // XXX: Should this just be unimplemented and you should use string parse insted?
        if let Some(token) = self.peek_token() {
            if token.chars().count() == 1 {
                let token = self.next_token().unwrap();
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

    // The `Serializer` implementation on the previous page serialized byte
    // arrays as JSON arrays of bytes. Handle that representation here.
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

    // Parsing an unit value always succeeds and doesn't advance parser state.
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    // Unit struct means a named value containing no data.
    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    // As is done here, serializers are encouraged to treat newtype structs as
    // insignificant wrappers around the data they contain. That means not
    // parsing anything other than the contained value.
    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
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

        let seq = if self.headline_tail().is_some() {
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

        let seq = if self.headline_tail().is_some() {
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
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
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
                if self.de.headline_tail().is_none() {
                    Ok(None)
                } else {
                    seed.deserialize(&mut *self.de).map(Some)
                }
            }
            Cursor::Child(n, offset) => {
                if n >= self.de.outline.children.len() {
                    Ok(None)
                } else {
                    let mut child_de = Deserializer {
                        outline: &self.de.outline.children[n],
                        offset: offset,
                        is_inline_seq: false,
                    };
                    child_de.normalize_comma();
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
                if self.de.headline_tail().is_none() {
                    Ok(None)
                } else {
                    seed.deserialize(&mut *self.de).map(Some)
                }
            }
            Cursor::Child(n, offset) => {
                if n >= self.de.outline.children.len() {
                    Ok(None)
                } else {
                    let mut child_de = Deserializer {
                        outline: &self.de.outline.children[n],
                        offset: offset,
                        is_inline_seq: true,
                    };
                    child_de.normalize_comma();
                    let ret = seed.deserialize(&mut child_de).map(Some);
                    // Save parse offset from key
                    // XXX: keys must always be inline values
                    self.cursor = Cursor::Child(n, child_de.offset);
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
                let mut child_de = Deserializer {
                    outline: &self.de.outline.children[n],
                    offset: offset,
                    is_inline_seq: false,
                };
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde::de;
    use std::fmt;

    fn test<T: de::DeserializeOwned + fmt::Debug + PartialEq>(outline: &str, value: T) {
        let mut outline = Outline::from(outline);

        // String to outline always produces empty headline and content in children.
        // Extract the first child as the unit to deserialize the type from.
        if !outline.children.is_empty() {
            outline = outline.children[0].clone();
        }

        let outline_value: T = from_outline(&outline).expect("Outline did not parse into value");

        assert_eq!(value, outline_value);
    }

    fn not_parsed<T: de::DeserializeOwned + fmt::Debug + PartialEq>(outline: &str) {
        let mut outline = Outline::from(outline);

        if !outline.children.is_empty() {
            outline = outline.children[0].clone();
        }

        assert!((from_outline(&outline) as Result<T>).is_err());
    }

    #[test]
    fn test_tokenizer() {
        let outline = Outline::from("foo bar baz");
        let outline = outline.children[0].clone();
        let mut de = Deserializer {
            outline: &outline,
            offset: 0,
            is_inline_seq: false,
        };

        assert_eq!(de.peek_token(), Some("foo"));
        assert_eq!(de.next_token(), Some("foo"));
        assert_eq!(de.next_token(), Some("bar"));
        assert_eq!(de.next_token(), Some("baz"));
    }

    #[test]
    fn test_simple() {
        test("", ());
        test("123", 123u32);
        test("2.71828", 2.71828f32);
        test("true", true);
        test("false", false);
        test("symbol", "symbol".to_string());
        test("two words", "two words".to_string());

        test("a", 'a');
        not_parsed::<char>("aa");
        test("殺", '殺');
        not_parsed::<char>("殺殺殺殺殺殺殺");

        not_parsed::<u32>("123 junk");
    }

    #[test]
    fn test_tuple() {
        test("123", (123u32,));
        test("123 zomg", (123u32, "zomg".to_string()));
    }

    #[test]
    fn test_struct() {
        #[derive(Debug, PartialEq, Deserialize)]
        struct Simple {
            num: i32,
            title: String,
            tags: Vec<String>,
        }

        test(
            "\
\tnum 32
\ttitle foo bar
\ttags foo bar",
            Simple {
                num: 32,
                title: "foo bar".into(),
                tags: vec!["foo".into(), "bar".into()],
            },
        );

        not_parsed::<Simple>(
            "\
\tnum 32 garbage
\ttitle foo bar
\ttags foo bar",
        );

        test(
            "\
\tnum 32
\ttitle
\t\tmany
\t\tlines
\ttags
\t\tfoo
\t\tbar",
            Simple {
                num: 32,
                title: "many\nlines\n".into(),
                tags: vec!["foo".into(), "bar".into()],
            },
        );

        not_parsed::<Simple>(
            "\
\tnom 32
\ttitle foo bar
\ttags foo bar",
        );
    }

    #[test]
    fn test_inline_struct() {
        #[derive(Default, Debug, PartialEq, Deserialize)]
        struct Vec {
            x: i32,
            y: i32,
        }

        test("x -5 y 10", Vec { x: -5, y: 10 });
    }

    #[test]
    fn test_nested_struct() {
        #[derive(Default, Debug, PartialEq, Deserialize)]
        struct Nesting {
            x: i32,
            y: i32,
            tail: Option<Box<Nesting>>,
        }

        test(
            "\
\tx 1
\ty 2
\ttail
\t\tx 3
\t\ty 4",
            Nesting {
                x: 1,
                y: 2,
                tail: Some(Box::new(Nesting {
                    x: 3,
                    y: 4,
                    tail: None,
                })),
            },
        );

        test(
            "\
\tx 1
\ty 2
\ttail x 3 y 4",
            Nesting {
                x: 1,
                y: 2,
                tail: Some(Box::new(Nesting {
                    x: 3,
                    y: 4,
                    tail: None,
                })),
            },
        );
    }

    #[test]
    fn test_inline_list() {
        test("1 2 3", vec![1u32, 2u32, 3u32]);

        test(
            "foo bar baz",
            vec!["foo".to_string(), "bar".to_string(), "baz".to_string()],
        );
    }

    #[test]
    fn test_nested_inline_list() {
        // They shouldn't be parseable.
        not_parsed::<Vec<Vec<u32>>>("1 2 3");
        not_parsed::<Vec<Vec<String>>>("foo bar baz");
    }

    #[test]
    fn test_simple_vertical_list() {
        test(
            "\
\t1
\t2
\t3",
            vec![1u32, 2u32, 3u32],
        );
    }

    #[test]
    fn test_string_block() {
        test(
            "\
\tEs brillig war. Die schlichte Toven
\tWirrten und wimmelten in Waben;",
            "Es brillig war. Die schlichte Toven\nWirrten und wimmelten in Waben;\n".to_string(),
        );
    }

    #[test]
    fn test_string_list() {
        // Vertical list
        test(
            "\
\tfoo bar
\tbaz",
            vec!["foo bar".to_string(), "baz".to_string()],
        );

        // XXX: Should this be made to be an error?
        //        // Must have comma before the nested item.
        //        not_parsed::<Vec<String>>(
        //            "\
        //\tfoo
        //\t\tbar
        //\t\tbaz",
        //        );
    }

    /* FIXME: Get this working
        #[test]
        fn test_nested_string_list() {
            test(
                r#"\
    \tfoo bar
    \tbaz xyzzy"#,
                vec![
                    vec!["foo".to_string(), "bar".to_string()],
                    vec!["baz".to_string(), "xyzzy".to_string()],
                ],
            );
        }
    */

    #[test]
    fn test_comma() {
        // A single inline comma is a magic extra element to separate indented objects. It can be
        // escaped by doubling it (any sequence of more than 1 comma gets one comma removed from
        // it). A comma is just a comma in an multi-line string block, no escaping needed there.
        test(
            "\
\t\tEs brillig war. Die schlichte Toven
\t\tWirrten und wimmelten in Waben;
\t,
\t\tUnd aller-mümsige Burggoven
\t\tDie mohmen Räth' ausgraben.",
            vec![
                "Es brillig war. Die schlichte Toven\nWirrten und wimmelten in Waben;\n"
                    .to_string(),
                "Und aller-mümsige Burggoven\nDie mohmen Räth' ausgraben.\n".to_string(),
            ],
        );

        // An optional starting comma is allowed
        test(
            "\
\t,
\t\tEs brillig war. Die schlichte Toven
\t\tWirrten und wimmelten in Waben;
\t,
\t\tUnd aller-mümsige Burggoven
\t\tDie mohmen Räth' ausgraben.",
            vec![
                "Es brillig war. Die schlichte Toven\nWirrten und wimmelten in Waben;\n"
                    .to_string(),
                "Und aller-mümsige Burggoven\nDie mohmen Räth' ausgraben.\n".to_string(),
            ],
        );

        // Need to also separate an indented block with comma
        test(
            "\
\tfoo
\t,
\t\tbar
\t\tbaz",
            vec!["foo".to_string(), "bar\nbaz\n".to_string()],
        );
    }

    #[test]
    fn test_escaped_comma() {
        // Double comma in vertical list becomes single comma
        test(
            "\
\tfoo
\t,,
\t,,,
\tbar",
            vec![
                "foo".to_string(),
                ",".to_string(),
                ",,".to_string(),
                "bar".to_string(),
            ],
        );

        // Text block can have comma lines.
        test(
            "\
\t\t,",
            vec![",\n".to_string()],
        );
    }

    #[derive(Default, Debug, PartialEq, Deserialize)]
    struct Options {
        foo: Option<()>,
        bar: Option<()>,
        baz: Option<()>,
    }

    #[test]
    fn test_option_struct() {
        test("", Options::default());

        test(
            "\
\tfoo",
            Options {
                foo: Some(()),
                ..Options::default()
            },
        );
        test(
            "\
\tbar
\tbaz",
            Options {
                bar: Some(()),
                baz: Some(()),
                ..Options::default()
            },
        );

        test("", vec![] as Vec<Options>);
    }

    #[test]
    fn test_option_struct_list() {
        test(
            "\
\t\tbar
\t\tbaz
\t,
\t\tfoo",
            vec![
                Options {
                    bar: Some(()),
                    baz: Some(()),
                    ..Options::default()
                },
                Options {
                    foo: Some(()),
                    ..Options::default()
                },
            ],
        );

        // Use comma to mark options with all flags off
        test(
            "\
\t\tbar
\t\tbaz
\t,
\t\tfoo
\t,",
            vec![
                Options {
                    bar: Some(()),
                    baz: Some(()),
                    ..Options::default()
                },
                Options {
                    foo: Some(()),
                    ..Options::default()
                },
                Options::default(),
            ],
        );

        test("\t,", vec![Options::default()]);

        test(
            "\
\t,
\t,",
            vec![Options::default(), Options::default()],
        );
    }
}
