#![warn(clippy::undocumented_unsafe_blocks)]

use clap::Parser;
use std::{
    io::{self, Write},
    net::{SocketAddr, TcpStream},
    path::Path,
    sync::mpsc,
    time::Duration,
};
use terminal_chat::*;

fn display_message(msg: &UserMessage) {
    print!("\x1b[90mfrom ");
    match &msg.sender {
        Some(id) => match id {
            Identifier::Socket(addr) => print!("\x1b[94m{addr}"),
            Identifier::User(name) => print!("\x1b[96m{name}"),
        },
        None => print!("\x1b[30m[null]"),
    }
    if let Destination::Chat(chat) = &msg.destination {
        print!("\x1b[90m in ");
        if chat.is_empty() {
            print!("\x1b[92mglobal");
        } else {
            print!("\x1b[94m{chat}");
        }
    }
    println!(
        "\x1b[90m at {}\x1b[90m:\x1b[0m\n{}",
        msg.timestamp.naive_local(),
        msg.text
    );
    if !msg.attachments.is_empty() {
        println!("\x1b[90mattachments:\x1b[94m");
        for attachment in &msg.attachments {
            println!("- [{}]({})", attachment.alt_text, attachment.filename);
        }
    }
    println!("\x1b[90m------\x1b[0m")
}

/// switch to a chat
fn switch_chat(stream: &mut TcpStream, curr_dest: &mut Destination, args: &str) {
    match args.parse() {
        Err(e) => eprintln!("invalid chat name: {e}"),

        Ok(dest) => {
            *curr_dest = dest;
            println!("future messages will be delivered to to `{args}`");
            if let Err(e) = (Message::Get {
                source: curr_dest.clone(),
                range: (0, 10),
            })
            .send(stream)
            {
                eprintln!("failed to request message history: {e}");
            }
        }
    }
}

/// create a new chat
fn create_chat(stream: &mut TcpStream, curr_dest: &mut Destination, args: &str) {
    let mut it = args.split_whitespace();
    match it.next() {
        None => eprintln!("missing destination"),

        Some(destination) => match destination.parse::<ChatName>() {
            Err(e) => eprintln!("invalid destination: {e}"),

            Ok(destination) => match it.map(|x| x.parse()).collect::<Result<_, _>>() {
                Err(e) => eprintln!("failed to parse member: {e}"),

                Ok(members) => {
                    println!("auto-switching current chat to `{destination}`");
                    *curr_dest = Destination::Chat(destination.clone());

                    let message = Message::CreateChat {
                        destination,
                        members,
                    };

                    if let Err(e) = message.send(stream) {
                        eprintln!("failed to send message: {e}");
                    }
                }
            },
        },
    }
}

// add/remove member(s) within the current chat
fn edit_chat_membership(stream: &mut TcpStream, curr_dest: &Destination, args: &str, remove: bool) {
    match curr_dest {
        Destination::Client(_) => eprintln!("cannot add/remove members from direct message"),

        Destination::Chat(chat) => {
            if chat.is_empty() {
                eprintln!("cannot add/remove members within the global chat");
            } else {
                match args
                    .split_whitespace()
                    .map(|arg| arg.parse::<Identifier>())
                    .collect::<Result<_, _>>()
                {
                    Err(e) => eprintln!("invalid chat member name: {e}"),

                    Ok(members) => {
                        if let Err(e) = (Message::ModifyChatMembers {
                            remove,
                            chat: chat.clone(),
                            members,
                        })
                        .send(stream)
                        {
                            eprintln!("failed to send message: {e}");
                        }
                    }
                }
            }
        }
    }
}

/// save an attachment from the most recent message
fn save_attachment(message_history: &[UserMessage], args: &str) {
    if let Some(message) = message_history.last()
        && let Some(item) = message.attachments.iter().find(|x| x.filename == args)
    {
        let path = Path::new(&item.filename);
        match std::fs::File::create_new(path) {
            Ok(mut file) => match file.write_all(&item.data) {
                Ok(()) => {
                    println!("\x1b[90msaved \x1b[94m`{}`\x1b[0m", path.display());
                }
                Err(e) => eprintln!("failed to store data: {e}"),
            },
            Err(e) => {
                eprintln!(
                    "failed to create file `{}`: {e}",
                    std::env::current_dir().unwrap().join(path).display()
                )
            }
        }
    } else {
        eprintln!("no such attachment");
    }
}

fn log_in(stream: &mut TcpStream, args: &str) {
    if let Some((username, password)) = args.split_once(" ") {
        match username.parse() {
            Ok(username) => {
                let message = Message::Login {
                    username,
                    password: password.trim().to_string(),
                };
                if let Err(e) = message.send(stream) {
                    eprintln!("failed to send message: {e}");
                }
            }
            Err(e) => eprintln!("invalid username: {e}"),
        }
    } else {
        eprintln!("expected <username> <password>; username cannot contain spaces");
    }
}

fn attach_to_message(incomplete_message: &mut UserMessage, args: &str) {
    if incomplete_message.attachments.len() < MAX_ATTACHMENTS {
        if let Some((alt_text, path_str)) = args
            .strip_prefix('[')
            .and_then(|x| x.strip_suffix(')'))
            .and_then(|x| x.split_once("]("))
        {
            let path_str = path_str
                .strip_prefix('\"')
                .and_then(|x| x.strip_suffix('\"')) // should be on both sides or neither
                .unwrap_or(path_str);
            match Attachment::new(Path::new(path_str), alt_text.to_string()) {
                Ok(attachment) => incomplete_message.attachments.push(attachment),
                Err(e) => eprintln!("invalid attachment: {e}"),
            }
        } else {
            eprintln!(
                "malformed attachment, expected [\x1b[90malt text\x1b[0m](\x1b[90mfile path\x1b[0m)"
            );
        }
    } else {
        eprintln!("too many attachments; max: {MAX_ATTACHMENTS}");
    }
}

fn run_command(
    stream: &mut TcpStream,
    curr_dest: &mut Destination,
    incomplete_message: &mut UserMessage,
    message_history: &[UserMessage],
    cmd: &str,
) {
    let (cmd, args) = cmd.split_at(cmd.find(' ').unwrap_or(cmd.len()));
    let args = &args[' '.len_utf8()..];
    match cmd {
        "chat" => switch_chat(stream, curr_dest, args),
        "chat.new" => create_chat(stream, curr_dest, args),
        "chat.add" => edit_chat_membership(stream, curr_dest, args, false),
        "chat.rem" => edit_chat_membership(stream, curr_dest, args, true),
        "save" => save_attachment(message_history, args),
        "iam" => log_in(stream, args),
        "atch" => attach_to_message(incomplete_message, args),

        _ => eprintln!("unknown command: {cmd}"),
    }
}

#[derive(Parser)]
struct StartupCli {
    #[arg(default_value = "127.0.0.1:8080")]
    target: SocketAddr,
}

fn main() {
    let stdin = StdinChannel::new();

    let StartupCli { target } = StartupCli::parse();
    println!("connecting to {target}...");
    let mut stream =
        TcpStream::connect_timeout(&target, Duration::from_secs(2)).expect("failed to connect");
    println!("connected to {}", stream.peer_addr().unwrap());

    stream
        .set_nonblocking(true)
        .expect("cannot set nonblocking");

    let mut curr_dest = Destination::default();
    let mut incomplete_message = UserMessage::default();
    let mut message_history: Vec<UserMessage> = Vec::new();

    loop {
        match stdin.try_recv() {
            Ok(text) => {
                if let Some(cmd) = text.strip_prefix('/') {
                    run_command(
                        &mut stream,
                        &mut curr_dest,
                        &mut incomplete_message,
                        &message_history,
                        cmd,
                    );
                } else {
                    if text.is_empty() {
                        incomplete_message.destination = curr_dest.clone();
                        // println!("sending message: {incomplete_message:?}");
                        if let Err(e) =
                            Message::User(std::mem::take(&mut incomplete_message)).send(&mut stream)
                        {
                            eprintln!("failed to send message: {e}");
                        }
                    } else {
                        let next_line = text.as_str().trim_end();
                        if !incomplete_message.text.is_empty() {
                            let upcoming_len = next_line.len() + '\n'.len_utf8();
                            incomplete_message.text.reserve(upcoming_len);
                            incomplete_message.text.push('\n');
                        }
                        incomplete_message.text.push_str(next_line);
                    }
                }
            }

            Err(mpsc::TryRecvError::Disconnected) => {
                println!("client disconnect");
                break;
            }

            Err(mpsc::TryRecvError::Empty) => {}
        }

        match Message::recv(&mut stream) {
            Err(e) => {
                if e.kind() != io::ErrorKind::WouldBlock {
                    eprintln!("failed to read message from server: {e}");
                    if matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionAborted
                            | io::ErrorKind::ConnectionReset
                            | io::ErrorKind::NotConnected
                    ) {
                        break;
                    }
                }
            }
            Ok(None) => {
                println!("server lost");
                break;
            }
            Ok(Some(msg)) => match msg {
                Message::User(umsg) => {
                    display_message(&umsg);
                    message_history.push(umsg);
                }

                Message::Acknowledge => println!("\x1b[90macknowledged\x1b[0m"),
                Message::Success => println!("\x1b[94msuccess\x1b[0m"),
                Message::Error(e) => println!("\x1b[91merror: {e}\x1b[0m"),
                Message::GetResponse(list) => {
                    message_history = list;
                    for msg in &message_history {
                        display_message(msg);
                    }
                }

                Message::CreateChat { .. }
                | Message::Login { .. }
                | Message::Get { .. }
                | Message::ModifyChatMembers { .. } => {
                    eprintln!("\x1b[90m[unintended]\x1b[0m")
                }
            },
        }
    }
    println!("shutting down");
}
