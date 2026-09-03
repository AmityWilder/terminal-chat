#![warn(clippy::undocumented_unsafe_blocks)]

use std::{
    collections::{BTreeSet, HashMap, hash_map::Entry},
    io,
    net::{SocketAddr, TcpListener, TcpStream},
    sync::mpsc,
};
use terminal_chat::{ADDRESS, Identifier, Message, MessageError, ServerMessage, StdinChannel};

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
    pub username: Option<String>,
    pub socket: TcpStream,
    pub addr: SocketAddr,
}

impl Client {
    pub fn identifier(&self) -> Identifier {
        self.username
            .as_ref()
            .map(|name| Identifier::User(name.clone()))
            .unwrap_or(Identifier::Socket(self.addr))
    }

    pub fn matches(&self, recip: &Identifier) -> bool {
        match recip {
            Identifier::Socket(addr) => &self.addr == addr,
            Identifier::User(name) => self.username.as_ref().is_some_and(|x| x == name),
        }
    }
}

fn route_message(
    clients: &mut [Client],
    users: &mut HashMap<String, String>,
    chats: &mut HashMap<String, BTreeSet<Identifier>>,
    sender_index: usize,
    msg: Message,
) {
    match msg {
        Message::User(mut umsg) => {
            response!((Message::Server(ServerMessage::Acknowledge)) -> &mut clients[sender_index].socket);
            umsg.sender = Some(clients[sender_index].identifier());
            // sanitization
            for attachment in &mut umsg.attachments {
                attachment.filename = attachment.filename.replace(char::is_whitespace, "_");
            }
            let destination = &umsg.destination;
            match chats.get(destination) {
                Some(members) => {
                    let msg = Message::User(umsg);
                    println!("distributing message:\n```\n{msg:?}\n```");
                    for member in members.iter() {
                        if clients[sender_index].matches(member) {
                            continue; // dont echo back to original sender. they know what they wrote.
                        }
                        if let Some(recipient) =
                            clients.iter_mut().find(|client| client.matches(member))
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
                    match chats.entry(destination.clone()) {
                        Entry::Occupied(_) => {
                            eprintln!("a chat with this name already exists");
                            response!((Message::Server(ServerMessage::Error(MessageError::ChatTaken))) -> &mut clients[sender_index].socket);
                        }
                        Entry::Vacant(entry) => {
                            // direct message stream created when group name matches user name and there are no other members
                            if members.is_empty()
                                && !destination
                                    .contains(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                            {
                                members.insert(Identifier::User(destination));
                            }
                            members.insert(
                                clients[sender_index]
                                    .username
                                    .as_ref()
                                    .map(|s| Identifier::User(s.clone()))
                                    .unwrap_or(Identifier::Socket(clients[sender_index].addr)),
                            );
                            entry.insert(members);
                            println!("chat created");
                            response!((Message::Server(ServerMessage::Success)) -> &mut clients[sender_index].socket);
                        }
                    }
                }

                ServerMessage::Login { username, password } => {
                    if username.contains(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')) {
                        println!("invalid username");
                        response!((Message::Server(ServerMessage::Error(MessageError::BadUsername))) -> &mut clients[sender_index].socket);
                    } else {
                        match users.entry(username.clone()) {
                            Entry::Occupied(entry) => {
                                if entry.get() == &password {
                                    clients[sender_index].username = Some(username);
                                    println!("logged into existing user");
                                } else {
                                    println!("incorrect password");
                                    response!((Message::Server(ServerMessage::Error(MessageError::WrongPassword))) -> &mut clients[sender_index].socket);
                                }
                            }
                            Entry::Vacant(entry) => {
                                entry.insert(password);
                                clients[sender_index].username = Some(username);
                                println!("logged into new user");
                            }
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
    let mut users = HashMap::<String, String>::new();
    // TODO: chats should probably have some way of locking out non-members
    let mut chats = HashMap::<String, BTreeSet<Identifier>>::new();

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
                clients.push(Client {
                    username: None,
                    socket,
                    addr,
                })
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
                        println!("disconnecting client {}", clients[i].addr);
                        clients.swap_remove(i);
                        continue; // `i` now refers to a different client
                    }
                }

                Ok(None) => {
                    println!("client {} disconnected", clients[i].addr);
                    clients.swap_remove(i);
                    continue; // `i` now refers to a different client
                }

                Ok(Some(msg)) => route_message(&mut clients, &mut users, &mut chats, i, msg),
            }
            i += 1;
        }
    }
    println!("shutting down");
}
