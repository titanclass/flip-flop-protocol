use circular_queue::CircularQueue;
use rand::prelude::*;
use std::{env, error::Error, net::SocketAddr, time::Duration};

use flip_flop_app::{Command, Event};
use tokio::{
    net::UdpSocket,
    sync::mpsc,
    time::{self, Instant},
};

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

    // Generate events in the background. We simple generate timestamp
    // and send them to our main loop.

    let (event_s, mut event_r) = mpsc::channel::<Instant>(100);

    tokio::spawn(async move {
        loop {
            let delay = { Duration::from_secs(rand::thread_rng().gen_range(0..3)) };
            if let Some(instant) = Instant::now().checked_add(delay) {
                time::sleep_until(instant).await;
                let _ = event_s.send(instant).await;
            }
        }
    });

    // This size should never exceed what can be sent in one packet. If you
    // have needs that exceed this constraint then you will need to consider
    // framing.
    const MAX_DATAGRAM_SIZE: usize = 32;
    const MAX_EVENTS: usize = 10;

    let mut recv_buf = [0; MAX_DATAGRAM_SIZE];
    let mut events = CircularQueue::<Event<EventId>>::with_capacity(MAX_EVENTS);
    let mut event_offset = 0;

    loop {
        tokio::select! {
            Ok((len, remote_addr)) = socket.recv_from(&mut recv_buf) => {
                if let Ok(command) = postcard::from_bytes::<Command<CommandId>>(&recv_buf[..len]) {
                    println!(
                        "SERVER: {:?} command received from {:?}. Replying.",
                        command, remote_addr
                    );

                    // We optimise searching for events by going backward in our
                    // circular buffer until we find the latest event where its
                    // offset exceeds the last one observed by the client. In the
                    // case where we have no last offset expressed by the client
                    // then we provide the oldest one we have.
                    let maybe_event = match command.last_event_offset {
                        Some(last_event_offset) => events
                            .iter()
                            .take_while(|e| e.offset > last_event_offset)
                            .last()
                            .or(events.iter().last()),
                        None => events.iter().last(),
                    };

                    // It is quite plausible that we have no events. In this case we
                    // will not send anything back to the client. The client should
                    // always have a timeout strategy in place and move on in the case
                    // where no event is replied in relation to a command.
                    // If we do have an event then we reply it to the client.
                    if let Some(event) = maybe_event {
                        let mut send_buf = [0; MAX_DATAGRAM_SIZE];
                        if let Ok(encoded_buf) = postcard::to_slice(&event, &mut send_buf) {
                            let _ = socket.send_to(encoded_buf, remote_addr).await;
                            println!("SERVER: {:?} event replied to {:?}", event, remote_addr);
                        }
                    } else {
                        println!("SERVER: No event to reply to {:?}", remote_addr);
                    }
                }
            }

            Some(event_instant) = event_r.recv() => {
                let event = Event {
                    id: EventId::SomeEvent,
                    offset: event_offset,
                    delta_ticks: Instant::now().duration_since(event_instant).as_secs(),
                    data: b"event-data",
                };
                events.push(event);

                // For this example, we will reset the event offset periodically
                // so that a client can demonstrate how it forgets state.
                if rand::thread_rng().gen_range(0..10) == 0 {
                    event_offset = 0;
                } else {
                    event_offset += 1;
                }
            }
        }
    }
}
