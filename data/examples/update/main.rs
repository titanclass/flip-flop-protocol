use std::time::Duration;

use aead::KeyInit;
use aes::Aes128;
use ccm::aead::generic_array::GenericArray;
use ccm::aead::AeadInPlace;
use ccm::{
    consts::{U4, U7},
    Ccm,
};
use flip_flop_data::{
    discovery::{MIN_PACKET_SIZE, MIN_PAYLOAD_SIZE},
    from_datagram, to_datagram,
    update::{PrepareForUpdate, Update, UpdateKey, Version, UPDATE_BYTES_OVERHEAD},
    DataSource, Header,
};
use rand::RngCore;
use tokio::sync::broadcast;
use tokio::time;

type AesCcm = Ccm<Aes128, U4, U7>;

// Our software update bytes.
static UPDATE: [u8; 100 * 1024] = [0u8; 100 * 1024];

// The port that a server is associated with.
const MY_APP_PORT: u8 = 2;

// This would normally consider the time on wire for a request and the time taken
// for a server to process it. Consideration for replies is not required as they
// will be no reply.
const SERVER_REQUEST_RECEIVE_TIME: Duration = Duration::from_millis(12);

// The amount of time we must periodically wait for a server to do what it must
// to process the bytes we've sent. This period should include both the time
// taken to transmit the bytes and the time allowed for a server to do its thing
// e.g. write the bytes it has buffered into flash storage.
const UPDATE_PROCESSING_TIME: Duration = Duration::from_millis(100);

// The number of bytes that gets sent with each update.
const UPDATE_BYTES_SIZE: usize = MIN_PAYLOAD_SIZE - UPDATE_BYTES_OVERHEAD;

// The number of bytes sent before we must pause and give the
// servers more time to process. This value must not be exceeded.
const UPDATE_BYTES_PROCESSING_THRESHOLD: usize = 4096;

mod client {

    use super::*;

    pub async fn task(tx: &broadcast::Sender<[u8; MIN_PACKET_SIZE]>, servers: &[(u8, [u8; 16])]) {
        let mut datagram_buf = [0u8; MIN_PACKET_SIZE];
        let mut frame_counter = 0;

        let mut rng = rand::thread_rng();
        let mut update_key = [0; 16];
        rng.fill_bytes(&mut update_key);

        let update_len = UPDATE.len();

        prepare_servers_for_update(
            tx,
            servers,
            &update_key,
            update_len,
            &mut frame_counter,
            &mut datagram_buf,
        )
        .await;

        update_servers(
            tx,
            &update_key,
            update_len,
            &mut frame_counter,
            &mut datagram_buf,
        )
        .await;
    }

    async fn prepare_servers_for_update(
        tx: &broadcast::Sender<[u8; MIN_PACKET_SIZE]>,
        servers: &[(u8, [u8; 16])],
        update_key: &[u8; 16],
        update_len: usize,
        frame_counter: &mut u16,
        datagram_buf: &mut [u8; MIN_PACKET_SIZE],
    ) {
        for (server_address, server_network_key) in servers {
            let server_network_cipher = AesCcm::new(GenericArray::from_slice(server_network_key));

            let prepare_for_update = PrepareForUpdate {
                version: Version {
                    major: 1,
                    minor: 2,
                    patch: 3,
                    pre: None,
                },
                server_ports: 1 << MY_APP_PORT,
                update_key: UpdateKey(*update_key),
                update_byte_len: update_len as u32,
                signed: false,
            };

            create_prepare_update_request(
                &server_network_cipher,
                &prepare_for_update,
                *frame_counter,
                datagram_buf,
            );

            // We're sending to just one server, but ordinarily, many servers would receive it.
            if tx.send(*datagram_buf).is_ok() {
                println!("CLIENT {frame_counter}: sent prepare for update request to {server_address}. Waiting for the server to process.");
                *frame_counter = frame_counter.wrapping_add(1);

                // We provide each server with enough time to process along with the time it takes to send our bytes on the wire.
                time::sleep(SERVER_REQUEST_RECEIVE_TIME).await;
            }
        }
    }

    fn create_prepare_update_request(
        network_cipher: &impl AeadInPlace,
        prepare_for_update: &PrepareForUpdate,
        frame_counter: u16,
        datagram_buf: &mut [u8; MIN_PACKET_SIZE],
    ) {
        let header = Header {
            version: 0,
            source: DataSource::Client,
            server_address: 0,
            server_port: 1,
            frame_counter,
        };

        to_datagram(
            network_cipher,
            &header,
            &postcard::to_vec::<PrepareForUpdate, MIN_PAYLOAD_SIZE>(prepare_for_update).unwrap(),
            datagram_buf,
        );
    }

    async fn update_servers(
        tx: &broadcast::Sender<[u8; MIN_PACKET_SIZE]>,
        update_key: &[u8; 16],
        update_len: usize,
        frame_counter: &mut u16,
        datagram_buf: &mut [u8; MIN_PACKET_SIZE],
    ) {
        let update_cipher = AesCcm::new(GenericArray::from_slice(update_key));

        let mut update_byte_offset = 0;
        let mut next_threshold_byte_offset = UPDATE_BYTES_PROCESSING_THRESHOLD.min(update_len);

        while update_byte_offset < UPDATE.len() {
            let to_update_byte_offset =
                (update_byte_offset + UPDATE_BYTES_SIZE).min(next_threshold_byte_offset);
            let update_bytes = &UPDATE[update_byte_offset..to_update_byte_offset];

            let update: Update<UPDATE_BYTES_SIZE> = Update {
                byte_offset: update_byte_offset as u32,
                bytes: heapless::Vec::from_slice(update_bytes).unwrap(),
            };

            create_update_request(&update_cipher, &update, *frame_counter, datagram_buf);

            if tx.send(*datagram_buf).is_err() {
                return;
            }

            let delay = if to_update_byte_offset == next_threshold_byte_offset {
                next_threshold_byte_offset += UPDATE_BYTES_PROCESSING_THRESHOLD;
                next_threshold_byte_offset = next_threshold_byte_offset.min(update_len);
                UPDATE_PROCESSING_TIME
            } else {
                SERVER_REQUEST_RECEIVE_TIME
            };

            println!("CLIENT {frame_counter}: sent update with offset {update_byte_offset} with len {}. Waiting {:?} for the server to process.", update_bytes.len(), delay);
            time::sleep(delay).await;

            *frame_counter = frame_counter.wrapping_add(1);

            update_byte_offset = to_update_byte_offset;
        }

        println!(
            "CLIENT {frame_counter}: sent updates Waiting {:?} for the server to process.",
            UPDATE_PROCESSING_TIME
        );
        time::sleep(UPDATE_PROCESSING_TIME).await;
    }

    fn create_update_request<const N: usize>(
        update_cipher: &impl AeadInPlace,
        update: &Update<N>,
        frame_counter: u16,
        datagram_buf: &mut [u8; MIN_PACKET_SIZE],
    ) {
        let header = Header {
            version: 0,
            source: DataSource::Client,
            server_address: 0,
            server_port: 1,
            frame_counter,
        };

        to_datagram(
            update_cipher,
            &header,
            &postcard::to_vec::<Update<N>, MIN_PAYLOAD_SIZE>(update).unwrap(),
            datagram_buf,
        );
    }
}

mod server {

    use super::*;

    struct UpdateInfo {
        cipher: AesCcm,
        byte_len: usize,
        next_byte_offset: usize,
    }

    pub async fn task(tx: broadcast::Sender<[u8; MIN_PACKET_SIZE]>, server: &(u8, [u8; 16])) {
        let mut rx = tx.subscribe();

        let (_server_address, server_network_key) = server;
        let server_cipher = AesCcm::new(GenericArray::from_slice(server_network_key));

        let current_version = "1.2.0".parse::<Version>().unwrap();
        let mut active_update_info: Option<UpdateInfo> = None;

        while let Ok(encrypted_payload) = rx.recv().await {
            // First try processing an update request for an active update
            if let Some((update_info, update)) = if let Some(update_info) = &mut active_update_info
            {
                process_client_update_request::<UPDATE_BYTES_SIZE>(
                    &update_info.cipher,
                    &encrypted_payload,
                )
                .map(|update| (update_info, update))
            } else {
                None
            } {
                if process_active_update(update_info, &update) {
                    active_update_info = None;
                    continue;
                }

            // If we're not processing an active update then try handling the
            // request as one that prepares us for a new update.
            } else if let Some(prepare_for_update) =
                process_client_prepare_for_update_request(&server_cipher, &encrypted_payload)
            {
                if let Some(new_active_update_info) =
                    activate_update_if_new_version(&prepare_for_update, &current_version)
                {
                    active_update_info = Some(new_active_update_info);
                }
            }
        }
    }

    fn process_client_update_request<const N: usize>(
        cipher: &AesCcm,
        datagram_buf: &[u8; MIN_PACKET_SIZE],
    ) -> Option<Update<N>> {
        from_datagram(
            datagram_buf,
            |h| h.server_address == 0x00 && h.server_port == 0x01 && h.source == DataSource::Client,
            cipher,
        )
        .ok()
        .and_then(|(_, b)| postcard::from_bytes::<Update<N>>(&b).ok())
    }

    fn process_active_update<const N: usize>(
        update_info: &mut UpdateInfo,
        update: &Update<N>,
    ) -> bool {
        // Bail out if the next byte offset isn't what we expect.
        if update.byte_offset as usize != update_info.next_byte_offset {
            println!(
                "SERVER: abandoning update given unexpected offset {} with len {}.",
                update.byte_offset,
                update.bytes.len()
            );
            return true;
        }

        println!(
            "SERVER: received update with offset {} with len {}.",
            update.byte_offset,
            update.bytes.len()
        );

        update_info.next_byte_offset += update.bytes.len();

        if update_info.next_byte_offset == update_info.byte_len {
            println!("SERVER: Doing something heavy with the last bytes of our buffer e.g. flashing memory with firmware.");

            println!("SERVER: {} bytes received. Update finished. Do something heavy again e.g. update firmware.", update_info.next_byte_offset);
            true
        } else if update_info.next_byte_offset % UPDATE_BYTES_PROCESSING_THRESHOLD == 0 {
            println!(
                "SERVER: Doing something heavy with our buffer e.g. flashing memory with firmware."
            );
            false
        } else {
            false
        }
    }

    fn process_client_prepare_for_update_request(
        cipher: &AesCcm,
        datagram_buf: &[u8; MIN_PACKET_SIZE],
    ) -> Option<PrepareForUpdate> {
        from_datagram(
            datagram_buf,
            |h| h.server_address == 0x00 && h.server_port == 0x01 && h.source == DataSource::Client,
            cipher,
        )
        .ok()
        .and_then(|(_, b)| postcard::from_bytes::<PrepareForUpdate>(&b).ok())
    }

    fn activate_update_if_new_version(
        prepare_for_update: &PrepareForUpdate,
        current_version: &Version,
    ) -> Option<UpdateInfo> {
        // Is this a version we're interested in? If so then note something about it.
        if &prepare_for_update.version > current_version {
            println!(
                "SERVER: updating from {:?} to {:?}.",
                current_version, prepare_for_update.version
            );

            Some(UpdateInfo {
                cipher: AesCcm::new(GenericArray::from_slice(&prepare_for_update.update_key.0)),
                byte_len: prepare_for_update.update_byte_len as usize,
                next_byte_offset: 0,
            })
        } else {
            None
        }
    }
}

#[tokio::main]
async fn main() {
    // Both the client and server share a private key. Each server in a network
    // should have its own private key.
    let mut rng = rand::thread_rng();
    let mut server_network_key = [0; 16];
    rng.fill_bytes(&mut server_network_key);

    let servers = vec![(1, server_network_key)];

    let (tx, _rx) = broadcast::channel(256);

    let task_tx = tx.clone();
    let task_servers = servers.clone();
    tokio::spawn(async move {
        server::task(task_tx.clone(), &task_servers[0]).await;
    });

    client::task(&tx, &servers).await;
}
