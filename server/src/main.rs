#![warn(clippy::undocumented_unsafe_blocks)]

use std::{
    collections::{BTreeSet, HashMap, hash_map::Entry},
    io,
    net::{SocketAddr, TcpListener, TcpStream},
    sync::mpsc,
};
use terminal_chat::{ADDRESS, Message, MessageError, ServerMessage, StdinChannel};

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

fn route_message(
    clients: &mut [Client],
    chats: &mut HashMap<String, BTreeSet<SocketAddr>>,
    sender_index: usize,
    msg: Message,
) {
    match msg {
        Message::User(mut umsg) => {
            response!((Message::Server(ServerMessage::Acknowledge)) -> &mut clients[sender_index].socket);
            let sender = clients[sender_index].addr;
            umsg.sender = Some(sender);
            let destination = &umsg.destination;
            match chats.get(destination) {
                Some(members) => {
                    let msg = Message::User(umsg);
                    println!("distributing message:\n```\n{msg:?}\n```");
                    for member in members.iter() {
                        if *member == sender {
                            continue; // dont echo back to original sender. they know what they wrote.
                        }
                        if let Some(recipient) =
                            clients.iter_mut().find(|client| client.addr == *member)
                        {
                            if let Err(e) = msg.clone().write_to(&mut recipient.socket) {
                                eprintln!("failed to send message to client `{member}`: {e}");
                            } else {
                                println!("sent message to client `{member}`")
                            }
                        } else {
                            eprintln!("chat member `{member}` offline");
                        }
                    }
                    response!((Message::Server(ServerMessage::Success)) -> &mut clients[sender_index].socket);
                }
                None => {
                    eprintln!("chat `{destination}` does not exist");
                    response!((Message::Server(ServerMessage::Error(MessageError::DstNexists))) -> &mut clients[sender_index].socket);
                }
            }
        }

        Message::Server(smsg) => {
            println!(
                "instruction from client {}:\n```\n{smsg:?}\n```",
                clients[sender_index].addr
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
                            response!((Message::Server(ServerMessage::Error(MessageError::ChatTaken))) -> &mut clients[sender_index].socket);
                        }
                        Entry::Vacant(entry) => {
                            members.insert(clients[sender_index].addr);
                            entry.insert(members);
                            println!("chat created");
                            response!((Message::Server(ServerMessage::Success)) -> &mut clients[sender_index].socket);
                        }
                    }
                }
            }
        }
    }
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

                Ok(Some(msg)) => route_message(&mut clients, &mut chats, i, msg),
            }
            i += 1;
        }
    }
    println!("shutting down");
}
