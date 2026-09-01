#![warn(clippy::undocumented_unsafe_blocks)]

use std::{
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    sync::{Arc, Mutex},
    thread,
};

type ImageByteCount = u16;
pub const MAX_IMAGE_BYTES: usize = 2048;
const _: () = {
    assert!(MAX_IMAGE_BYTES.ilog2() <= ImageByteCount::BITS);
};

/// TODO
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Image {
    /// At most [`MAX_IMAGE_BYTES`] bytes
    pub data: Vec<u8>,
}

type TextByteCount = u16;
pub const MAX_TEXT_BYTES: usize = 2048;
const _: () = {
    assert!(MAX_TEXT_BYTES.ilog2() <= TextByteCount::BITS);
};

type ImageCount = u8;
pub const MAX_IMAGES: usize = 8;
const _: () = {
    assert!(MAX_IMAGES.ilog2() <= ImageCount::BITS);
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Message {
    /// At most [`MAX_TEXT_BYTES`] bytes
    pub text: String,
    /// At most [`MAX_IMAGES`] images
    pub images: Vec<Image>,
}

impl Message {
    pub fn write_to<W: Write>(&self, stream: &mut W) -> io::Result<()> {
        // header
        stream.write_all(
            &TextByteCount::try_from(self.text.len())
                .unwrap()
                .to_le_bytes(),
        )?;
        stream.write_all(
            &ImageCount::try_from(self.images.len())
                .unwrap()
                .to_le_bytes(),
        )?;
        for image in &self.images {
            stream.write_all(
                &ImageByteCount::try_from(image.data.len())
                    .unwrap()
                    .to_le_bytes(),
            )?;
        }
        // content
        stream.write_all(self.text.as_bytes())?;
        for image in &self.images {
            stream.write_all(&image.data)?;
        }
        Ok(())
    }

    pub fn read_from<R: Read>(stream: &mut R) -> io::Result<Option<Self>> {
        // header
        let mut text_len = [0; _];
        let n = stream.read(&mut text_len)?;
        if n == 0 {
            return Ok(None);
        } else if n != text_len.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "failed to fill whole buffer",
            ));
        }
        let text_len = TextByteCount::from_le_bytes(text_len);
        let mut image_count = [0; _];
        stream.read_exact(&mut image_count)?;
        let image_count = ImageCount::from_le_bytes(image_count);
        let mut image_sizes = [[0; _]; 8];
        for image_size in &mut image_sizes[..image_count as usize] {
            stream.read_exact(image_size)?;
        }
        let image_sizes: [_; 8] =
            std::array::from_fn(|i| ImageByteCount::from_le_bytes(image_sizes[i]));
        // content
        Ok(Some(Self {
            text: {
                let mut text = vec![0; text_len as usize];
                stream.read_exact(&mut text)?;
                String::from_utf8(text).map_err(io::Error::other)?
            },
            images: image_sizes
                .into_iter()
                .take(image_count as usize)
                .map(|image_size| {
                    let mut data = vec![0; image_size as usize];
                    stream.read_exact(&mut data)?;
                    Ok(Image { data })
                })
                .collect::<io::Result<Vec<_>>>()?,
        }))
    }
}

#[derive(Debug)]
pub struct Server {
    listener: TcpListener,
}

impl Server {
    pub fn new<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        TcpListener::bind(addr).map(|listener| Self { listener })
    }

    pub fn run(&self) {
        let clients = Mutex::new(Vec::new());
        thread::scope(|s| {
            thread::Builder::new()
                .name("server connection obtainer".to_string())
                .spawn_scoped(s, || {
                    loop {
                        println!("awaiting client connect...");
                        match self.listener.accept() {
                            Err(e) => eprintln!("failed to connect: {e}"),
                            Ok(client) => {
                                clients.lock().expect("client list poisoned").push(client);
                            }
                        }
                    }
                })
                .expect("failed to spawn thread");

            thread::Builder::new()
                .name("server connection handler".to_string())
                .spawn_scoped(s, || {
                    loop {
                        let mut clients = clients.lock().expect("client list poisoned");
                        let mut i = 0;
                        while i < clients.len() {
                            let (socket, addr) = &mut clients[i];
                            match Message::read_from(socket) {
                                Err(e) => {
                                    eprintln!("failed to read message from client {addr}: {e}");
                                }

                                Ok(None) => {
                                    println!("client {addr} disconnected");
                                    clients.swap_remove(i);
                                    continue; // `i` now refers to a different client
                                }

                                Ok(Some(msg)) => {
                                    println!("message from client {addr}:\n```\n{msg:?}\n```");
                                }
                            }
                            i += 1;
                        }
                    }
                })
                .expect("failed to spawn thread");
        })
    }
}

#[derive(Debug)]
pub struct Client {
    stream: TcpStream,
}

impl Client {
    pub fn new<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        TcpStream::connect(addr).map(|stream| Self { stream })
    }

    pub fn send_message(&mut self, message: Message) -> io::Result<()> {
        message.write_to(&mut self.stream)
    }

    pub fn read_message(&mut self) -> io::Result<Option<Message>> {
        Message::read_from(&mut self.stream)
    }
}

fn main() {
    const ADDRESS: SocketAddr =
        SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

    thread::scope(|s| {
        thread::Builder::new()
            .name("server".to_string())
            .spawn_scoped(s, || {
                Server::new(ADDRESS).expect("failed to create server").run()
            })
            .expect("failed to spawn thread");

        // client 1
        thread::Builder::new()
            .name("client 1".to_string())
            .spawn_scoped(s, || {
                let mut client = Client::new(ADDRESS).expect("failed to create client");
                client
                    .send_message(Message {
                        text: "test".to_string(),
                        images: Vec::new(),
                    })
                    .expect("failed to send message");
            })
            .expect("failed to spawn thread");

        // client 2
        thread::Builder::new()
            .name("client 2".to_string())
            .spawn_scoped(s, || {
                let mut client = Client::new(ADDRESS).expect("failed to create client");
                client
                    .send_message(Message {
                        text: "ping".to_string(),
                        images: Vec::new(),
                    })
                    .expect("failed to send message");
            })
            .expect("failed to spawn thread");
    });
    println!("finished");
}
