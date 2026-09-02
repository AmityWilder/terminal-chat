#![warn(clippy::undocumented_unsafe_blocks)]

use std::{io, net::TcpStream, sync::mpsc};
use terminal_chat::{ADDRESS, Message, StdinChannel};

fn display_message(msg: &Message) {
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

    loop {
        match stdin.try_recv() {
            Ok(text) => {
                let message = Message {
                    text,
                    attachments: Vec::new(),
                };

                println!("sending \"{message:?}\"...");

                message
                    .write_to(&mut stream)
                    .expect("failed to send message");
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
            Ok(Some(msg)) => {
                println!("message from server {ADDRESS}:\n```");
                display_message(&msg);
                println!("```");
            }
        }
    }
    println!("shutting down");
}
