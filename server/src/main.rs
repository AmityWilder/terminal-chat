#![warn(clippy::undocumented_unsafe_blocks)]

use std::{
    collections::{BTreeSet, HashMap, hash_map::Entry},
    io,
    net::{SocketAddr, TcpListener, TcpStream},
    sync::mpsc,
};
use terminal_chat::*;

macro_rules! response {
    (($msg:expr) -> $socket:expr) => {{
        let msg: Message = $msg;
        println!("responding to client with \"{msg:?}\"");
        if let Err(e) = msg.send($socket) {
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

fn send_message_to_client(msg: &Message, clients: &mut [Client], recipient: &Identifier) {
    if let Some(client) = clients.iter_mut().find(|client| client.matches(recipient)) {
        match msg.clone().send(&mut client.socket) {
            Ok(()) => println!("sent message to client `{recipient}`"),
            Err(e) => eprintln!("failed to send message to client `{recipient}`: {e}"),
        }
    } else {
        eprintln!("client `{recipient}` offline");
    }
}

/// Direct message stream ID (user pair)
fn dmid(sndr: Identifier, rcvr: Identifier) -> Result<[Identifier; 2], MessageError> {
    match sndr.cmp(&rcvr) {
        std::cmp::Ordering::Less => Ok([sndr.clone(), rcvr.clone()]),
        std::cmp::Ordering::Greater => Ok([rcvr.clone(), sndr.clone()]),

        std::cmp::Ordering::Equal => {
            eprintln!("cannot deliver to self");
            Err(MessageError::SelfSend)
        }
    }
}

fn route_message(
    clients: &mut [Client],
    users: &mut HashMap<String, String>,
    chats: &mut HashMap<String, Chat>,
    dm_history: &mut HashMap<[Identifier; 2], MessageHistory>,
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
            match &umsg.destination {
                Destination::Chat(chat) => match chats.get_mut(chat) {
                    Some(Chat { members, messages }) => {
                        messages.push(umsg.clone());
                        println!("distributing message:\n```\n{umsg:?}\n```");
                        let msg = Message::User(umsg);
                        for member in members.iter() {
                            if clients[sender_index].matches(member) {
                                continue; // dont echo back to original sender. they know what they wrote.
                            }
                            send_message_to_client(&msg, clients, member);
                        }
                        response!((Message::Server(ServerMessage::Success)) -> &mut clients[sender_index].socket);
                    }
                    None => {
                        eprintln!("chat `{chat:?}` does not exist");
                        response!((Message::Server(ServerMessage::Error(MessageError::DstNexists))) -> &mut clients[sender_index].socket);
                    }
                },

                Destination::Client(identifier) => {
                    match dmid(clients[sender_index].identifier(), identifier.clone()) {
                        Err(e) => {
                            eprintln!("could not get user pair: {e}");
                            response!((Message::Server(ServerMessage::Error(e))) -> &mut clients[sender_index].socket);
                        }

                        Ok(user_pair) => {
                            let identifier = identifier.clone(); // identifier is a partial borrow of umsg
                            dm_history.entry(user_pair).or_default().push(umsg.clone());
                            println!("sending direct message message:\n```\n{umsg:?}\n```");
                            send_message_to_client(&Message::User(umsg), clients, &identifier)
                        }
                    }
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
                            members.insert(
                                clients[sender_index]
                                    .username
                                    .as_ref()
                                    .map(|s| Identifier::User(s.clone()))
                                    .unwrap_or(Identifier::Socket(clients[sender_index].addr)),
                            );
                            entry.insert(Chat {
                                members,
                                messages: Vec::new(),
                            });
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

                ServerMessage::Get {
                    source,
                    range: (start, end),
                } => {
                    let history = match source {
                        Destination::Chat(chat) => chats.get(&chat).map(|chat| &chat.messages),

                        Destination::Client(identifier) => {
                            match dmid(clients[sender_index].identifier(), identifier.clone()) {
                                Err(e) => {
                                    eprintln!("could not get user pair: {e}");
                                    response!((Message::Server(ServerMessage::Error(e))) -> &mut clients[sender_index].socket);
                                    return;
                                }

                                Ok(user_pair) => dm_history.get(&user_pair),
                            }
                        }
                    };
                    if let Some(history) = history {
                        let messages = history
                            .iter()
                            .rev()
                            .skip(start)
                            .take(end.saturating_sub(start))
                            .cloned()
                            .collect();
                        response!((Message::Server(ServerMessage::GetResponse(messages))) -> &mut clients[sender_index].socket);
                    } else {
                        response!((Message::Server(ServerMessage::Error(MessageError::DstNexists))) -> &mut clients[sender_index].socket);
                    }
                }

                ServerMessage::GetResponse(_) => {
                    eprintln!("invlalid: server did not request messages");
                }
            }
        }
    }
}

type MessageHistory = Vec<UserMessage>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
struct Chat {
    pub members: BTreeSet<Identifier>,
    pub messages: MessageHistory,
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
    let mut chats = HashMap::<String, Chat>::new();
    // dm identifiers are ordered
    let mut dm_history = HashMap::<[Identifier; 2], MessageHistory>::new();

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
            match Message::recv(&mut clients[i].socket) {
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

                Ok(Some(msg)) => route_message(
                    &mut clients,
                    &mut users,
                    &mut chats,
                    &mut dm_history,
                    i,
                    msg,
                ),
            }
            i += 1;
        }
    }
    println!("shutting down");
}
