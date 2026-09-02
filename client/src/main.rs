#![warn(clippy::undocumented_unsafe_blocks)]

use std::{io, net::TcpStream, sync::mpsc, thread};
use terminal_chat::{ADDRESS, Message};

fn display_message(msg: &Message) {
    println!("{}", msg.text);
    for image in &msg.attachments {
        println!("image: {}", image.alt_text);
    }
}

fn main() {
    let (sndr, rcvr) = mpsc::channel::<String>();
    thread::Builder::new()
        .name("stdin channel".to_string())
        .spawn(move || {
            loop {
                let mut buffer = String::new();
                io::stdin()
                    .read_line(&mut buffer)
                    .expect("failed to read input");
                while buffer.ends_with(['\n', '\r']) {
                    buffer.pop();
                }
                sndr.send(buffer).expect("failed to send buffer");
            }
        })
        .expect("failed to spawn thread");

    let mut stream = TcpStream::connect(ADDRESS).expect("failed to create client");
    stream
        .set_nonblocking(true)
        .expect("cannot set nonblocking");

    loop {
        match rcvr.try_recv() {
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
                        return;
                    }
                }
            }
            Ok(None) => println!("server responded without a message"),
            Ok(Some(msg)) => {
                println!("message from server {ADDRESS}:\n```");
                display_message(&msg);
                println!("```");
            }
        }
    }
}
