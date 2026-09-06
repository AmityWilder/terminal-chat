use std::borrow::Cow;

#[derive(thiserror::Error, Debug)]
pub enum UnescapeError {
    #[error("unsupported escape: \\{0}")]
    BadEsc(char),

    #[error("incomplete multi-character escape")]
    Invalid,

    #[error("string ends with unescaped '\\'")]
    TrailingEsc,
}

pub fn has_escape(s: &str) -> bool {
    s.contains('\\')
}

pub fn count_escapes(s: &str) -> usize {
    let mut num_escapes = 0;
    let mut is_esc = false;
    for ch in s.chars() {
        is_esc = !is_esc && ch == '\\';
        // no special case for hex escapes needed, we're just counting the number of unescaped '\'s.
        num_escapes += is_esc as usize;
    }
    num_escapes
}

fn unescape_char(start: usize, ch: char) -> Result<(usize, char), UnescapeError> {
    Ok((
        start + ch.len_utf8(),
        match ch {
            '0'..='9' => char::from(ch as u8 - b'0'),

            'a' => '\x07',
            'b' => '\x08',
            'f' => '\x0C',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'v' => '\x0B',
            '\\' => '\\',
            '"' => '\"',
            '\'' => '\'',

            _ => return Err(UnescapeError::BadEsc(ch)),
        },
    ))
}

#[derive(Debug, Clone)]
pub struct Escapes<'a> {
    s: &'a str,
    it: std::str::CharIndices<'a>,
}

impl<'a> Escapes<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            s,
            it: s.char_indices(),
        }
    }

    fn count_escapes(&self) -> usize {
        let mut num_escapes = 0;
        let mut is_esc = false;
        for (_, ch) in self.it.clone() {
            is_esc = !is_esc && ch == '\\';
            // no special case for hex escapes needed, we're just counting the number of unescaped '\'s.
            num_escapes += is_esc as usize;
        }
        num_escapes
    }

    fn extract_hex(&mut self) -> Result<(usize, char), UnescapeError> {
        if let Some((
            (k, '0'..='9' | 'A'..='F' | 'a'..='f'),
            (_, '0'..='9' | 'A'..='F' | 'a'..='f'),
        )) = self.it.next().zip(self.it.next())
        {
            let end = k + 2; // ascii chars are 1 byte each
            let value = char::from(
                u8::from_str_radix(&self.s[k..end], 16).expect("should be guarded by condition"),
            );
            Ok((end, value))
        } else {
            Err(UnescapeError::Invalid)
        }
    }

    fn extract_oct(&mut self) -> Result<(usize, char), UnescapeError> {
        if let Some((((k, '0'..='7'), (_, '0'..='7')), (_, '0'..='7'))) =
            self.it.next().zip(self.it.next()).zip(self.it.next())
        {
            let end = k + 3; // ascii chars are 1 byte each
            let value = char::from(
                u8::from_str_radix(&self.s[k..end], 8).expect("should be guarded by condition"),
            );
            Ok((end, value))
        } else {
            Err(UnescapeError::Invalid)
        }
    }
}

impl Iterator for Escapes<'_> {
    type Item = Result<(std::ops::Range<usize>, char), UnescapeError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.it.by_ref().find(|(_, ch)| *ch == '\\').map(|(i, _)| {
            self.it
                .next()
                .ok_or(UnescapeError::TrailingEsc)
                .and_then(|(j, ch)| {
                    match ch {
                        'x' => self.extract_hex(),
                        'o' => self.extract_oct(),
                        _ => unescape_char(j, ch),
                    }
                    .map(|(end, repl)| (i..end, repl))
                })
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.count_escapes();
        (n, Some(n))
    }
}

impl ExactSizeIterator for Escapes<'_> {}

impl std::iter::FusedIterator for Escapes<'_> {}

pub fn get_escapes(s: &str) -> Result<Vec<(std::ops::Range<usize>, char)>, UnescapeError> {
    Escapes::new(s).collect()
}

/// Convert all `\_` patterns to their other meanings.
pub fn unescape(string: &mut String) -> Result<(), UnescapeError> {
    // run in reverse so indices don't get invalidated
    for (range, repl) in get_escapes(string)?.into_iter().rev() {
        string.replace_range(range, repl.encode_utf8(&mut [0; char::MAX_LEN_UTF8]));
    }
    Ok(())
}

pub fn to_unescaped(s: &str) -> Result<Cow<'_, str>, UnescapeError> {
    if has_escape(s) {
        let mut string = s.to_string();
        unescape(&mut string)?;
        Ok(Cow::Owned(string))
    } else {
        Ok(Cow::Borrowed(s))
    }
}

#[cfg(test)]
mod tests {
    use super::{UnescapeError, unescape};

    #[test]
    fn test_singlechars() -> Result<(), UnescapeError> {
        let mut string = r#"i said \"hello\\"\n to him"#.to_string();
        unescape(&mut string)?;
        assert_eq!(string, "i said \"hello\\\"\n to him");
        Ok(())
    }

    #[test]
    fn test_noescape() -> Result<(), UnescapeError> {
        let mut string = "this text has no escapes".to_string();
        unescape(&mut string)?;
        assert_eq!(string, "this text has no escapes");
        Ok(())
    }

    #[test]
    fn test_hex() -> Result<(), UnescapeError> {
        let mut string = r#"the char '\x55' should be hex"#.to_string();
        unescape(&mut string)?;
        assert_eq!(string, "the char '\x55' should be hex");
        Ok(())
    }

    #[test]
    fn test_octal() -> Result<(), UnescapeError> {
        let mut string = r#"the char '\o153' should be hex"#.to_string();
        unescape(&mut string)?;
        assert_eq!(
            string,
            format!("the char '{}' should be hex", char::from(0o153u8))
        );
        Ok(())
    }
}
