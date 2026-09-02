#![warn(clippy::undocumented_unsafe_blocks)]

use std::{
    collections::{BTreeSet, HashMap, hash_map::Entry},
    io,
    net::{SocketAddr, TcpListener, TcpStream},
    sync::mpsc,
};
use terminal_chat::{ADDRESS, Message, ServerMessage, StdinChannel};

macro_rules! response {
    (($msg:expr) -> $socket:expr) => {{
        let msg: Message = $msg;
        println!("responding to client with \"{msg:?}\"");
        if let Err(e) = msg.write_to($socket) {
            eprintln!("failed to send response to client: {e}");
        }
    }};
}

#[derive(Debug)]
struct Client {
    pub socket: TcpStream,
    pub addr: SocketAddr,
}

fn main() {
    let stdin = StdinChannel::new();

    let listener = TcpListener::bind(ADDRESS).expect("failed to create server");
    listener
        .set_nonblocking(true)
        .expect("cannot set nonblocking");

    let mut clients = Vec::new();
    let mut chats = HashMap::<String, BTreeSet<SocketAddr>>::new();

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
                clients.push(Client { socket, addr })
            }
        }

        let mut i = 0;
        while i < clients.len() {
            match Message::read_from(&mut clients[i].socket) {
                Err(e) => {
                    if e.kind() != io::ErrorKind::WouldBlock {
                        eprintln!(
                            "failed to read message from client {}: {e}",
                            clients[i].addr
                        );
                    }
                }

                Ok(None) => {
                    println!("client {} disconnected", clients[i].addr);
                    clients.swap_remove(i);
                    continue; // `i` now refers to a different client
                }

                Ok(Some(msg)) => match msg {
                    Message::User(mut umsg) => {
                        // response!((Message::Server(ServerMessage::Acknowledge)) -> &mut clients[i].socket);
                        let sender = clients[i].addr;
                        umsg.sender = Some(sender);
                        println!("distributing message:\n```\n{umsg:?}\n```");
                        match chats.get(&umsg.destination) {
                            Some(members) => {
                                for member in members.iter() {
                                    if *member == sender {
                                        continue;
                                    }
                                    if let Some(recipient) =
                                        clients.iter_mut().find(|client| client.addr == *member)
                                    {
                                        if let Err(e) = Message::User(umsg.clone())
                                            .write_to(&mut recipient.socket)
                                        {
                                            eprintln!(
                                                "failed to send message to client `{member}`: {e}"
                                            );
                                        } else {
                                            println!("sent message to client `{member}`")
                                        }
                                    } else {
                                        eprintln!("chat member `{member}` offline");
                                    }
                                }
                                response!((Message::Server(ServerMessage::Success)) -> &mut clients[i].socket);
                            }
                            None => {
                                eprintln!("chat does not exist");
                                response!((Message::Server(ServerMessage::Error("nonexistent".to_string()))) -> &mut clients[i].socket);
                            }
                        }
                    }

                    Message::Server(smsg) => {
                        println!(
                            "instruction from client {}:\n```\n{smsg:?}\n```",
                            clients[i].addr
                        );
                        match smsg {
                            ServerMessage::Acknowledge => println!("acknowledged"),
                            ServerMessage::Success => println!("success"),
                            ServerMessage::Error(e) => eprintln!("error: {e}"),

                            ServerMessage::CreateChat {
                                destination,
                                mut members,
                            } => {
                                println!("creating chat...");
                                match chats.entry(destination) {
                                    Entry::Occupied(_) => {
                                        eprintln!("a chat with this name already exists");
                                        response!((Message::Server(ServerMessage::Error("taken".to_string()))) -> &mut clients[i].socket);
                                    }
                                    Entry::Vacant(entry) => {
                                        members.insert(clients[i].addr);
                                        entry.insert(members);
                                        println!("chat created");
                                        response!((Message::Server(ServerMessage::Success)) -> &mut clients[i].socket);
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
