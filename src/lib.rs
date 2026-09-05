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

use chrono::{DateTime, Utc};
use deflate::deflate_bytes;
use inflate::inflate_bytes;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    io::{self, Read, Write},
    net::{AddrParseError, Ipv4Addr, SocketAddr, TcpStream},
    path::Path,
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
};

pub const ADDRESS: SocketAddr = SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);

pub const MAX_ATTACHMENTS: usize = 8;

/// Arbitrary data that can be sent alongside a message.
/// If requested, the recipient can download it as a file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Attachment {
    /// At most [`MAX_FILENAME_BYTES`] bytes
    pub filename: String,

    /// At most [`MAX_ALT_TEXT_BYTES`] bytes
    pub alt_text: String,

    /// At most [`MAX_ATTACHMENT_BYTES`] bytes
    pub data: Vec<u8>,
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

/// A message sent by a user containing text and/or files.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct UserMessage {
    /// Server will use [`None`] to indicate "sent by server".
    /// Server will overwrite with client's actual address.
    /// Manual assignment by client will be ignored
    /// (and potentially flagged as suspicious if not matching their actual address).
    pub sender: Option<Identifier>,

    /// Where to send the message (see [`Destination`]).
    /// Absense of a destination (empty string) will be interpreted as global chat.
    pub destination: Destination,

    /// The server will always replace this value with the time *it* received the message.
    /// Clients can be tampered with and are not trusted to make claims about baseline reality.
    pub timestamp: DateTime<Utc>,

    /// The text content of the message
    pub text: String,

    /// At most [`MAX_ATTACHMENTS`] attachments.
    pub attachments: Vec<Attachment>,
}

impl UserMessage {
    pub fn new(destination: Destination, text: String, attachments: Vec<Attachment>) -> Self {
        Self {
            sender: None,
            destination,
            timestamp: Utc::now(),
            text,
            attachments,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageError {
    DstNexists,
    ChatTaken,
    WrongPassword,
    BadUsername,
    SelfSend,
    MsgNexists,
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
            Self::SelfSend => write!(f, "cannot send messages to yourself"),
            Self::MsgNexists => write!(f, "requested message(s) that do(es) not exist"),
        }
    }
}

impl std::error::Error for MessageError {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct ChatName(String);

impl std::fmt::Display for ChatName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl ChatName {
    pub const MAX_BYTES: usize = 255;

    /// Truncate and convert illegal symbols, like spaces into underscores.
    /// This isn't built into the parsing methods because the user should have a chance to confirm after the name is sanitized,
    /// in case it truncates "`assets`" -> "`ass`" or converts "`f𝓻𝓲𝓮𝓷𝓭𝓼`" -> "`f______`"
    pub fn sanitize(s: &str) -> String {
        let mut res = s
            .trim_start_matches(|ch: char| !ch.is_ascii_lowercase() || ch.is_whitespace())
            .trim()
            .replace(|ch: char| !matches!(ch, 'a'..='z' | '0'..='9' | '-'), "-");
        res.truncate(Self::MAX_BYTES); // all characters are ascii, so this shouldn't panic
        debug_assert!(
            Self::is_valid(&res).is_ok(),
            "sanitization should produce valid names"
        );
        res
    }

    /// Test if a candidate name is allowed, returning the error preventing validity if it isn't
    pub fn is_valid(s: &str) -> Result<(), ParseChatNameError> {
        if s.len() > Self::MAX_BYTES {
            Err(ParseChatNameError::TooLong)
        } else if !s.starts_with(|ch: char| ch.is_ascii_lowercase())
            || s.contains(|ch: char| !matches!(ch, 'a'..='z' | '0'..='9' | '-'))
        {
            Err(ParseChatNameError::BadChar)
        } else {
            Ok(())
        }
    }
}

impl std::ops::Deref for ChatName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseChatNameError {
    BadChar,
    TooLong,
}

impl std::fmt::Display for ParseChatNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadChar => write!(
                f,
                "chat names may only contain a-z, 0-9, or -, and must start with a letter"
            ),
            Self::TooLong => write!(
                f,
                "chat names have a maximum length of {}",
                ChatName::MAX_BYTES
            ),
        }
    }
}

impl std::error::Error for ParseChatNameError {}

impl TryFrom<String> for ChatName {
    type Error = ParseChatNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::is_valid(value.as_str()).map(|()| Self(value))
    }
}

impl std::str::FromStr for ChatName {
    type Err = ParseChatNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::is_valid(s).map(|()| Self(s.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Username(String);

impl std::fmt::Display for Username {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Username {
    pub const MAX_BYTES: usize = 255;

    /// Truncate and convert illegal symbols, like spaces into underscores.
    /// This isn't built into the parsing methods because the user should have a chance to confirm after the name is sanitized,
    /// in case it truncates "`assets`" -> "`ass`" or converts "`f𝓻𝓲𝓮𝓷𝓭𝓼`" -> "`f______`"
    pub fn sanitize(s: &str) -> String {
        let mut res = s
            .trim_start_matches(|ch: char| ch.is_ascii_digit() || ch.is_whitespace())
            .trim()
            .replace(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'), "_");
        res.truncate(Self::MAX_BYTES); // all characters are ascii, so this shouldn't panic
        debug_assert!(
            Self::is_valid(&res).is_ok(),
            "sanitization should produce valid names"
        );
        res
    }

    /// Test if a candidate name is allowed, returning the error preventing validity if it isn't
    pub fn is_valid(s: &str) -> Result<(), ParseUserNameError> {
        if s.len() > Self::MAX_BYTES {
            Err(ParseUserNameError::TooLong)
        } else if s.starts_with(|ch: char| ch.is_ascii_digit())
            || s.contains(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        {
            Err(ParseUserNameError::BadChar)
        } else {
            Ok(())
        }
    }
}

impl std::ops::Deref for Username {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseUserNameError {
    BadChar,
    TooLong,
}

impl std::fmt::Display for ParseUserNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadChar => write!(
                f,
                "usernames may only contain a-z, A-Z, 0-9, or _, and cannot start with a number"
            ),
            Self::TooLong => write!(
                f,
                "usernames have a maximum length of {}",
                Username::MAX_BYTES
            ),
        }
    }
}

impl std::error::Error for ParseUserNameError {}

impl TryFrom<String> for Username {
    type Error = ParseUserNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::is_valid(value.as_str()).map(|()| Self(value))
    }
}

impl std::str::FromStr for Username {
    type Err = ParseUserNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::is_valid(s).map(|()| Self(s.to_string()))
    }
}

/// Identifier of a connected client
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Identifier {
    /// Uniquely identifies one connected user, but cannot be trusted to belong to the same one after disconnecting and reconnecting.
    /// A user may have an entirely different socket address after reconnecting.
    ///
    /// This option exists mainly for users who haven't logged in yet or want to remain anonymous.
    /// Socket addresses will be removed from group chats upon disconnecting.
    Socket(SocketAddr),

    /// The username of a client - multiple sockets can be the same user, even at the same time, and can disconnect
    User(Username),
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Identifier::Socket(addr) => addr.fmt(f),
            Identifier::User(name) => name.fmt(f),
        }
    }
}

#[derive(Debug)]
pub enum ParseIdentifierError {
    Socket(AddrParseError),
    Username(ParseUserNameError),
}

impl std::str::FromStr for Identifier {
    type Err = ParseIdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains(':') {
            s.parse()
                .map(Identifier::Socket)
                .map_err(ParseIdentifierError::Socket)
        } else {
            s.parse()
                .map(Identifier::User)
                .map_err(ParseIdentifierError::Username)
        }
    }
}

impl std::fmt::Display for ParseIdentifierError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Socket(e) => write!(f, "failed to parse socket: {e}"),
            Self::Username(e) => write!(f, "invalid username: {e}"),
        }
    }
}

impl std::error::Error for ParseIdentifierError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Socket(e) => Some(e),
            Self::Username(e) => Some(e),
        }
    }
}

/// Where a message should be sent:
/// - A named group chat
/// - A client
///   - via username
///   - via socket
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Destination {
    Chat(ChatName),
    Client(Identifier),
}

impl Default for Destination {
    fn default() -> Self {
        Self::Chat(ChatName::default())
    }
}

impl std::str::FromStr for Destination {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(if let Some(s) = s.strip_prefix('#') {
            // chat
            Self::Chat(s.parse().map_err(io::Error::other)?)
        } else {
            Self::Client(if let Some(s) = s.strip_prefix('@') {
                // user
                Identifier::User(s.parse().map_err(io::Error::other)?)
            } else {
                // address
                Identifier::Socket(s.parse().map_err(|e| io::Error::other(format!(
                    "{e} | tip: prefix chats with `#` (ex: #chat) and users with `@` (ex: @user); anything else will be read as a socket"
                )))?)
            })
        })
    }
}

impl std::fmt::Display for Destination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Chat(chat) => write!(f, "#{chat}"),
            Self::Client(Identifier::User(name)) => write!(f, "@{name}"),
            Self::Client(Identifier::Socket(addr)) => write!(f, "{addr}"),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemberDiff {
    Add,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Message {
    Acknowledge,
    Success,
    Error(MessageError),
    User(UserMessage),
    CreateChat {
        chat: ChatName,
        members: BTreeSet<Identifier>,
    },
    Login {
        username: Username,
        /// TODO: harden this against man-in-the-middle attacks.
        /// the only security it provides rn is that other clients don't have to know it to message you.
        password: String,
    },
    Get {
        source: Destination,
        range: (usize, usize),
    },
    GetResponse(Vec<UserMessage>),
    ModifyChatMembers {
        addrem: MemberDiff,
        chat: ChatName,
        members: BTreeSet<Identifier>,
    },
}

impl Message {
    const INCOMING_MESSAGE_CODE: u8 = 255;

    pub fn send(&self, socket: &mut TcpStream) -> io::Result<()> {
        // write the message to a buffer in case there are errors
        let bytes = ameon::to_bytes(self)?;
        // compress
        let buf = deflate_bytes(&bytes);

        // indicate an incoming message
        socket.write_all(&[Self::INCOMING_MESSAGE_CODE])?;

        // println!("sending {} bytes: {buf:?}", buf.len()); // debug

        // send the message through the socket
        socket.write_all(
            &u64::try_from(buf.len())
                .map_err(io::Error::other)?
                .to_le_bytes(),
        )?;
        socket.write_all(&buf)?;
        Ok(())
    }

    pub fn recv(socket: &mut TcpStream) -> io::Result<Option<Self>> {
        let mut byte = 0;

        // is a message incoming?
        if socket.read(std::slice::from_mut(&mut byte))? == 0 {
            return Ok(None); // no message
        }
        assert_eq!(byte, Self::INCOMING_MESSAGE_CODE, "desynchronized");

        let buf = {
            // read the entire message into a buffer, so we don't get desynced if there are errors
            let mut socket = TempBlockingTkn::begin(socket)?;
            let mut len_buf = [0; _];
            socket.read_exact(&mut len_buf)?;
            let len = u64::from_le_bytes(len_buf);
            let mut socket = (&mut *socket).take(len);
            let len = usize::try_from(len).map_err(io::Error::other)?;
            let mut buf = Vec::with_capacity(len);
            socket.read_to_end(&mut buf)?;
            if buf.len() != len {
                Err(ameon::Error::READ_EXACT_EOF)?;
            }
            buf
        }; // TempBlockingTkn goes out of scope and ends blocking

        // println!("received {} bytes: {buf:?}", buf.len()); // debug

        // decompress
        let bytes = inflate_bytes(&buf).map_err(io::Error::other)?;
        // reinterpret the buffer as a Message
        ameon::from_bytes(&bytes).map_err(Into::into).map(Some)
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
                                if buffer.as_str() == "exit" {
                                    println!("closing");
                                    break;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message() -> ameon::Result<()> {
        for item in [
            Message::Acknowledge,
            Message::Success,
            Message::Error(MessageError::DstNexists),
            Message::User(UserMessage::new(
                Destination::Client(Identifier::Socket("127.0.0.1:5555".parse().unwrap())),
                "hello".to_string(),
                Vec::new(),
            )),
            Message::User(UserMessage::new(
                Destination::Client(Identifier::Socket("10.0.0.5:8080".parse().unwrap())),
                "hi!".to_string(),
                vec![Attachment {
                    filename: "test.txt".to_string(),
                    alt_text: "example".to_string(),
                    data: b"rhgu234ifghqw a9fdh1398 y3rbg9779q842gyb89re8iyuwsda".to_vec(),
                }],
            )),
        ] {
            println!("---\n{item:#?}");
            let bytes = ameon::to_bytes(&item)?;
            println!("{bytes:?}");
            let message: Message = ameon::from_bytes(&bytes)?;
            println!("{message:?}");
            assert_eq!(message, item);
        }
        Ok(())
    }
}
