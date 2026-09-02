#![warn(clippy::undocumented_unsafe_blocks)]

use std::{
    io,
    net::{SocketAddr, TcpStream},
    sync::mpsc,
};
use terminal_chat::{ADDRESS, Message, ServerMessage, StdinChannel, UserMessage};

fn display_message(msg: &UserMessage) {
    println!("{}", msg.text);
    for image in &msg.attachments {
        println!("image: {}", image.alt_text);
    }
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
                    match cmd {
                        "setdst" => {
                            println!("rerouting to `{args}`");
                            curr_dest.clear();
                            curr_dest.push_str(args);
                        }

                        "newchat" => {
                            let mut it = args.split_whitespace();
                            match it.next().map(str::to_string) {
                                None => eprintln!("missing destination"),

                                Some(destination) => {
                                    match it
                                        .map(|x| x.parse())
                                        .collect::<Result<Vec<SocketAddr>, _>>()
                                    {
                                        Err(e) => eprintln!(
                                            "failed to parse member (expecting SocketAddr): {e}"
                                        ),

                                        Ok(members) => {
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
                        destination: curr_dest.clone(),
                        text,
                        attachments: Vec::new(),
                    });

                    println!("sending \"{message:?}\"...");

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
                Message::User(msg) => {
                    println!("message from server:\n```");
                    display_message(&msg);
                    println!("```");
                }
                Message::Server(_) => eprintln!("unexpected message type: {msg:?}"),
            },
        }
    }
    println!("shutting down");
}
