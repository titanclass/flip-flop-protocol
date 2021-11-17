use std::{env, error::Error, net::SocketAddr};

use flip_flop_app::{Command, Event};
use tokio::net::UdpSocket;

#[path = "../common/lib.rs"]
mod common;
use crate::common::{CommandId, EventId};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let local_addr: SocketAddr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".into())
        .parse()?;

    let socket = UdpSocket::bind(local_addr).await?;

    println!("SERVER: listening on {:?}", local_addr);

    // This size should never exceed what can be sent in one packet. If you
    // have needs that exceed this constraint then you will need to consider
    // framing.
    const MAX_DATAGRAM_SIZE: usize = 32;

    let mut recv_buf = [0; MAX_DATAGRAM_SIZE];

    loop {
        let (len, remote_addr) = socket.recv_from(&mut recv_buf).await?;
        if let Ok(command) = postcard::from_bytes::<Command<CommandId>>(&recv_buf[..len]) {
            println!(
                "SERVER: {:?} command received from {:?}. Replying.",
                command, remote_addr
            );

            let mut send_buf = [0; MAX_DATAGRAM_SIZE];
            let event = Event {
                id: EventId::SomeEvent,
                offset: command.last_event_offset.map(|o| o + 1).unwrap_or(0),
                time_delta: 1,
                data: b"event-data",
            };
            if let Ok(encoded_buf) = postcard::to_slice(&event, &mut send_buf) {
                let _ = socket.send_to(encoded_buf, remote_addr).await;
                println!("SERVER: {:?} event replied to {:?}", event, remote_addr);
            }
        }
    }
}
