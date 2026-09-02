#![warn(clippy::undocumented_unsafe_blocks)]

use std::{
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
};

pub const ADDRESS: SocketAddr = SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
const START_OF_TEXT: u8 = 1;
const START_OF_SVRM: u8 = 2;

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
        let mut data = vec![Default::default(); $len];
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
        if $range.contains(&$value) {
            $stream.write_all(
                &<$Int>::try_from($value)
                    .expect("assertion should cover this")
                    .to_le_bytes(),
            )
        } else {
            Err(io::Error::other($error))
        }
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
    pub DestinationLen = u16;
    pub MAX_DESTINATION_BYTES = 2048;
}

/// At most [`MAX_DESTINATION_BYTES`] bytes.
type Destination = String;

portable_size! {
    pub TextLen = u16;
    pub MAX_TEXT_BYTES = 2048;
}

portable_size! {
    pub AttachmentCount = u8;
    pub MAX_ATTACHMENTS = 8;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct UserMessage {
    pub destination: Destination,

    /// At most [`MAX_TEXT_BYTES`] bytes
    pub text: String,

    /// At most [`MAX_ATTACHMENTS`] images
    pub attachments: Vec<Attachment>,
}

impl UserMessage {
    pub fn write_to<W: Write>(&self, mut stream: W) -> io::Result<()> {
        // write the message content
        write_int!((DestinationLen) [..MAX_DESTINATION_BYTES] "destination too long" (self.destination.len()) -> stream)?;
        write_int!((TextLen) [..=MAX_TEXT_BYTES] "message too long" (self.text.len()) -> stream)?;
        write_int!((AttachmentCount) [..=MAX_ATTACHMENTS] "too many attachments" (self.attachments.len()) -> stream)?;
        stream.write_all(self.destination.as_bytes())?;
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

    pub fn read_from<R: Read>(mut stream: R) -> io::Result<Self> {
        // read the message content
        let destination_len = read_int!((DestinationLen) stream)?;
        let text_len = read_int!((TextLen) stream)?;
        let attachment_count = read_int!((AttachmentCount) stream)?;
        let destination = read_string!([destination_len as usize] stream)?;
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

        Ok(Self {
            destination,
            text,
            attachments,
        })
    }
}

portable_size! {
    pub MemberCount = u8;
    pub MAX_CHAT_MEMBERS = 256;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ServerMessage {
    CreateChat {
        destination: Destination,
        members: Vec<SocketAddr>,
    },
}

impl ServerMessage {
    const CREATE_CHAT_VARIANT: u8 = 0;

    pub fn write_to<W: Write>(&self, mut stream: W) -> io::Result<()> {
        match self {
            Self::CreateChat {
                destination,
                members,
            } => {
                stream.write_all(&[Self::CREATE_CHAT_VARIANT])?;
                write_int!((DestinationLen) [..=MAX_DESTINATION_BYTES] "destination too long" (destination.len()) -> stream)?;
                write_int!((MemberCount) [..=MAX_CHAT_MEMBERS] "too many members" (members.len()) -> stream)?;
                stream.write_all(destination.as_bytes())?;
                for member in members {
                    let addr = member.to_string();
                    write_int!((u8) [..=256] "address too long" (addr.len()) -> stream)?;
                    stream.write_all(addr.as_bytes())?;
                }
            }
        }
        Ok(())
    }

    pub fn read_from<R: Read>(mut stream: R) -> io::Result<Self> {
        match read_int!((u8) stream)? {
            Self::CREATE_CHAT_VARIANT => {
                let dst_len = read_int!((DestinationLen) stream)?;
                let member_count = read_int!((MemberCount) stream)?;
                Ok(Self::CreateChat {
                    destination: read_string!([dst_len as usize] stream)?,
                    members: std::iter::repeat_with(|| {
                        let addr_len = read_int!((u8) stream)?;
                        let addr = read_string!([addr_len as usize] stream)?;
                        addr.parse().map_err(io::Error::other)
                    })
                    .take(member_count as usize)
                    .collect::<io::Result<Vec<_>>>()?,
                })
            }

            _ => Err(io::Error::other("unknown variant")),
        }
    }
}

struct TempBlockingTkn<'a>(&'a mut TcpStream);

impl std::ops::Deref for TempBlockingTkn<'_> {
    type Target = TcpStream;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl std::ops::DerefMut for TempBlockingTkn<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl Drop for TempBlockingTkn<'_> {
    fn drop(&mut self) {
        self.0
            .set_nonblocking(true)
            .expect("cannot set nonblocking");
    }
}

impl<'a> TempBlockingTkn<'a> {
    fn begin(stream: &'a mut TcpStream) -> io::Result<Self> {
        stream.set_nonblocking(false)?;
        Ok(Self(stream))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Message {
    User(UserMessage),
    Server(ServerMessage),
}

impl Message {
    pub fn write_to(&self, stream: &mut TcpStream) -> io::Result<()> {
        match self {
            Self::User(msg) => {
                // indicate an incoming message
                stream.write_all(&[START_OF_TEXT])?;
                msg.write_to(stream)
            }
            Self::Server(msg) => {
                // indicate an incoming message
                stream.write_all(&[START_OF_SVRM])?;
                msg.write_to(stream)
            }
        }
    }

    pub fn read_from(stream: &mut TcpStream) -> io::Result<Option<Self>> {
        let mut msg_type = 0;

        // is a message incoming?
        if stream.read(std::slice::from_mut(&mut msg_type))? == 0 {
            return Ok(None); // no message
        }

        // block until the full message is read
        let mut tkn = TempBlockingTkn::begin(stream)?;

        Ok(Some(match msg_type {
            START_OF_TEXT => Message::User(UserMessage::read_from(&mut *tkn)?),
            START_OF_SVRM => Message::Server(ServerMessage::read_from(&mut *tkn)?),

            _ => {
                eprintln!(
                    "unknown message type: {msg_type:?} ('{}')",
                    char::from(msg_type)
                );
                return Err(io::Error::other("unexpected message type"));
            }
        }))
    } // token goes out of scope and ends blocking
}

#[derive(Debug)]
pub struct StdinChannel {
    rcvr: Receiver<String>,
    _thread: JoinHandle<()>,
}

impl std::ops::Deref for StdinChannel {
    type Target = Receiver<String>;

    fn deref(&self) -> &Self::Target {
        &self.rcvr
    }
}

impl std::ops::DerefMut for StdinChannel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.rcvr
    }
}

impl StdinChannel {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let (sndr, rcvr) = mpsc::channel::<String>();
        Self {
            rcvr,
            _thread: thread::Builder::new()
                .name("stdin channel".to_string())
                .spawn(move || {
                    loop {
                        let mut buffer = String::new();
                        match io::stdin().read_line(&mut buffer) {
                            Ok(0) => {
                                println!("stdin closed");
                                break;
                            }
                            Ok(_) => {
                                while buffer.ends_with(['\n', '\r']) {
                                    buffer.pop();
                                }
                                if let Err(e) = sndr.send(buffer) {
                                    eprintln!("failed to send buffer: {e}");
                                }
                            }
                            Err(e) => {
                                eprintln!("failed to read stdin: {e}");
                                break;
                            }
                        }
                    }
                })
                .expect("failed to spawn thread"),
        }
    }
}
