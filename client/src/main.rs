#![warn(clippy::undocumented_unsafe_blocks)]

use std::net::TcpStream;
use terminal_chat::{ADDRESS, Message};

fn display_message(msg: &Message) {
    println!("{}", msg.text);
    for image in &msg.images {
        println!("image: {}", image.alt_text);
    }
}

fn main() {
    let mut stream = TcpStream::connect(ADDRESS).expect("failed to create client");

    Message {
        text: "ping".to_string(),
        images: Vec::new(),
    }
    .write_to(&mut stream)
    .expect("failed to send message");

    match Message::read_from(&mut stream) {
        Err(e) => panic!("failed to read message from server: {e}"),
        Ok(None) => println!("server responded without a message"),
        Ok(Some(msg)) => {
            println!("message from server {ADDRESS}:\n```");
            display_message(&msg);
            println!("```");
        }
    }
}
