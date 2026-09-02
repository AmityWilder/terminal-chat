#![warn(clippy::undocumented_unsafe_blocks)]

use std::{
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
};

pub const ADDRESS: SocketAddr =
    SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

trait FromBytes<const N: usize>: Sized {
    fn from_bytes(data: [u8; N]) -> Self;

    fn read<R: Read>(mut stream: R) -> io::Result<Self> {
        let mut text_len = [0; _];
        stream.read_exact(&mut text_len)?;
        Ok(Self::from_bytes(text_len))
    }
}

trait ToBytes<const N: usize>: Sized {
    fn to_bytes(self) -> [u8; N];

    fn write<W: Write>(self, mut stream: W) -> io::Result<()> {
        stream.write_all(&self.to_bytes())
    }
}

macro_rules! int_byte_conversion {
    ($($Type:ty),+) => {$(
        impl FromBytes<{ std::mem::size_of::<$Type>() }> for $Type {
            fn from_bytes(data: [u8; std::mem::size_of::<$Type>()]) -> Self {
                Self::from_le_bytes(data)
            }
        }
        impl ToBytes<{ std::mem::size_of::<$Type>() }> for $Type {
            fn to_bytes(self) -> [u8; std::mem::size_of::<$Type>()] {
                self.to_le_bytes()
            }
        }
    )+};
}

fn read_str_len<R: Read>(mut stream: R, len: usize) -> io::Result<String> {
    let mut text = vec![0; len];
    stream.read_exact(&mut text)?;
    String::from_utf8(text).map_err(io::Error::other)
}

int_byte_conversion!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize
);

macro_rules! portable_size {
    (
        $(#[$typedoc:meta])*
        $typevis:vis $Type:ident = $Repr:ty;
        $(#[$maxdoc:meta])*
        $maxvis:vis $MAX:ident = $amount:literal;
    ) => {
        /// Portable type for streaming size data
        $(#[$typedoc])*
        $typevis type $Type = $Repr;
        /// Maximum allowable data (for buffer sizing)
        $(#[$maxdoc])*
        $maxvis const $MAX: usize = $amount;
        const _: () = {
            assert!(
                $MAX.ilog2() <= $Type::BITS,
                "limit must fit in type"
            );
        };
    };
}

portable_size! {
    pub AltTextLen = u16;
    pub MAX_ALT_TEXT_BYTES = 1024;
}

portable_size! {
    pub FilenameLen = u8;
    pub MAX_FILENAME_BYTES = 256;
}

portable_size! {
    pub AttachmentSize = u16;
    pub MAX_ATTACHMENT_BYTES = 2048;
}

/// Arbitrary data that can be sent alongside a message.
/// If requested, the recipient can download it as a file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Attachment {
    /// At most [`MAX_FILENAME_BYTES`] bytes
    pub filename: String,

    /// At most [`MAX_ALT_TEXT_BYTES`] bytes
    pub alt_text: String,

    /// At most [`MAX_ATTACHMENT_BYTES`] bytes
    pub data: Vec<u8>,
}

portable_size! {
    pub TextLen = u16;
    pub MAX_TEXT_BYTES = 2048;
}

portable_size! {
    pub AttachmentCount = u8;
    pub MAX_ATTACHMENTS = 8;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Message {
    /// At most [`MAX_TEXT_BYTES`] bytes
    pub text: String,

    /// At most [`MAX_ATTACHMENTS`] images
    pub images: Vec<Attachment>,
}

const START_OF_TEXT: u8 = 1;

impl Message {
    pub fn write_to(&self, stream: &mut TcpStream) -> io::Result<()> {
        stream.write_all(const { &[START_OF_TEXT] })?;
        assert!(self.text.len() <= MAX_TEXT_BYTES, "invalid text length");

        stream.write_all(&TextLen::try_from(self.text.len()).unwrap().to_le_bytes())?;
        assert!(self.images.len() <= MAX_ATTACHMENTS, "invalid image count");

        stream.write_all(
            &AttachmentCount::try_from(self.images.len())
                .unwrap()
                .to_le_bytes(),
        )?;

        stream.write_all(self.text.as_bytes())?;
        for image in &self.images {
            assert!(
                image.filename.len() <= MAX_FILENAME_BYTES,
                "invalid image alt text length"
            );
            stream.write_all(
                &FilenameLen::try_from(image.filename.len())
                    .unwrap()
                    .to_le_bytes(),
            )?;

            assert!(
                image.alt_text.len() <= MAX_ALT_TEXT_BYTES,
                "invalid image alt text length"
            );
            stream.write_all(
                &AltTextLen::try_from(image.alt_text.len())
                    .unwrap()
                    .to_le_bytes(),
            )?;

            assert!(
                image.data.len() <= MAX_ATTACHMENT_BYTES,
                "invalid image data size"
            );
            stream.write_all(
                &AttachmentSize::try_from(image.data.len())
                    .unwrap()
                    .to_le_bytes(),
            )?;

            stream.write_all(image.alt_text.as_bytes())?;
            stream.write_all(&image.data)?;
        }
        Ok(())
    }

    pub fn read_from(stream: &mut TcpStream) -> io::Result<Option<Self>> {
        let mut buf = [0];
        if stream.read(&mut buf)? == 0 {
            return Ok(None);
        }
        assert_eq!(buf, [START_OF_TEXT]);
        stream.set_nonblocking(false)?; // we want to block to read the rest of the message

        let text_len = TextLen::read(&mut *stream)?;
        let attachment_count = AttachmentCount::read(&mut *stream)?;

        let res = Self {
            text: read_str_len(&mut *stream, text_len as usize)?,
            images: std::iter::repeat_with(|| {
                let filename_len = FilenameLen::read(&mut *stream)?;
                let alt_len = AltTextLen::read(&mut *stream)?;
                let attachment_size = AttachmentSize::read(&mut *stream)?;

                let file_name = read_str_len(&mut *stream, filename_len as usize)?;
                let alt_text = read_str_len(&mut *stream, alt_len as usize)?;

                let mut data = vec![0; attachment_size as usize];
                stream.read_exact(&mut data)?;

                Ok(Attachment {
                    filename: file_name,
                    alt_text,
                    data,
                })
            })
            .take(attachment_count as usize)
            .collect::<io::Result<Vec<_>>>()?,
        };

        stream.set_nonblocking(true)?; // now that the message is finished being read, we can go nonblocking again
        Ok(Some(res))
    }
}
