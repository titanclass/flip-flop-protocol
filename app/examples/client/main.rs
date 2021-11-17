use std::{env, error::Error, net::SocketAddr, sync::Arc};

use flip_flop_app::{Command, Event};
use tokio::net::UdpSocket;

#[path = "../common/lib.rs"]
mod common;
use crate::common::{CommandId, EventId};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let remote_addr: SocketAddr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".into())
        .parse()?;

    let local_addr: SocketAddr = if remote_addr.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    }
    .parse()?;

    let socket = UdpSocket::bind(local_addr).await?;

    let r = Arc::new(socket);
    let s = r.clone();

    // This size should never exceed what can be sent in one packet. If you
    // have needs that exceed this constraint then you will need to consider
    // framing.
    const MAX_DATAGRAM_SIZE: usize = 32;

    tokio::spawn(async move {
        let mut send_buf = [0; MAX_DATAGRAM_SIZE];
        let command = Command {
            id: CommandId::SomeCommand,
            data: b"command-data",
            last_event_offset: None,
        };
        if let Ok(_) = postcard::to_slice(&command, &mut send_buf) {
            let _ = s.send_to(&send_buf, remote_addr).await;
            println!("CLIENT: {:?} command sent to {:?}", command, remote_addr);
        }
    });

    println!("CLIENT: listening on {:?}", local_addr);

    let mut recv_buf = [0; MAX_DATAGRAM_SIZE];

    loop {
        let (len, remote_addr) = r.recv_from(&mut recv_buf).await?;
        if let Ok(event) = postcard::from_bytes::<Event<EventId>>(&recv_buf[..len]) {
            println!("CLIENT: {:?} event received from {:?}", event, remote_addr);
        }
    }
}
