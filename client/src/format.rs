use std::range::Range;

const FLAG_ESCAPED: u8 = 1;
const FLAG_ITALIC: u8 = 2;
const FLAG_BOLD: u8 = 4;
const FLAG_UNDERLINE: u8 = 8;
const FLAG_HIDDEN: u8 = 16;
const FLAG_STRIKETHROUGH: u8 = 32;
/// Single grave: all text is literal, without needing escapes
const FLAG_CODE: u8 = 64;
/// Double graves: all text is literal, without needing escapes, including single graves
const FLAG_DCODE: u8 = 128;
/// Triple graves: all text is literal, without needing escapes, including double graves
const FLAG_TCODE: u8 = FLAG_CODE | FLAG_DCODE;

const ALL_FLAGS: u8 = FLAG_ESCAPED
    | FLAG_ITALIC
    | FLAG_BOLD
    | FLAG_UNDERLINE
    | FLAG_HIDDEN
    | FLAG_STRIKETHROUGH
    | FLAG_CODE
    | FLAG_DCODE
    | FLAG_TCODE;

const ENABLE_ITALIC_ANSI: Replacement = Replacement::Str("\x1b[3m");
const ENABLE_BOLD_ANSI: Replacement = Replacement::Str("\x1b[1m");
const ENABLE_UNDERLINE_ANSI: Replacement = Replacement::Str("\x1b[4m");
const ENABLE_HIDDEN_ANSI: Replacement = Replacement::Str("\x1b[8m");
const ENABLE_STRIKETHROUGH_ANSI: Replacement = Replacement::Str("\x1b[9m");

const DISABLE_ITALIC_ANSI: Replacement = Replacement::Str("\x1b[23m");
const DISABLE_BOLD_ANSI: Replacement = Replacement::Str("\x1b[22m");
const DISABLE_UNDERLINE_ANSI: Replacement = Replacement::Str("\x1b[24m");
const DISABLE_HIDDEN_ANSI: Replacement = Replacement::Str("\x1b[28m");
const DISABLE_STRIKETHROUGH_ANSI: Replacement = Replacement::Str("\x1b[29m");

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    Unclosed(u8),
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unclosed(flags) => {
                write!(f, "style(s) started but not ended: ")?;
                let mut has_prev = false;
                for (flag, name) in [
                    (FLAG_ITALIC, "italic"),
                    (FLAG_BOLD, "bold"),
                    (FLAG_UNDERLINE, "underline"),
                    (FLAG_HIDDEN, "hidden"),
                    (FLAG_STRIKETHROUGH, "strikethrough"),
                    (FLAG_CODE, "code (`)"),
                    (FLAG_DCODE, "double code (``)"),
                    (FLAG_TCODE, "triple code (```)"),
                ] {
                    if has_prev {
                        write!(f, ", ")?;
                    }
                    has_prev = true;
                    if (flags & flag) == flag {
                        write!(f, "{name}")?;
                    }
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Replacement {
    Char(char),
    Str(&'static str),
}

impl Replacement {
    pub fn encode_utf8(self, dst: &mut [u8]) -> &str {
        match self {
            Self::Char(ch) => ch.encode_utf8(dst),
            Self::Str(s) => s,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FormatNodes<'a> {
    s: &'a str,
    iter: std::iter::Peekable<std::str::CharIndices<'a>>,
    flags: u8,
    stored: Option<<Self as Iterator>::Item>,
}

impl<'a> FormatNodes<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            s,
            iter: s.char_indices().peekable(),
            flags: 0,
            stored: None,
        }
    }

    #[inline]
    fn set_flag(&mut self, flag: u8) {
        self.flags |= flag;
    }

    #[inline]
    fn unset_flag(&mut self, flag: u8) {
        self.flags &= !flag;
    }

    #[inline]
    fn toggle_flag(&mut self, flag: u8) {
        self.flags ^= flag;
    }

    #[inline]
    fn assign_flag(&mut self, flag: u8, value: bool) {
        // branchless multiplexer
        self.flags &= !flag; // unset the flag for a clean slate
        self.flags |= flag * value as u8; // set the flag if it is true
    }

    #[inline]
    fn flag(&self, flag: u8) -> bool {
        (self.flags & flag) == flag
    }
}

impl<'a> Iterator for FormatNodes<'a> {
    type Item = Result<(Range<usize>, Replacement), FormatError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let stored @ Some(_) = self.stored.take() {
            return stored;
        }

        while let Some((start, ch)) = self.iter.next() {
            match ch {
                // escape - just skip the next character
                '\\' => {
                    _ = self
                        .iter
                        .next_if(|(_, ch)| matches!(ch, '`' | '*' | '_' | '|' | '~'))
                }

                // code
                '`' => {
                    return if self.iter.next_if(|(_, ch1)| *ch1 == '`').is_some() {
                        if self.iter.next_if(|(_, ch2)| *ch2 == '`').is_some() {
                            const DELIM_LEN: usize = "```".len();
                            // "```": tcode
                            //self.set_flag(FLAG_TCODE);
                            let item = Some(Ok((
                                Range::from(start..start + DELIM_LEN),
                                Replacement::Str(""),
                            )));
                            self.stored = Some(Err(FormatError::Unclosed(self.flags))); // fallback if none found
                            while let Some((close, ch)) = self.iter.next() {
                                if ch == '`'
                                    && self.iter.next_if(|(_, ch1)| *ch1 == '`').is_some()
                                    && self.iter.next_if(|(_, ch2)| *ch2 == '`').is_some()
                                {
                                    self.stored = Some(Ok((
                                        Range::from(close..close + DELIM_LEN),
                                        Replacement::Str(""),
                                    )));
                                    //self.unset_flag(FLAG_TCODE);
                                    break;
                                }
                            }
                            item
                        } else {
                            const DELIM_LEN: usize = "``".len();
                            // "``": dcode
                            //self.set_flag(FLAG_DCODE);
                            let item = Some(Ok((
                                Range::from(start..start + DELIM_LEN),
                                Replacement::Str(""),
                            )));
                            self.stored = Some(Err(FormatError::Unclosed(self.flags))); // fallback if none found
                            while let Some((close, ch)) = self.iter.next() {
                                if ch == '`' && self.iter.next_if(|(_, ch1)| *ch1 == '`').is_some()
                                {
                                    self.stored = Some(Ok((
                                        Range::from(close..close + DELIM_LEN),
                                        Replacement::Str(""),
                                    )));
                                    //self.unset_flag(FLAG_DCODE);
                                    break;
                                }
                            }
                            item
                        }
                    } else {
                        const DELIM_LEN: usize = "`".len();
                        // "`": code
                        //self.set_flag(FLAG_CODE);
                        let item = Some(Ok((
                            Range::from(start..start + DELIM_LEN),
                            Replacement::Str(""),
                        )));
                        self.stored = Some(Err(FormatError::Unclosed(self.flags))); // fallback if none found
                        for (close, ch) in &mut self.iter {
                            if ch == '`' {
                                self.stored = Some(Ok((
                                    Range::from(close..close + DELIM_LEN),
                                    Replacement::Str(""),
                                )));
                                //self.unset_flag(FLAG_CODE);
                                break;
                            }
                        }
                        item
                    };
                }

                // bold or italic
                '*' => {
                    if self.iter.next_if(|(_, ch)| *ch == '*').is_some() {
                        // "**": bold
                        self.toggle_flag(FLAG_BOLD);
                        return Some(Ok((
                            Range::from(start..start + 2), // "**" is 2 bytes (ASCII)
                            if self.flag(FLAG_BOLD) {
                                ENABLE_BOLD_ANSI
                            } else {
                                DISABLE_BOLD_ANSI
                            },
                        )));
                    } else {
                        // "*": italic
                        self.toggle_flag(FLAG_ITALIC);
                        return Some(Ok((
                            Range::from(start..start + 1), // "*" is 1 byte (ASCII)
                            if self.flag(FLAG_ITALIC) {
                                ENABLE_ITALIC_ANSI
                            } else {
                                DISABLE_ITALIC_ANSI
                            },
                        )));
                    }
                }

                // underline
                '_' if self.iter.next_if(|(_, ch)| *ch == '_').is_some() => {
                    self.toggle_flag(FLAG_UNDERLINE);
                    return Some(Ok((
                        Range::from(start..start + 2), // "__" is 2 bytes (ASCII)
                        if self.flag(FLAG_UNDERLINE) {
                            ENABLE_UNDERLINE_ANSI
                        } else {
                            DISABLE_UNDERLINE_ANSI
                        },
                    )));
                }

                // hidden
                '|' if self.iter.next_if(|(_, ch)| *ch == '|').is_some() => {
                    self.toggle_flag(FLAG_HIDDEN);
                    return Some(Ok((
                        Range::from(start..start + 2), // "||" is 2 bytes (ASCII)
                        if self.flag(FLAG_HIDDEN) {
                            ENABLE_HIDDEN_ANSI
                        } else {
                            DISABLE_HIDDEN_ANSI
                        },
                    )));
                }

                // strikethrough
                '~' if self.iter.next_if(|(_, ch)| *ch == '~').is_some() => {
                    self.toggle_flag(FLAG_STRIKETHROUGH);
                    return Some(Ok((
                        Range::from(start..start + 2), // "~~" is 2 bytes (ASCII)
                        if self.flag(FLAG_STRIKETHROUGH) {
                            ENABLE_STRIKETHROUGH_ANSI
                        } else {
                            DISABLE_STRIKETHROUGH_ANSI
                        },
                    )));
                }

                _ => (),
            }
        }
        // this is not very `std::iter::FusedIterator`
        ((self.flags & ALL_FLAGS) != 0).then_some(Err(FormatError::Unclosed(self.flags)))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

fn get_format_nodes(s: &str) -> Result<Vec<(Range<usize>, Replacement)>, FormatError> {
    FormatNodes::new(s).collect()
}

/// Convert all escape sequnces to their other meanings in-place.
pub fn format(string: &mut String) -> Result<(), FormatError> {
    let mut char_buf = [0; char::MAX_LEN_UTF8];
    // run in reverse so indices don't get invalidated
    for (range, repl) in get_format_nodes(string)?.into_iter().rev() {
        string.replace_range(range, repl.encode_utf8(&mut char_buf));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bold() {
        let mut s = "apple **orange** banana".to_string();
        format(&mut s).unwrap();
        assert_eq!(&s, "apple \x1b[1morange\x1b[22m banana");
    }

    #[test]
    fn test_italic() {
        let mut s = "apple *orange* banana".to_string();
        format(&mut s).unwrap();
        assert_eq!(&s, "apple \x1b[3morange\x1b[23m banana");
    }

    #[test]
    fn test_bold_italic() {
        let mut s = "apple ***orange*** banana".to_string();
        format(&mut s).unwrap();
        assert_eq!(&s, "apple \x1b[1m\x1b[3morange\x1b[22m\x1b[23m banana");
    }

    #[test]
    fn test_bold_italic_mixed() {
        let mut s = "apple *ora**nge*** banana".to_string();
        format(&mut s).unwrap();
        assert_eq!(&s, "apple \x1b[3mora\x1b[1mnge\x1b[22m\x1b[23m banana");
    }

    #[test]
    fn test_code() {
        let mut s = "apple `**orange**` banana".to_string();
        format(&mut s).unwrap();
        assert_eq!(&s, "apple **orange** banana");
    }

    #[test]
    fn test_dcode() {
        let mut s = "apple ``ee ` ee`` banana".to_string();
        format(&mut s).unwrap();
        assert_eq!(&s, "apple ee ` ee banana");
    }

    #[test]
    fn test_tcode() {
        let mut s = "apple ```ee `` ee``` banana".to_string();
        format(&mut s).unwrap();
        assert_eq!(&s, "apple ee `` ee banana");
    }

    #[test]
    fn test_multiescape() {
        let mut s = r#"hello **big dylan**! i remember your \x1b[91mred"#.to_string();
        format(&mut s).unwrap();
        assert_eq!(
            &s,
            "hello \x1b[1mbig dylan\x1b[22m! i remember your \\x1b[91mred"
        );
        crate::string::unescape(&mut s).unwrap();
        assert_eq!(
            &s,
            "hello \x1b[1mbig dylan\x1b[22m! i remember your \x1b[91mred"
        );
    }
}
