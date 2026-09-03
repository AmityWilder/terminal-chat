use std::io::{self, Read, Write};

pub trait WriteTo: Sized {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()>;
}

impl<T: WriteTo> WriteTo for &T {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        (*self).write_to(stream)
    }
}

impl WriteTo for u8 {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        stream.write_all(std::slice::from_ref(self))
    }
}
impl WriteTo for bool {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        (*self as u8).write_to(stream)
    }
}
impl WriteTo for u16 {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        stream.write_all(self.to_le_bytes().as_slice())
    }
}
impl WriteTo for u32 {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        stream.write_all(self.to_le_bytes().as_slice())
    }
}
impl WriteTo for u64 {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        stream.write_all(self.to_le_bytes().as_slice())
    }
}
impl WriteTo for u128 {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        stream.write_all(self.to_le_bytes().as_slice())
    }
}

impl WriteTo for i8 {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        stream.write_all(std::slice::from_ref(&self.cast_unsigned()))
    }
}
impl WriteTo for i16 {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        stream.write_all(self.to_le_bytes().as_slice())
    }
}
impl WriteTo for i32 {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        stream.write_all(self.to_le_bytes().as_slice())
    }
}
impl WriteTo for i64 {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        stream.write_all(self.to_le_bytes().as_slice())
    }
}
impl WriteTo for i128 {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        stream.write_all(self.to_le_bytes().as_slice())
    }
}

impl WriteTo for std::time::Duration {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        // WHY IS THIS A DIFFERENT TYPE?!
        (self.as_millis() as u64).write_to(stream)
    }
}
impl WriteTo for std::time::SystemTime {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        self.duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .write_to(stream)
    }
}

impl WriteTo for std::net::SocketAddr {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        self.to_string().write_to::<_, 1>(stream)
    }
}

impl<T: WriteTo> WriteTo for Option<T> {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        self.is_some().write_to(stream)?;
        if let Some(value) = self {
            value.write_to(stream)?;
        }
        Ok(())
    }
}

/// Variable-size [`WriteTo`] - writes the buffer size as an integer encoded in `N` bytes, then the buffer content
pub trait VarWriteTo {
    fn write_to<W: ?Sized + Write, const N: usize>(&self, stream: &mut W) -> io::Result<()>;
}

impl VarWriteTo for usize {
    fn write_to<W: ?Sized + Write, const N: usize>(&self, stream: &mut W) -> io::Result<()> {
        const {
            assert!(N <= u32::MAX as usize);
        }
        let value = *self;
        if const { N < 8 } && (value >> const { N as u32 * 8 }) != 0 {
            return Err(io::Error::other(format!(
                "cannot store buffer size ({value}) in {N} bytes"
            )));
        }
        stream.write_all(&value.to_le_bytes()[..N])
    }
}

impl VarWriteTo for [u8] {
    fn write_to<W: ?Sized + Write, const N: usize>(&self, stream: &mut W) -> io::Result<()> {
        self.len().write_to::<_, N>(stream)?;
        stream.write_all(self)
    }
}

impl VarWriteTo for str {
    fn write_to<W: ?Sized + Write, const N: usize>(&self, stream: &mut W) -> io::Result<()> {
        self.as_bytes().write_to::<_, N>(stream)
    }
}

pub trait WriteListTo {
    fn write_list<W: ?Sized + Write, const N: usize>(self, stream: &mut W) -> io::Result<()>;
}

impl<I> WriteListTo for I
where
    I: IntoIterator<Item: WriteTo, IntoIter: ExactSizeIterator>,
{
    fn write_list<W: ?Sized + Write, const N: usize>(self, stream: &mut W) -> io::Result<()> {
        let it = self.into_iter();
        it.len().write_to::<_, N>(stream)?;
        for item in it {
            item.write_to(stream)?;
        }
        Ok(())
    }
}

pub trait ReadFrom: Sized {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self>;
}

impl ReadFrom for u8 {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        let mut byte = 0;
        stream
            .read_exact(std::slice::from_mut(&mut byte))
            .map(|()| byte)
    }
}
impl ReadFrom for bool {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        Ok(u8::read_from(stream)? != 0)
    }
}
impl ReadFrom for u16 {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        let mut buf = [0; _];
        stream
            .read_exact(&mut buf)
            .map(|()| Self::from_le_bytes(buf))
    }
}
impl ReadFrom for u32 {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        let mut buf = [0; _];
        stream
            .read_exact(&mut buf)
            .map(|()| Self::from_le_bytes(buf))
    }
}
impl ReadFrom for u64 {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        let mut buf = [0; _];
        stream
            .read_exact(&mut buf)
            .map(|()| Self::from_le_bytes(buf))
    }
}
impl ReadFrom for u128 {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        let mut buf = [0; _];
        stream
            .read_exact(&mut buf)
            .map(|()| Self::from_le_bytes(buf))
    }
}

impl ReadFrom for i8 {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        let mut byte = 0;
        stream
            .read_exact(std::slice::from_mut(&mut byte))
            .map(|()| byte.cast_signed())
    }
}
impl ReadFrom for i16 {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        let mut buf = [0; _];
        stream
            .read_exact(&mut buf)
            .map(|()| Self::from_le_bytes(buf))
    }
}
impl ReadFrom for i32 {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        let mut buf = [0; _];
        stream
            .read_exact(&mut buf)
            .map(|()| Self::from_le_bytes(buf))
    }
}
impl ReadFrom for i64 {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        let mut buf = [0; _];
        stream
            .read_exact(&mut buf)
            .map(|()| Self::from_le_bytes(buf))
    }
}
impl ReadFrom for i128 {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        let mut buf = [0; _];
        stream
            .read_exact(&mut buf)
            .map(|()| Self::from_le_bytes(buf))
    }
}

impl ReadFrom for std::time::Duration {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        u64::read_from(stream).map(Self::from_millis)
    }
}
impl ReadFrom for std::time::SystemTime {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        std::time::Duration::read_from(stream).and_then(|duration| {
            std::time::UNIX_EPOCH
                .checked_add(duration)
                .ok_or(io::Error::other("invalid time"))
        })
    }
}

impl ReadFrom for std::net::SocketAddr {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        String::read_from::<_, 1>(stream)?
            .parse()
            .map_err(io::Error::other)
    }
}

impl<T: ReadFrom> ReadFrom for Option<T> {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        (u8::read_from(stream)? != 0)
            .then(|| T::read_from(stream))
            .transpose()
    }
}

/// Variable-size [`ReadFrom`] - reads the buffer size as an integer encoded in `N` bytes, then the buffer content
pub trait VarReadFrom: Sized {
    fn read_from<R: ?Sized + Read, const N: usize>(stream: &mut R) -> io::Result<Self>;
}

impl VarReadFrom for usize {
    fn read_from<R: ?Sized + Read, const N: usize>(stream: &mut R) -> io::Result<Self> {
        const {
            assert!(
                std::mem::size_of::<usize>() >= N,
                "cannot extract N bytes into usize"
            );
        }
        let mut len_bytes = [0; _];
        stream.read_exact(&mut len_bytes[..N])?;
        let n = usize::from_le_bytes(len_bytes);
        Ok(n)
    }
}

impl VarReadFrom for Vec<u8> {
    fn read_from<R: ?Sized + Read, const N: usize>(stream: &mut R) -> io::Result<Self> {
        let len = usize::read_from::<_, N>(stream)?;
        let mut buf = vec![0; len];
        stream.read_exact(buf.as_mut_slice())?;
        Ok(buf)
    }
}

impl VarReadFrom for String {
    fn read_from<R: ?Sized + Read, const N: usize>(stream: &mut R) -> io::Result<Self> {
        Vec::<u8>::read_from::<_, N>(stream)
            .and_then(|buf| String::from_utf8(buf).map_err(io::Error::other))
    }
}

pub trait ReadListFrom<T>: Sized {
    fn read_list<R: ?Sized + Read, const N: usize>(stream: &mut R) -> io::Result<Self>;
}

impl<T: ReadFrom, U: FromIterator<T>> ReadListFrom<T> for U {
    fn read_list<R: ?Sized + Read, const N: usize>(stream: &mut R) -> io::Result<Self> {
        let len = usize::read_from::<_, N>(stream)?;
        std::iter::repeat_with(|| T::read_from(stream))
            .take(len)
            .collect::<io::Result<_>>()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownVariantError<T>(pub T);

impl<T: Copy + std::fmt::Display + Into<char>> std::fmt::Display for UnknownVariantError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let varient = self.0;
        let ch: char = varient.into();
        write!(f, "unknown enum variant: {varient} ('{ch}')",)
    }
}

impl<T> std::error::Error for UnknownVariantError<T> where Self: std::fmt::Display + std::fmt::Debug {}
