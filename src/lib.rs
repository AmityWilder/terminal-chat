//! NOTE TO SELF: Never trust the client to do something, except to the ends that it is necessary for function.
//! The client can be trusted to connect and send text data & attachments.
//! It can be "trusted" (it may fail, but it doesn't need to be sheilded) to send data following the proper format.
//! However everything else has the potential to be modded by the end user.
//!
//! This library is used by both the client and the server.
//! It is in the best interest of a client to use it, however there is no GUARANTEE they will.
//! Thus, security features CANNOT be exclusively here. The server much verify and sanitize everything sent by a client.
//! Just because a type's "WriteTo" implementation checks for validity does NOT mean data will be valid when it reaches the server.

#![warn(clippy::undocumented_unsafe_blocks)]

use crate::serde::*;
use std::{
    collections::BTreeSet,
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    path::Path,
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::SystemTime,
};

mod serde;

pub const ADDRESS: SocketAddr = SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);

pub const MAX_ATTACHMENTS: usize = 8;

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

impl WriteTo for Attachment {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        self.filename.write_to::<_, 2>(stream)?;
        self.alt_text.write_to::<_, 1>(stream)?;
        self.data.write_to::<_, 4>(stream)?;
        Ok(())
    }
}

impl ReadFrom for Attachment {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        Ok(Self {
            filename: String::read_from::<_, 2>(stream)?,
            alt_text: String::read_from::<_, 1>(stream)?,
            data: Vec::read_from::<_, 4>(stream)?,
        })
    }
}

impl Attachment {
    pub fn new(path: &Path, alt_text: String) -> io::Result<Self> {
        if path.is_file() {
            let data = std::fs::read(path)?;
            let filename = path
                .file_name()
                .expect("file cannot terminate in `..`")
                .to_string_lossy()
                .to_string();
            println!("shortened filename: `{filename}`");
            Ok(Self {
                filename,
                alt_text,
                data,
            })
        } else {
            Err(io::Error::other("no such file"))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserMessage {
    /// Server will use [`None`] to indicate "sent by server".
    /// Server will overwrite with client's actual address.
    /// Manual assignment by client will be ignored
    /// (and potentially flagged as suspicious if not matching their actual address).
    pub sender: Option<Identifier>,

    pub destination: Destination,

    /// The server will always replace this value with the time *it* received the message.
    /// Clients can be tampered with and are not trusted to make claims about baseline reality.
    pub timestamp: SystemTime,

    /// At most [`MAX_TEXT_BYTES`] bytes
    pub text: String,

    /// At most [`MAX_ATTACHMENTS`] images
    pub attachments: Vec<Attachment>,
}

impl Default for UserMessage {
    fn default() -> Self {
        Self {
            sender: Default::default(),
            destination: Default::default(),
            timestamp: SystemTime::now(),
            text: Default::default(),
            attachments: Default::default(),
        }
    }
}

impl WriteTo for UserMessage {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        self.sender.write_to(stream)?;
        self.destination.write_to(stream)?;
        self.timestamp.write_to(stream)?;
        self.text.write_to::<_, 2>(stream)?;
        self.attachments.iter().write_list::<_, 1>(stream)?;
        Ok(())
    }
}

impl ReadFrom for UserMessage {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        Ok(Self {
            sender: Option::read_from(stream)?,
            destination: Destination::read_from(stream)?,
            timestamp: SystemTime::read_from(stream)?,
            text: String::read_from::<_, 2>(stream)?,
            attachments: Vec::read_list::<_, 1>(stream)?,
        })
    }
}

impl UserMessage {
    pub fn new(destination: Destination, text: String, attachments: Vec<Attachment>) -> Self {
        Self {
            sender: None,
            destination,
            timestamp: SystemTime::now(),
            text,
            attachments,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageError {
    DstNexists,
    ChatTaken,
    WrongPassword,
    BadUsername,
}

impl WriteTo for MessageError {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        match self {
            Self::DstNexists => 0u8,
            Self::ChatTaken => 1u8,
            Self::WrongPassword => 2u8,
            Self::BadUsername => 3u8,
        }
        .write_to(stream)
    }
}

impl ReadFrom for MessageError {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        match u8::read_from(stream)? {
            0 => Ok(Self::DstNexists),
            1 => Ok(Self::ChatTaken),
            2 => Ok(Self::WrongPassword),
            3 => Ok(Self::BadUsername),

            x => Err(io::Error::other(UnknownVariantError(x))),
        }
    }
}

impl std::fmt::Display for MessageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DstNexists => write!(
                f,
                "nonexistent destination; chat is undefined or recipient is unrecognized"
            ),
            Self::ChatTaken => write!(
                f,
                "a chat with that name already exists, you do not have permission to overwrite it"
            ),
            Self::WrongPassword => write!(f, "incorrect password, try again"),
            Self::BadUsername => write!(f, "usernames may only contain a-z, A-Z, 0-9, and _"),
        }
    }
}

impl std::error::Error for MessageError {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Identifier {
    Socket(SocketAddr),
    User(String),
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Identifier::Socket(addr) => addr.fmt(f),
            Identifier::User(name) => name.fmt(f),
        }
    }
}

impl WriteTo for Identifier {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        match self {
            Self::Socket(_) => 0u8,
            Self::User(_) => 1u8,
        }
        .write_to(stream)?;
        match self {
            Self::Socket(addr) => addr.to_string().write_to::<_, 1>(stream),
            Self::User(name) => name.write_to::<_, 1>(stream),
        }
    }
}

impl ReadFrom for Identifier {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        match u8::read_from(stream)? {
            0 => SocketAddr::read_from(stream).map(Self::Socket),
            1 => String::read_from::<_, 1>(stream).map(Self::User),

            x => Err(io::Error::other(UnknownVariantError(x))),
        }
    }
}

#[derive(Debug)]
pub enum ParseIdentifierError {
    Socket(<SocketAddr as std::str::FromStr>::Err),
    Username,
}

impl std::str::FromStr for Identifier {
    type Err = ParseIdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains(':') {
            s.parse()
                .map(Identifier::Socket)
                .map_err(ParseIdentifierError::Socket)
        } else if !s.contains(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')) {
            Ok(Identifier::User(s.to_string()))
        } else {
            Err(ParseIdentifierError::Username)
        }
    }
}

impl std::fmt::Display for ParseIdentifierError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Socket(e) => write!(f, "failed to parse socket: {e}"),
            Self::Username => write!(
                f,
                "invalid username characters: can only be alphanumeric ASCII"
            ),
        }
    }
}

impl std::error::Error for ParseIdentifierError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Socket(e) => Some(e),
            Self::Username => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Destination {
    Chat(String),
    Client(Identifier),
}

impl Default for Destination {
    fn default() -> Self {
        Self::Chat(String::new())
    }
}

impl std::str::FromStr for Destination {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(if let Some(s) = s.strip_prefix('#') {
            // chat
            Self::Chat(s.to_string())
        } else {
            Self::Client(if let Some(s) = s.strip_prefix('@') {
                // user
                Identifier::User(s.to_string())
            } else {
                // address
                Identifier::Socket(s.parse().map_err(|e| io::Error::other(format!("{e} | tip: prefix chats with `#` (ex: #chat) and users with `@` (ex: @user); anything else will be read as a socket")))?)
            })
        })
    }
}

impl WriteTo for Destination {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        match self {
            Self::Chat(chat) => {
                0u8.write_to(stream)?;
                chat.write_to::<_, 2>(stream)
            }
            Self::Client(id) => {
                1u8.write_to(stream)?;
                id.write_to(stream)
            }
        }
    }
}

impl ReadFrom for Destination {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        match u8::read_from(stream)? {
            0 => String::read_from::<_, 2>(stream).map(Self::Chat),
            1 => Identifier::read_from(stream).map(Self::Client),

            x => Err(io::Error::other(UnknownVariantError(x))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ServerMessage {
    Acknowledge,
    Success,
    Error(MessageError),
    CreateChat {
        destination: String,
        members: BTreeSet<Identifier>,
    },
    Login {
        username: String,
        /// TODO: harden this against man-in-the-middle attacks.
        /// the only security it provides rn is that other clients don't have to know it to message you.
        password: String,
    },
}

impl WriteTo for ServerMessage {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        match self {
            Self::Acknowledge => 0u8,
            Self::Success => 1u8,
            Self::Error(_) => 2u8,
            Self::CreateChat { .. } => 3u8,
            Self::Login { .. } => 4u8,
        }
        .write_to(stream)?;
        match self {
            Self::Acknowledge | Self::Success => Ok(()),
            Self::Error(e) => e.write_to(stream),
            Self::CreateChat {
                destination,
                members,
            } => {
                destination.write_to::<_, 1>(stream)?;
                members.iter().write_list::<_, 2>(stream)
            }
            Self::Login { username, password } => {
                username.write_to::<_, 1>(stream)?;
                password.write_to::<_, 1>(stream)
            }
        }
    }
}

impl ReadFrom for ServerMessage {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        match u8::read_from(stream)? {
            0 => Ok(Self::Acknowledge),
            1 => Ok(Self::Success),
            2 => MessageError::read_from(stream).map(Self::Error),
            3 => Ok(Self::CreateChat {
                destination: String::read_from::<_, 1>(stream)?,
                members: BTreeSet::from_iter(Vec::read_list::<_, 2>(stream)?),
            }),
            4 => Ok(Self::Login {
                username: String::read_from::<_, 1>(stream)?,
                password: String::read_from::<_, 1>(stream)?,
            }),

            x => Err(io::Error::other(UnknownVariantError(x))),
        }
    }
}

/// Switches a TcpStream to blocking upon creation.
/// Resets it to non-blocking when dropped.
/// This way, even if there's an unexpected early return (like an error), the stream will reset to non-blocking.
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

impl WriteTo for Message {
    fn write_to<W: ?Sized + Write>(&self, stream: &mut W) -> io::Result<()> {
        match self {
            Self::User(_) => 0u8,
            Self::Server(_) => 1u8,
        }
        .write_to(stream)?;
        match self {
            Self::User(msg) => msg.write_to(stream),
            Self::Server(msg) => msg.write_to(stream),
        }
    }
}

impl ReadFrom for Message {
    fn read_from<R: ?Sized + Read>(stream: &mut R) -> io::Result<Self> {
        match u8::read_from(stream)? {
            0 => UserMessage::read_from(stream).map(Self::User),
            1 => ServerMessage::read_from(stream).map(Self::Server),

            x => Err(io::Error::other(UnknownVariantError(x))),
        }
    }
}

impl Message {
    const INCOMING_MESSAGE_CODE: u8 = 255;

    pub fn send(&self, socket: &mut TcpStream) -> io::Result<()> {
        // write the message to a buffer in case there are errors
        let mut buf: Vec<u8> = Vec::new();
        self.write_to(&mut buf)?;

        // indicate an incoming message
        socket.write_all(&[Self::INCOMING_MESSAGE_CODE])?;

        // send the message through the socket
        buf.write_to::<_, 8>(socket)
    }

    pub fn recv(socket: &mut TcpStream) -> io::Result<Option<Self>> {
        let mut msg_type = 0;

        // is a message incoming?
        if socket.read(std::slice::from_mut(&mut msg_type))? == 0 {
            return Ok(None); // no message
        }

        let buf: Vec<u8> = {
            // read the entire message into a buffer, so we don't get desynced if there are errors
            Vec::read_from::<_, 8>(&mut *TempBlockingTkn::begin(socket)?)?
        }; // TempBlockingTkn goes out of scope and ends blocking

        // reinterpret the buffer as a Message
        Self::read_from(&mut buf.as_slice()).map(Some)
    }
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
