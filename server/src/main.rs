#![warn(clippy::undocumented_unsafe_blocks)]

use std::{
    collections::{HashMap, hash_map::Entry},
    io,
    net::TcpListener,
    sync::mpsc,
};
use terminal_chat::{ADDRESS, Message, ServerMessage, StdinChannel, UserMessage};

macro_rules! response {
    (($text:expr) -> ($addr:expr) via $socket:expr) => {{
        let text = $text.to_string();
        println!("responding to client with \"{text}\"");
        if let Err(e) = Message::User(UserMessage {
            destination: $addr.to_string(),
            text,
            attachments: Vec::new(),
        })
        .write_to($socket)
        {
            eprintln!("failed to send response to client: {e}");
        }
    }};
}

fn main() {
    let stdin = StdinChannel::new();

    let listener = TcpListener::bind(ADDRESS).expect("failed to create server");
    listener
        .set_nonblocking(true)
        .expect("cannot set nonblocking");

    let mut clients = Vec::new();
    let mut chats = HashMap::new();

    println!("awaiting client connect...");
    loop {
        match stdin.try_recv() {
            Ok(input) => match input.as_str() {
                "ping" => println!("echo"),
                _ => eprintln!("unknown command: {input}"),
            },

            Err(mpsc::TryRecvError::Disconnected) => {
                println!("server disconnect");
                break;
            }

            Err(mpsc::TryRecvError::Empty) => {}
        }

        match listener.accept() {
            Err(e) => {
                if e.kind() != io::ErrorKind::WouldBlock {
                    eprintln!("failed to connect: {e}");
                }
            }
            Ok((socket, addr)) => {
                println!("client {addr} connected");
                clients.push((socket, addr))
            }
        }

        let mut i = 0;
        while i < clients.len() {
            let (socket, addr) = &mut clients[i];
            match Message::read_from(socket) {
                Err(e) => {
                    if e.kind() != io::ErrorKind::WouldBlock {
                        eprintln!("failed to read message from client {addr}: {e}");
                    }
                }

                Ok(None) => {
                    println!("client {addr} disconnected");
                    clients.swap_remove(i);
                    continue; // `i` now refers to a different client
                }

                Ok(Some(msg)) => match msg {
                    Message::User(umsg) => {
                        println!("message from client {addr}:\n```\n{umsg:?}\n```");
                        response!(("acknowledged") -> (addr) via socket);
                    }
                    Message::Server(smsg) => {
                        println!("instruction from client {addr}:\n```\n{smsg:?}\n```");
                        match smsg {
                            ServerMessage::CreateChat {
                                destination,
                                members,
                            } => {
                                println!("creating chat...");
                                match chats.entry(destination) {
                                    Entry::Occupied(_) => {
                                        eprintln!("a chat with this name already exists");
                                        response!(("failed: already exists") -> (addr) via socket);
                                    }
                                    Entry::Vacant(entry) => {
                                        entry.insert(members).insert(*addr);
                                        println!("chat created");
                                        response!(("success") -> (addr) via socket);
                                    }
                                }
                            }
                        }
                    }
                },
            }
            i += 1;
        }
    }
    println!("shutting down");
}
