//! Ame (Amy) Object Notation

#![feature(io_const_error)]
#![warn(clippy::undocumented_unsafe_blocks)]

mod de;
mod error;
mod ser;

pub use de::{Deserializer, from_bytes};
pub use error::{Error, Result};
pub use ser::{Serializer, to_bytes};

#[cfg(test)]
mod tests {
    use crate::*;
    use serde::{Deserialize, Serialize};

    /// should be able to serialize and deserialize a byte string exactly as it was given
    #[test]
    fn test_symmetric() -> Result<()> {
        const BYTES: &[u8] = b"y53t237r8wbf71u08t4h  7qh8mwe9c9fh238 cync9rfcjmh39   72rm1m  3 c\"::\"%^:L$WT\":ETEW\'47qG^%&#$*@!&(#%!\\::<>>>MSDF<AS?,./";
        let data = to_bytes(BYTES)?;
        let echo: Vec<u8> = from_bytes(data.as_slice())?;
        assert_eq!(&echo, BYTES);
        Ok(())
    }

    #[test]
    fn test_struct() -> Result<()> {
        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct Foo {
            apple: bool,
            orange: usize,
        }
        let foo = Foo {
            apple: false,
            orange: 5473,
        };
        assert_eq!(from_bytes::<Foo>(&to_bytes(&foo)?)?, foo);
        Ok(())
    }

    #[test]
    fn test_enum() -> Result<()> {
        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        enum Foo {
            Apple,
            Orange(u32),
            Banana { peel: bool, color: String },
        }
        let foo = Foo::Apple;
        let bar = Foo::Orange(65);
        let baz = Foo::Banana {
            peel: true,
            color: "yellow".to_string(),
        };
        assert_eq!(from_bytes::<Foo>(&to_bytes(&foo)?)?, foo);
        assert_eq!(from_bytes::<Foo>(&to_bytes(&bar)?)?, bar);
        assert_eq!(from_bytes::<Foo>(&to_bytes(&baz)?)?, baz);
        Ok(())
    }
}
