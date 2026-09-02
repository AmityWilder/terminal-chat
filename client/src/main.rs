#![warn(clippy::undocumented_unsafe_blocks)]

use std::{io, net::TcpStream, sync::mpsc};
use terminal_chat::{ADDRESS, Message, ServerMessage, StdinChannel, UserMessage};

fn display_message(msg: &UserMessage) {
    println!(
        "\x1b[90mfrom \x1b[94m{}\x1b[90m in \x1b[94m{}\x1b[90m:\x1b[0m",
        msg.sender
            .map(|x| x.to_string())
            .unwrap_or("[null]".to_string()),
        &msg.destination
    );
    println!("{}", msg.text);
    for image in &msg.attachments {
        println!("image: {}", image.alt_text);
    }
    println!("\x1b[90m------\x1b[0m")
}

fn main() {
    let stdin = StdinChannel::new();

    let mut stream = TcpStream::connect(ADDRESS).expect("failed to create client");
    stream
        .set_nonblocking(true)
        .expect("cannot set nonblocking");

    let mut curr_dest = String::new();

    loop {
        match stdin.try_recv() {
            Ok(text) => {
                if let Some(cmd) = text.strip_prefix('/') {
                    let (cmd, args) = cmd.split_at(cmd.find(' ').unwrap_or(cmd.len()));
                    let args = args.trim();
                    match cmd {
                        "setchat" => {
                            println!("future messages will be delivered to to `{args}`");
                            if args.contains(|ch: char| ch.is_whitespace()) {
                                eprintln!(
                                    "warning: `{args}` contains whitespace characters.
                                    whitespace in chat names is not currently supported,
                                    so your messages might not be delivered"
                                );
                            }
                            curr_dest.clear();
                            curr_dest.push_str(args);
                        }

                        "newchat" => {
                            let mut it = args.split_whitespace();
                            match it.next().map(str::to_string) {
                                None => eprintln!("missing destination"),

                                Some(destination) => {
                                    match it.map(|x| x.parse()).collect::<Result<_, _>>() {
                                        Err(e) => eprintln!(
                                            "failed to parse member (expecting SocketAddr): {e}"
                                        ),

                                        Ok(members) => {
                                            println!(
                                                "auto-switching current chat to `{destination}`"
                                            );
                                            curr_dest.clear();
                                            curr_dest.push_str(&destination);

                                            let message =
                                                Message::Server(ServerMessage::CreateChat {
                                                    destination,
                                                    members,
                                                });

                                            if let Err(e) = message.write_to(&mut stream) {
                                                eprintln!("failed to send message: {e}");
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        _ => eprintln!("unknown command: {cmd}"),
                    }
                } else {
                    let message = Message::User(UserMessage {
                        sender: None,
                        destination: curr_dest.clone(),
                        text,
                        attachments: Vec::new(),
                    });

                    // println!("sending \"{message:?}\"...");

                    if let Err(e) = message.write_to(&mut stream) {
                        eprintln!("failed to send message: {e}");
                    }
                }
            }

            Err(mpsc::TryRecvError::Disconnected) => {
                println!("client disconnect");
                break;
            }

            Err(mpsc::TryRecvError::Empty) => {}
        }

        match Message::read_from(&mut stream) {
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
                Message::User(umsg) => display_message(&umsg),
                Message::Server(smsg) => {
                    print!("server response: ");
                    match smsg {
                        ServerMessage::Acknowledge => println!("acknowledged"),
                        ServerMessage::Success => println!("success"),
                        ServerMessage::Error(e) => println!("error: {e}"),

                        ServerMessage::CreateChat { .. } => eprintln!("[unintended recipient]"),
                    }
                }
            },
        }
    }
    println!("shutting down");
}
