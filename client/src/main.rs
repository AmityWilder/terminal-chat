#![warn(clippy::undocumented_unsafe_blocks)]

use std::{
    io::{self, Write},
    net::TcpStream,
    path::Path,
    sync::mpsc,
    time::SystemTime,
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
        print!("\x1b[90m in \x1b[94m{chat}");
    }
    print!("\x1b[90m at ");
    match msg.timestamp.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(dur) => print!("{}ms since unix epoch", dur.as_millis()),
        Err(e) => print!("\x1b[91m{e}"),
    }
    println!("\x1b[90m:\x1b[0m\n{}", msg.text);
    println!("\x1b[90mattachments:\x1b[94m");
    for attachment in &msg.attachments {
        println!("- [{}]({})", attachment.alt_text, attachment.filename);
    }
    println!("\x1b[90m------\x1b[0m")
}

fn run_command(
    stream: &mut TcpStream,
    curr_dest: &mut Destination,
    message_history: &[UserMessage],
    cmd: &str,
) {
    let (cmd, args) = cmd.split_at(cmd.find(' ').unwrap_or(cmd.len()));
    let args = args.trim();
    match cmd {
        "setchat" => match args.parse() {
            Ok(dest) => {
                *curr_dest = dest;
                println!("future messages will be delivered to to `{args}`");
                if args.contains(|ch: char| ch.is_whitespace()) {
                    eprintln!(
                        "warning: `{args}` contains whitespace characters. \
                    whitespace in chat names is not currently supported, \
                    so your messages might not be delivered"
                    );
                }
                if let Err(e) = Message::Server(ServerMessage::Get {
                    source: curr_dest.clone(),
                    range: (0, 10),
                })
                .send(stream)
                {
                    eprintln!("failed to request message history: {e}");
                }
            }

            Err(e) => eprintln!("invalid chat name: {e}"),
        },

        "newchat" => {
            let mut it = args.split_whitespace();
            match it.next().map(str::to_string) {
                None => eprintln!("missing destination"),

                Some(destination) => match it
                    .map(|x| x.parse().map_err(io::Error::other))
                    .collect::<Result<_, _>>()
                {
                    Err(e) => eprintln!("failed to parse member: {e}"),

                    Ok(members) => {
                        println!("auto-switching current chat to `{destination}`");
                        *curr_dest = Destination::Chat(destination.clone());

                        let message = Message::Server(ServerMessage::CreateChat {
                            destination,
                            members,
                        });

                        if let Err(e) = message.send(stream) {
                            eprintln!("failed to send message: {e}");
                        }
                    }
                },
            }
        }

        "save" => {
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

        "iam" => {
            if let Some((username, password)) = args.split_once(" ") {
                let message = Message::Server(ServerMessage::Login {
                    username: username.trim().to_string(),
                    password: password.trim().to_string(),
                });

                if let Err(e) = message.send(stream) {
                    eprintln!("failed to send message: {e}");
                }
            } else {
                eprintln!("expected <username> <password>; username cannot contain spaces");
            }
        }

        _ => eprintln!("unknown command: {cmd}"),
    }
}

fn main() {
    let stdin = StdinChannel::new();

    let mut stream = TcpStream::connect(ADDRESS).expect("failed to create client");
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
                    run_command(&mut stream, &mut curr_dest, &message_history, cmd);
                } else {
                    if text.is_empty() {
                        incomplete_message.destination = curr_dest.clone();
                        println!("sending message: {incomplete_message:?}");
                        if let Err(e) =
                            Message::User(std::mem::take(&mut incomplete_message)).send(&mut stream)
                        {
                            eprintln!("failed to send message: {e}");
                        }
                    } else if incomplete_message.text.is_empty() {
                        incomplete_message.text = text;
                    } else {
                        if incomplete_message.attachments.len() < MAX_ATTACHMENTS {
                            if let Some((alt_text, path_str)) = text
                                .strip_prefix('[')
                                .and_then(|x| x.strip_suffix("\")"))
                                .and_then(|x| x.split_once("](\""))
                            {
                                match Attachment::new(Path::new(path_str), alt_text.to_string()) {
                                    Ok(attachment) => {
                                        incomplete_message.attachments.push(attachment)
                                    }
                                    Err(e) => eprintln!("invalid attachment: {e}"),
                                }
                            } else {
                                eprintln!(
                                    "malformed attachment, expected [\x1b[90malt text\x1b[0m](\x1b[90mfile path\x1b[0m)"
                                );
                            }
                        }
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

                Message::Server(smsg) => match smsg {
                    ServerMessage::Acknowledge => println!("\x1b[90macknowledged\x1b[0m"),
                    ServerMessage::Success => println!("\x1b[94msuccess\x1b[0m"),
                    ServerMessage::Error(e) => println!("\x1b[91merror: {e}\x1b[0m"),
                    ServerMessage::GetResponse(list) => {
                        message_history = list;
                        for msg in &message_history {
                            display_message(msg);
                        }
                    }

                    ServerMessage::CreateChat { .. }
                    | ServerMessage::Login { .. }
                    | ServerMessage::Get { .. } => eprintln!("\x1b[90m[unintended]\x1b[0m"),
                },
            },
        }
    }
    println!("shutting down");
}
