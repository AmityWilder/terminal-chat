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

macro_rules! err_response {
    (($err:expr) -> $socket:expr) => {
        response!((Message::Error($err)) -> $socket)
    };
}

#[derive(Debug)]
struct Client {
    pub username: Option<Username>,
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
    users: &mut HashMap<Username, String>,
    chats: &mut HashMap<ChatName, Chat>,
    dm_history: &mut HashMap<[Identifier; 2], MessageHistory>,
    sender_index: usize,
    msg: Message,
) {
    match msg {
        Message::User(mut umsg) => {
            response!((Message::Acknowledge) -> &mut clients[sender_index].socket);
            umsg.sender = Some(clients[sender_index].identifier());
            // sanitization
            for attachment in &mut umsg.attachments {
                attachment.filename = attachment.filename.replace(char::is_whitespace, "_");
            }
            match &umsg.destination {
                // global chat
                Destination::Chat(chat) if chat.is_empty() => {
                    // messages.push(umsg.clone()); // todo: global chat history?
                    println!("distributing message:\n```\n{umsg:?}\n```");
                    let msg = Message::User(umsg);
                    for (_, client) in clients
                        .iter_mut()
                        .enumerate()
                        .filter(|(i, _)| *i != sender_index)
                    {
                        let recipient = client.identifier();
                        match msg.clone().send(&mut client.socket) {
                            Ok(()) => println!("sent message to client `{recipient}`"),
                            Err(e) => {
                                eprintln!("failed to send message to client `{recipient}`: {e}")
                            }
                        }
                    }
                }

                Destination::Chat(chat) => match chats.get_mut(chat) {
                    None => {
                        eprintln!("chat `{chat:?}` does not exist");
                        err_response!((MessageError::DstNexists) -> &mut clients[sender_index].socket);
                    }

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
                        response!((Message::Success) -> &mut clients[sender_index].socket);
                    }
                },

                Destination::Client(identifier) => {
                    match dmid(clients[sender_index].identifier(), identifier.clone()) {
                        Err(e) => {
                            eprintln!("could not get user pair: {e}");
                            err_response!((e) -> &mut clients[sender_index].socket);
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

        Message::Acknowledge => println!("acknowledged"),
        Message::Success => println!("success"),
        Message::Error(e) => eprintln!("error: {e}"),

        Message::CreateChat {
            destination,
            mut members,
        } => {
            println!("creating chat...");
            match chats.entry(destination.clone()) {
                Entry::Occupied(_) => {
                    eprintln!("a chat with this name already exists");
                    err_response!((MessageError::ChatTaken) -> &mut clients[sender_index].socket);
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
                    response!((Message::Success) -> &mut clients[sender_index].socket);
                }
            }
        }

        Message::Login { username, password } => {
            if username.contains(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')) {
                println!("invalid username");
                err_response!((MessageError::BadUsername) -> &mut clients[sender_index].socket);
            } else {
                match users.entry(username.clone()) {
                    Entry::Occupied(entry) => {
                        if entry.get() == &password {
                            clients[sender_index].username = Some(username);
                            println!("logged into existing user");
                            response!((Message::Success) -> &mut clients[sender_index].socket);
                        } else {
                            println!("incorrect password");
                            err_response!((MessageError::WrongPassword) -> &mut clients[sender_index].socket);
                        }
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(password);
                        clients[sender_index].username = Some(username);
                        println!("logged into new user");
                        response!((Message::Success) -> &mut clients[sender_index].socket);
                    }
                }
            }
        }

        Message::Get {
            source,
            range: (start, end),
        } => {
            let history = match &source {
                Destination::Chat(chat) => chats.get(chat).map(|chat| &chat.messages),

                Destination::Client(identifier) => {
                    match dmid(clients[sender_index].identifier(), identifier.clone()) {
                        Err(e) => {
                            eprintln!("could not get user pair: {e}");
                            err_response!((e) -> &mut clients[sender_index].socket);
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
                response!((Message::GetResponse(messages)) -> &mut clients[sender_index].socket);
            } else {
                match &source {
                    Destination::Chat(_) => {
                        err_response!((MessageError::DstNexists) -> &mut clients[sender_index].socket)
                    }
                    Destination::Client(_) => {
                        response!((Message::Success) -> &mut clients[sender_index].socket)
                    }
                }
            }
        }

        Message::GetResponse(_) => {
            eprintln!("invlalid: server did not request messages");
        }

        Message::ModifyChatMembers {
            remove,
            chat,
            mut members,
        } => match chats.get_mut(&chat) {
            Some(chat) => {
                if remove {
                    chat.members = &chat.members - &members; // set difference
                } else {
                    chat.members.append(&mut members); // set union
                }
            }

            None => {
                err_response!((MessageError::DstNexists) -> &mut clients[sender_index].socket)
            }
        },
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

    let target = std::env::args()
        .nth(1)
        .and_then(|x| x.parse().ok())
        .unwrap_or(ADDRESS);
    let listener = TcpListener::bind(target).expect("failed to create server");
    listener
        .set_nonblocking(true)
        .expect("cannot set nonblocking");

    let mut clients = Vec::new();
    let mut users = HashMap::<Username, String>::new();
    // TODO: chats should probably have some way of locking out non-members
    let mut chats = HashMap::<ChatName, Chat>::new();
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
