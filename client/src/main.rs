#![warn(clippy::undocumented_unsafe_blocks)]

use clap::Parser;
use std::{
    io,
    net::{SocketAddr, TcpStream},
    sync::mpsc,
    time::Duration,
};
use terminal_chat::*;

mod commands;
use crate::commands::Command;

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
                    if let Err(e) = Command::run(
                        &mut stream,
                        &mut curr_dest,
                        &mut incomplete_message,
                        &message_history,
                        cmd,
                    ) {
                        if let commands::Error::Clap(error) = e {
                            if let Err(e) = error.print() {
                                eprintln!("\x1b[91mfailed to print: {e}\x1b[0m");
                            }
                        } else {
                            eprintln!("\x1b[91m{e}\x1b[0m");
                        }
                    }
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
