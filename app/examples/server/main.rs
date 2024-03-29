use circular_queue::CircularQueue;
use rand::prelude::*;
use std::{env, error::Error, net::SocketAddr, time::Duration};

use flip_flop_app::{CommandRequest, EventOf, NoEE};
use tokio::{
    net::UdpSocket,
    sync::mpsc,
    time::{self, Instant},
};

#[path = "../common/lib.rs"]
mod common;
use crate::common::{Command, Event};

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
            let event_time = Instant::now();
            let delay = { Duration::from_secs(rand::thread_rng().gen_range(0..3)) };
            if let Some(instant) = event_time.checked_add(delay) {
                time::sleep_until(instant).await;
                let _ = event_s.send(event_time).await;
            }
        }
    });

    // This size should never exceed what can be sent in one packet. If you
    // have needs that exceed this constraint then you will need to consider
    // framing.
    const MAX_DATAGRAM_SIZE: usize = 32;
    const MAX_EVENTS: usize = 10;

    let mut recv_buf = [0; MAX_DATAGRAM_SIZE];
    let mut events = CircularQueue::<(Event, u32, Instant)>::with_capacity(MAX_EVENTS);
    let mut start: Option<u32> = None;
    let mut end: Option<u32> = None;

    // Randomise the starting offset to increase the probably of a client
    // detecting that a server has started up.
    let mut event_offset = rand::thread_rng().gen_range(0..MAX_EVENTS) as u32;

    loop {
        tokio::select! {
            Ok((len, remote_addr)) = socket.recv_from(&mut recv_buf) => {
                if let Ok(request) = postcard::from_bytes::<CommandRequest<Command>>(&recv_buf[..len]) {
                    println!(
                        "SERVER: {:?} command received from {:?}. Replying.",
                        request, remote_addr
                    );

                    // We optimise searching for events by going backward in our
                    // circular buffer until we find the latest event where its
                    // offset is adjacent to the last one observed by the client. In the
                    // case where we have nothing in relation to the last offset expressed
                    // by the client then we provide the oldest one we have. See the
                    // offset-rules.md doc for details.
                    let maybe_event: Option<(EventOf<Event, NoEE>, Instant)> = if let (Some(start), Some(end)) = (start, end) {
                        if request.last_event_offset.is_none() || (request.last_event_offset >= Some(start) && request.last_event_offset <= Some(end)) {
                            let next_event_offset = request.last_event_offset.map(|o| o.wrapping_add(1));
                            let mut events_iter =
                                events
                                .iter()
                                .skip_while(|(_, o, _)| Some(*o) != next_event_offset && Some(*o) != request.last_event_offset);
                            let next_e = events_iter.next();
                            let last_e = events_iter.next();
                            let e = match (next_e, last_e) {
                                (Some((_, o, _)), _) if Some(*o) == next_event_offset => next_e.cloned(),
                                (_, Some((_, o, _))) if Some(*o) == request.last_event_offset => None,
                                (Some((_, o, _)), _) if Some(*o) == request.last_event_offset => None,
                                _ => events.iter().last().cloned(),
                            };
                            e.map(|(e, o, t)|(EventOf::Logged(e, o), t))
                        } else {
                            Some((EventOf::Recovery(start, end), Instant::now()))
                        }
                    } else {
                        None
                    };

                    let reply = flip_flop_app::event_reply(maybe_event, |t|Instant::now().duration_since(t).as_secs());

                    let mut send_buf = [0; MAX_DATAGRAM_SIZE];
                    if let Ok(encoded_buf) = postcard::to_slice(&reply, &mut send_buf) {
                        let _ = socket.send_to(encoded_buf, remote_addr).await;
                        println!("SERVER: {:?} event replied to {:?}", reply, remote_addr);
                    }
                }
            }

            Some(event_instant) = event_r.recv() => {
                // For this example, we will reset the event offset periodically
                // so that a client can demonstrate how it forgets state.
                if rand::thread_rng().gen_range(0..40) == 0 {
                    println!("SERVER: Resetting events");
                    events.clear();
                    event_offset = rand::thread_rng().gen_range(0..MAX_EVENTS) as u32;
                    start = Some(event_offset);
                    end = Some(event_offset);
                } else {
                    println!("SERVER: event stored for offset {}", event_offset);
                    events.push((Event::SomeEvent, event_offset, event_instant));
                    if start.is_none() {
                        start = Some(event_offset);
                    }
                    end = Some(event_offset);
                    event_offset = event_offset.wrapping_add(1);
                }
            }
        }
    }
}
