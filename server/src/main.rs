#![warn(clippy::undocumented_unsafe_blocks)]

use std::{io, net::TcpListener};
use terminal_chat::{ADDRESS, Message};

fn main() {
    let listener = TcpListener::bind(ADDRESS).expect("failed to create server");
    listener
        .set_nonblocking(true)
        .expect("cannot set nonblocking");

    let mut clients = Vec::new();

    println!("awaiting client connect...");
    loop {
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
                        eprintln!("failed to read message from client {addr}: {e}")
                    }
                }

                Ok(None) => {
                    println!("client {addr} disconnected");
                    clients.swap_remove(i);
                    continue; // `i` now refers to a different client
                }

                Ok(Some(msg)) => {
                    println!("message from client {addr}:\n```\n{msg:?}\n```");
                    Message {
                        text: "acknowledged".to_string(),
                        images: Vec::new(),
                    }
                    .write_to(socket)
                    .expect("failed to send response to client");
                }
            }
            i += 1;
        }
    }
}
