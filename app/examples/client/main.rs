use std::{env, error::Error, net::SocketAddr, sync::Arc, time::Duration};

use chrono::Local;
use flip_flop_app::{CommandRequest, EventReply, Logged};
use tokio::{
    net::UdpSocket,
    time::{self, Instant},
};

#[path = "../common/lib.rs"]
mod common;
use crate::common::{Command, Event};

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

    let mut last_event_offset = 0_u32;
    let mut event_count = 0_u32;

    println!("CLIENT: listening on {:?}", local_addr);

    let mut next_send_time = Instant::now();

    let mut init_mode = true;

    loop {
        // Wake at a regular interval which is what we need to do
        // to cycle predictably through our servers when operating in
        // half duplex mode such that they all get some airtime.
        time::sleep_until(next_send_time).await;

        // Tell the server to do something and let it know what we know
        // of its state by communicating the last event offset we received
        // for it.
        let mut send_buf = [0; MAX_DATAGRAM_SIZE];
        let command = if init_mode {
            None
        } else {
            Some(Command::SomeCommand)
        };
        let request = CommandRequest {
            last_event_offset,
            command,
        };
        if let Ok(encoded_buf) = postcard::to_slice(&request, &mut send_buf) {
            let _ = s.send_to(encoded_buf, remote_addr).await;
            println!("CLIENT: {:?} command sent to {:?}", request, remote_addr);
        }

        // Receive an event from the server. If we don't get anything within
        // a short timeout then we move on.
        let mut recv_buf = [0; MAX_DATAGRAM_SIZE];
        if let Ok(Ok((len, remote_addr))) =
            time::timeout(Duration::from_millis(100), r.recv_from(&mut recv_buf)).await
        {
            if let Ok(reply) = postcard::from_bytes::<EventReply<Logged<Event>>>(&recv_buf[..len]) {
                if let Some(local_time) = Local::now().checked_sub_signed(
                    chrono::Duration::from_std(Duration::from_secs(reply.delta_ticks))
                        .unwrap_or(chrono::Duration::seconds(0)),
                ) {
                    println!(
                        "CLIENT: event time {:?} {:?} event {} received from {:?}",
                        local_time, reply, event_count, remote_addr
                    );
                }
                let expected_event_offset = last_event_offset.wrapping_add(1);
                match reply.event {
                    Some(Logged(_, offset)) if offset != expected_event_offset => {
                        init_mode = true;
                        event_count = 0;
                        last_event_offset = offset;
                        println!("CLIENT: Previous events for this server are now forgotten given an offset != what we expected");
                    }
                    Some(Logged(_, offset)) => {
                        event_count = event_count.wrapping_add(1);
                        last_event_offset = offset;
                    }
                    None => init_mode = false,
                }
            }
        }

        next_send_time += Duration::from_secs(1); // Bit of a problem when we reach the end of time... ;-)
    }
}
