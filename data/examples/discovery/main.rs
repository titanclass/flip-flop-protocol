use std::time::Duration;

use aead::KeyInit;
use aes::Aes128;
use ccm::aead::generic_array::GenericArray;
use ccm::aead::AeadInPlace;
use ccm::{
    consts::{U4, U7},
    Ccm,
};
use flip_flop_data::discovery::{
    Identified, Identify, MAX_ADDRESSES, MIN_PACKET_SIZE, MIN_PAYLOAD_SIZE,
};
use flip_flop_data::{from_datagram, to_datagram, DataSource, Header};
use futures::future;
use tokio::sync::broadcast;
use tokio::time;

type AesCcm = Ccm<Aes128, U4, U7>;

const CLIENT_TIME_WINDOW: Duration = Duration::from_millis(1000);
const SERVER_REPLY_WINDOW: Duration = Duration::from_millis(900);

mod client {

    use super::*;

    pub async fn task(
        tx: &broadcast::Sender<[u8; MIN_PACKET_SIZE]>,
        identify: &mut Identify,
        frame_counter: u16,
    ) -> bool {
        let mut finished = true;

        let key = GenericArray::from_slice(b"0000000000000000");
        let cipher = AesCcm::new(key);

        let mut datagram_buf = [0u8; MIN_PACKET_SIZE];

        create_client_request(&cipher, identify, frame_counter, &mut datagram_buf);
        if tx.send(datagram_buf).is_ok() {
            println!("CLIENT {frame_counter}: sent identify request. Waiting one second for all replies.");

            let mut rx = tx.subscribe();
            let time_window = time::timeout(CLIENT_TIME_WINDOW, future::pending::<()>());
            tokio::pin!(time_window);

            let mut addresses = [0; MAX_ADDRESSES];
            loop {
                tokio::select! {
                    r = rx.recv() => if let Ok(encrypted_payload) = r {
                        if let Some(identified) = process_server_reply(&cipher, &encrypted_payload) {
                            addresses[identified.server_address as usize] += 1;
                        } else {
                            finished = false;
                        }
                    } else {
                        break
                    },
                    _ = &mut time_window => {
                        println!("CLIENT {frame_counter}: time window finished.");
                        break;
                    }
                }
            }
            for (address, count) in addresses.iter().enumerate() {
                let count = *count;
                if count == 1 {
                    identify.set_address(address as u8);
                } else if count > 1 {
                    finished = false;
                }
            }
            let found = identify.iter().fold(0, |mut a, e| {
                if e {
                    a += 1;
                    a
                } else {
                    a
                }
            }) - 1;
            println!("CLIENT {frame_counter}: Found: {found}.");
        }

        finished
    }

    fn create_client_request(
        cipher: &impl AeadInPlace,
        identify: &Identify,
        frame_counter: u16,
        datagram_buf: &mut [u8; MIN_PACKET_SIZE],
    ) {
        let header = Header {
            version: 0,
            source: DataSource::Client,
            server_address: 0,
            server_port: 0,
            frame_counter,
        };

        to_datagram(
            cipher,
            &header,
            &postcard::to_vec::<Identify, MIN_PAYLOAD_SIZE>(identify).unwrap(),
            datagram_buf,
        );
    }

    fn process_server_reply(
        cipher: &impl AeadInPlace,
        datagram_buf: &[u8; MIN_PACKET_SIZE],
    ) -> Option<Identified> {
        from_datagram(
            datagram_buf,
            |h| h.server_address == 0x00 && h.server_port == 0x00 && h.source == DataSource::Server,
            cipher,
        )
        .and_then(|(_, b)| postcard::from_bytes::<Identified>(&b).ok())
    }
}

mod server {
    use super::*;

    pub async fn task(
        tx: broadcast::Sender<[u8; MIN_PACKET_SIZE]>,
        frame_counter: u16,
        server_address: &mut Option<u8>,
    ) {
        let key = GenericArray::from_slice(b"0000000000000000");
        let cipher = AesCcm::new(key);

        let mut datagram_buf = [0u8; MIN_PACKET_SIZE];

        let mut rx = tx.subscribe();
        if let Ok(encrypted_payload) = rx.recv().await {
            if let Some(identify) = process_client_request(&cipher, &encrypted_payload) {
                if server_address
                    .map(|sa| !identify.is_address_set(sa))
                    .unwrap_or(true)
                {
                    if let Some(identified) = try_create_server_reply(
                        &cipher,
                        &identify,
                        frame_counter,
                        &mut datagram_buf,
                    ) {
                        *server_address = Some(identified.server_address);
                        time::sleep(SERVER_REPLY_WINDOW).await;
                        let _ = tx.send(datagram_buf);
                    }
                }
            }
        }
    }

    fn process_client_request(
        cipher: &AesCcm,
        datagram_buf: &[u8; MIN_PACKET_SIZE],
    ) -> Option<Identify> {
        from_datagram(
            datagram_buf,
            |h| h.server_address == 0x00 && h.server_port == 0x00 && h.source == DataSource::Client,
            cipher,
        )
        .and_then(|(_, b)| postcard::from_bytes::<Identify>(&b).ok())
    }

    fn try_create_server_reply(
        cipher: &AesCcm,
        identify: &Identify,
        frame_counter: u16,
        datagram_buf: &mut [u8; MIN_PACKET_SIZE],
    ) -> Option<Identified> {
        if let Some(identified) =
            Identified::with_random_address(identify.iter(), &mut rand::thread_rng(), 0b00000010)
        {
            let header = Header {
                version: 0,
                source: DataSource::Server,
                server_address: 0,
                server_port: 0,
                frame_counter,
            };

            to_datagram(
                cipher,
                &header,
                &postcard::to_vec::<Identified, MIN_PAYLOAD_SIZE>(&identified).unwrap(),
                datagram_buf,
            );

            Some(identified)
        } else {
            None
        }
    }
}

#[tokio::main]
async fn main() {
    let (tx, _rx) = broadcast::channel(256);

    for _ in 0..255 {
        let task_tx = tx.clone();
        tokio::spawn(async move {
            let mut frame_counter = 0u16;
            let mut server_address = None;
            loop {
                server::task(task_tx.clone(), frame_counter, &mut server_address).await;
                frame_counter = frame_counter.wrapping_add(1);
            }
        });
    }

    let addresses = [0; MIN_PAYLOAD_SIZE];
    let mut identify = Identify { addresses };
    identify.set_address(0); // Address 0 is the client and is therefore reserved
    let mut frame_counter = 0u16;
    loop {
        if client::task(&tx, &mut identify, frame_counter).await {
            break;
        }
        frame_counter = frame_counter.wrapping_add(1);
    }

    println!("Finished in {} seconds", frame_counter + 1);
}
