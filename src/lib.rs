#![warn(clippy::undocumented_unsafe_blocks)]

use std::{
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
};

pub const ADDRESS: SocketAddr =
    SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

macro_rules! read_int {
    (($Int:ty) $stream:expr) => {{
        let mut value = [0; _];
        $stream
            .read_exact(&mut value)
            .map(|()| <$Int>::from_le_bytes(value))
    }};
}

macro_rules! read_vec {
    ([$len:expr] $stream:expr) => {{
        let mut data = vec![0; $len];
        $stream.read_exact(&mut data).map(|()| data)
    }};
}

macro_rules! read_string {
    ([$len:expr] $stream:expr) => {{
        read_vec!([$len] $stream)
            .and_then(|data| String::from_utf8(data).map_err(io::Error::other))
    }};
}

macro_rules! write_int {
    (($Int:ty) [$range:expr] $error:literal ($value:expr) -> $stream:expr) => {{
        assert!($range.contains(&$value), $error);
        $stream.write_all(&<$Int>::try_from($value).unwrap().to_le_bytes())
    }};
}

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
    pub attachments: Vec<Attachment>,
}

const START_OF_TEXT: u8 = 1;

impl Message {
    pub fn write_to(&self, stream: &mut TcpStream) -> io::Result<()> {
        // indicate an incoming message
        stream.write_all(const { &[START_OF_TEXT] })?;

        // write the message content
        write_int!((TextLen) [..=MAX_TEXT_BYTES] "message too long" (self.text.len()) -> stream)?;
        write_int!((AttachmentCount) [..=MAX_ATTACHMENTS] "too many attachments" (self.attachments.len()) -> stream)?;
        stream.write_all(self.text.as_bytes())?;
        for attachment in &self.attachments {
            write_int!((FilenameLen) [..=MAX_FILENAME_BYTES] "filename too long" (attachment.filename.len()) -> stream)?;
            write_int!((AltTextLen) [..=MAX_ALT_TEXT_BYTES] "alt text too long" (attachment.alt_text.len()) -> stream)?;
            write_int!((AttachmentSize) [..=MAX_ATTACHMENT_BYTES] "attachment too large" (attachment.data.len()) -> stream)?;
            stream.write_all(attachment.alt_text.as_bytes())?;
            stream.write_all(attachment.data.as_slice())?;
        }

        Ok(())
    }

    pub fn read_from(stream: &mut TcpStream) -> io::Result<Option<Self>> {
        let mut buf = [0];
        // is a message incoming?
        if stream.read(&mut buf)? == 0 {
            return Ok(None);
        }
        assert_eq!(buf, [START_OF_TEXT]);

        stream.set_nonblocking(false)?; // we want to block to read the rest of the message

        // read the message content
        let text_len = read_int!((TextLen) stream)?;
        let attachment_count = read_int!((AttachmentCount) stream)?;
        let text = read_string!([text_len as usize] stream)?;
        let attachments = std::iter::repeat_with(|| {
            let filename_len = read_int!((FilenameLen) stream)?;
            let alt_len = read_int!((AltTextLen) stream)?;
            let attachment_size = read_int!((AttachmentSize) stream)?;
            let file_name = read_string!([filename_len as usize] stream)?;
            let alt_text = read_string!([alt_len as usize] stream)?;
            let data = read_vec!([attachment_size as usize] stream)?;
            Ok(Attachment {
                filename: file_name,
                alt_text,
                data,
            })
        })
        .take(attachment_count as usize)
        .collect::<io::Result<Vec<_>>>()?;

        stream.set_nonblocking(true)?; // now that the message is finished being read, we can go nonblocking again

        Ok(Some(Self { text, attachments }))
    }
}
