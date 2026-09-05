use clap::{ArgAction, Parser, Subcommand, value_parser};
use std::{
    collections::BTreeSet,
    io::{self, Write},
    net::TcpStream,
    path::{Path, PathBuf},
};
use terminal_chat::*;

#[derive(Parser)]
struct LiveCli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone)]
struct CliArgs<'a> {
    input: &'a str,
}

impl<'a> CliArgs<'a> {
    const DELIMITERS: [char; 3] = ['"', '\'', '`'];

    fn new(input: &'a str) -> Self {
        Self {
            input: input.trim_end(),
        }
    }

    /// Extract a quoted string, not stopping until an unescaped `"` (or end) is reached.
    /// `"`s are not included in the output, but are removed from the stream. Escaped characters are not converted.
    /// The associated bool is true if the closing quote is missing.
    fn extract_string(&mut self) -> (&'a str, bool) {
        let delim = self
            .input
            .chars()
            .next()
            .expect("extract_string expects input to be non-empty");
        assert!(
            Self::DELIMITERS.contains(&delim),
            "extract_string expects a string to include the opening delimiter"
        );
        let input = &self.input[delim.len_utf8()..]; // skip open delimiter, we have visited it
        let mut is_escaped = false; // indicates the previous character was a '\\' (not preceded by another '\\')
        let arg;
        // state machine
        for (i, ch) in input.char_indices() {
            if !is_escaped && ch == delim {
                // closing delimiter
                arg = &self.input[..i];
                self.input = &self.input[i + ch.len_utf8()..]; // exclude delimiter from remainder
                return (arg, false);
            }
            is_escaped = !is_escaped && ch == '\\';
        }
        (arg, self.input) = self.input.split_at(self.input.len());
        (arg, true)
    }

    /// Extract all text until whitespace (or end).
    fn extract_word(&mut self) -> &'a str {
        let mid = self
            .input
            .find(|ch: char| ch.is_whitespace())
            .unwrap_or(self.input.len());
        let word;
        (word, self.input) = self.input.split_at(mid);
        word
    }
}

impl<'a> Iterator for CliArgs<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.input = self.input.trim_start();
        (!self.input.is_empty()).then(|| {
            if self.input.starts_with(Self::DELIMITERS) {
                let (arg, missing_close) = self.extract_string();
                if missing_close {
                    eprintln!(
                        "warning: string argument is missing a closing delimiter (', \", or `)"
                    );
                }
                arg
            } else {
                self.extract_word()
            }
        })
    }
}

#[derive(Subcommand)]
pub enum Command {
    /// Change which chat to send messages in
    #[command(name = "chat.set")]
    SetChat {
        /// Which chat to target
        destination: Destination,
    },

    /// Create a new chat with some users
    #[command(name = "chat.new")]
    CreateChat {
        /// What the chat should be named
        name: ChatName,
        /// Who should be in the chat
        members: Vec<Identifier>,
    },

    /// Edit the member list of an existing chat
    #[command(name = "chat.edit")]
    ModifyChat {
        /// Which chat to edit the member list of - defaults to the current chat
        #[arg(short, long)]
        chat: Option<ChatName>,

        /// The `members` argument lists members to be added
        #[arg(short, long, group = "addrem", action = ArgAction::SetTrue)]
        add: bool,

        /// The `members` argument lists members to be removed
        #[arg(short, long, group = "addrem", action = ArgAction::SetTrue)]
        remove: bool,

        /// Which members to change
        members: Vec<Identifier>,
    },

    /// Download an attachment from a message
    #[command(name = "atch.sav")]
    SaveAttachment {
        /// The index of the attachment to download
        #[arg(short = 'i', long = "index", group = "source", required_unless_present = "filename", value_parser = value_parser!(u8).range(0..MAX_ATTACHMENTS as i64))]
        file_index: Option<u8>,

        /// The filename of the attachment to download
        #[arg(
            short = 'f',
            long = "file",
            group = "source",
            required_unless_present = "file_index"
        )]
        filename: Option<String>,

        /// What to save the file as (defaults to the `filename` argument, within the executable directory)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Add an attachment to the current message
    #[command(name = "atch.add")]
    Attach {
        /// Human-readable description of the file
        #[arg(short, long, alias = "alt", default_value = "")]
        alt_text: String,

        /// Path to the file you want to send
        file: PathBuf,
    },

    /// Tell the server what @ other users should use to reach you
    #[command(name = "iam")]
    Login {
        /// The @ that other clients can message you through
        username: Username,

        /// Your password - if your username isn't currently in use, you will create a new login with this password
        password: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Clap(#[from] clap::error::Error),

    #[error("too many attachments; max {}", MAX_ATTACHMENTS)]
    TooManyAttachments,

    #[error("cannot add/remove members from direct message")]
    EditDm,

    #[error("cannot add/remove members within the global chat")]
    EditGlobal,

    #[error("no such attachment")]
    AtchNexists,

    #[error("failed to store data: {0}")]
    FailedStorage(#[source] io::Error),

    #[error("failed to create file `{}`: {e}", path.display())]
    FailedFileCreation {
        #[source]
        e: io::Error,
        path: PathBuf,
    },

    #[error("invalid attachment: {0}")]
    InvalidAttachment(#[source] io::Error),

    #[error("failed to send request: {0}")]
    RequestFailed(#[source] io::Error),
}

type Result<T> = std::result::Result<T, Error>;

/// switch to a chat
fn switch_chat(
    stream: &mut TcpStream,
    curr_dest: &mut Destination,
    destination: Destination,
) -> Result<()> {
    *curr_dest = destination;
    println!("future messages will be delivered to to `{curr_dest}`");

    // load message from the chat
    let request = Message::Get {
        source: curr_dest.clone(),
        range: (0, 10),
    };
    request.send(stream).map_err(Error::RequestFailed)
}

/// create a new chat
fn create_chat(
    stream: &mut TcpStream,
    curr_dest: &mut Destination,
    chat: ChatName,
    members: BTreeSet<Identifier>,
) -> Result<()> {
    Message::CreateChat {
        chat: chat.clone(),
        members,
    }
    .send(stream)
    .map_err(Error::RequestFailed)?;

    *curr_dest = Destination::Chat(chat);
    println!("auto-switching current chat to `{curr_dest}`");
    Ok(())
}

// add/remove member(s) within the current chat
fn edit_chat_membership(
    stream: &mut TcpStream,
    curr_dest: &Destination,
    chat: Option<&ChatName>,
    members: BTreeSet<Identifier>,
    addrem: MemberDiff,
) -> Result<()> {
    let chat = match chat {
        Some(chat) => chat,
        None => match curr_dest {
            Destination::Client(_) => return Err(Error::EditDm),
            Destination::Chat(chat) => chat,
        },
    };

    if chat.is_empty() {
        return Err(Error::EditGlobal);
    }

    Message::ModifyChatMembers {
        addrem,
        chat: chat.clone(),
        members,
    }
    .send(stream)
    .map_err(Error::RequestFailed)
}

/// save an attachment from the most recent message
fn save_attachment(
    message_history: &[UserMessage],
    file_index: Option<u8>,
    filename: Option<&str>,
    output: Option<&Path>,
) -> Result<()> {
    if let Some(attachment) =
        message_history
            .last()
            .and_then(|message| match (file_index, filename) {
                (None, Some(filename)) => {
                    message.attachments.iter().find(|x| x.filename == filename)
                }
                (Some(index), None) => message.attachments.get(index as usize),

                (Some(_), Some(_)) | (None, None) => unreachable!("clap should reject this"),
            })
    {
        let path = output.unwrap_or(Path::new(attachment.filename.as_str()));
        let mut file = std::fs::File::create_new(path).map_err(|e| Error::FailedFileCreation {
            e,
            path: path.to_path_buf(),
        })?;
        file.write_all(&attachment.data)
            .map_err(Error::FailedStorage)?;
        println!("\x1b[90msaved \x1b[94m`{}`\x1b[0m", path.display());
        Ok(())
    } else {
        Err(Error::AtchNexists)
    }
}

fn log_in(stream: &mut TcpStream, username: Username, password: String) -> Result<()> {
    Message::Login { username, password }
        .send(stream)
        .map_err(Error::RequestFailed)
}

fn attach_to_message(
    incomplete_message: &mut UserMessage,
    alt_text: String,
    path: PathBuf,
) -> Result<()> {
    if incomplete_message.attachments.len() >= MAX_ATTACHMENTS {
        return Err(Error::TooManyAttachments);
    }
    let attachment = Attachment::new(&path, alt_text).map_err(Error::InvalidAttachment)?;
    incomplete_message.attachments.push(attachment);
    Ok(())
}

impl Command {
    pub fn run(
        stream: &mut TcpStream,
        curr_dest: &mut Destination,
        incomplete_message: &mut UserMessage,
        message_history: &[UserMessage],
        input: &str,
    ) -> Result<()> {
        // println!("parsing command: {input:?}"); // debug
        // println!("arguments: {:?}", CliArgs::new(input).collect::<Vec<_>>()); // debug
        match LiveCli::try_parse_from(std::iter::once("").chain(CliArgs::new(input)))?.command {
            Command::SetChat { destination } => switch_chat(stream, curr_dest, destination),

            Command::CreateChat { name, members } => {
                create_chat(stream, curr_dest, name, BTreeSet::from_iter(members))
            }

            Command::ModifyChat {
                chat,
                add,
                remove,
                members,
            } => edit_chat_membership(
                stream,
                curr_dest,
                chat.as_ref(),
                BTreeSet::from_iter(members),
                match (add, remove) {
                    (true, false) | (false, false) => MemberDiff::Add, // default
                    (false, true) => MemberDiff::Remove,
                    (true, true) => unreachable!("clap 'group' option should reject this"),
                },
            ),

            Command::SaveAttachment {
                file_index,
                filename,
                output,
            } => save_attachment(
                message_history,
                file_index,
                filename.as_deref(),
                output.as_deref(),
            ),

            Command::Login { username, password } => log_in(stream, username, password),

            Command::Attach { alt_text, file } => {
                attach_to_message(incomplete_message, alt_text, file)
            }
        }
    }
}
