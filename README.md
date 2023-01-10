# flip-flop-protocol

*A single-client/multi-server event-bus protocol suitable for half-duplex communications.*

Flip-flop is an application layer protocol optimised for half-duplex communication where a single client may monitor and control a number of servers. It can be used in an industrial control setting where it is more flexible than Modbus and similar protocols.  

Flip-flop is flexible because it uses a vocabulary of _commands_ and _events_ specific to each application.  

## Protocol Operation

Protocol operation consists of a series of message exchanges driven by the client.  In the common case the client sends a _command_ to a server which responds with an _event_.  Unlike synchronous request/reply protocols, the command and event may be unrelated and either or both can be omitted from an exchange.  

If the command is omitted the message exchange is termed a _poll_ and the server returns a pending event, if any.

A command instructs a server to do something, typically resulting in an event.  An event indicates a change of state in a server which could also be caused by an input, a timer or some other effect on the server. In a data acquisition application events could be frequent and commands infrequent.  

Command delivery is 'best effort'.   If the transport indicates an error then the client cannot assume the command was or was not delivered.  However the client can ascertain the state of the server and recover in an application specific way.

Event delivery is reliable in the face of transport errors. Other failures, such as a server restart, are detected allowing application specific recovery.  The intent of the event delivery mechanism is that the client can track the relevant state of each server, visible through its events.

## Event Delivery

A numeric _offset_ is assigned to each event by the server that generates it.  The client tracks the offset of the latest event successfully received from each server.  Each command or poll sent by the client carries this offset. The server normally responds with the following event or no event if the client's offset is up to date. 

A server maintains a short history of events. This enables a client to request the same event more than once, in the case of a transport error.  In general the client can fall behind the server by the length of the history. 

A loss of synchronization between client and server occurs when neither the client's offset nor its successor is found in the server's history.  This indicates an overrun where more events were generated on the server than could be stored or delivered.  Alternatively, either the client or the server may have restarted.  In either case application specific recovery may be required.   

The protocol requires the server to deliver the oldest event in its history in this scenario.  The client detects the loss of synchronization when the received event does not have the expected offset.

Details of offset calculation and assignment to events are given in [offset-rules.md](offset-rules.md).

## Event Times

Events also convey a time delta relative to the time at being served to diminish the effects of clock drift between a client and server. A client may then normalise an event's time with its own clock.

## Data Link Layer

A simplified data link layer protocol is also provided by this project so that flip-flop can be used where IP networks are not present e.g. with serial communications such as RS-485. This data layer provides a server address for up to 255 devices, 8 server ports per device, an opaque variable length payload, and AES-CCM encryption that includes authentication and error checking.

## Server discovery

> Server discovery relies on a pre-shared key between the client and servers. In the case where a key may be
> assumed e.g. a well known key of "0000000000000000", it is prone to man-in-the-middle attacks 
> and so server discovery should be performed in a controlled manner e.g. when an operator temporarily puts the client into a 
> mode of discovery having physically installed a client and/or one or more servers. Server discovery is not intended 
> to run continuously with keys that are not pre-shared.

The data link layer provides an optional discovery protocol that can be used in addition to it. The protocol depends on the data link packet format for its address scheme and the ability to detect corrupt packets via its MIC.

 When activated, the discovery protocol is able to automatically discover new servers. The client initiates server discovery.

The client sends an "identify" message to all servers. An identify message has a data source of "client", an
address of 0x00 and a port of 0x00. The payload is a bit field of 256 bits (32 bytes). The MIC is formed using
a well known key shared between the client and servers.

The identify message bit field has bits set in positions that represent a server address between 0 and 255 e.g. bit 1 represents addresss 1, bit 14 represents address 14 and so on. These server addresses
are the ones known to the client when broadcasting an identify message. The first time a client runs it will 
have no prior knowledge of any server and so all bits will be set to 0.

Servers that do not already have an address represented by the identify message's bit field are required to reply
with a payload indicating a value between 1 and 255, which will become its address. This generated address must
not conflict with an address already known to the client i.e. the number is not in conflict with addresses 
indicated by the identify message's bit field.

Each server replies within a time window and at a random time within that window. The length of the time
window is recommended to be 900ms. At 115200 baud, 12,800 bytes can be transmitted in 1 second, including 
1 stop bit for each byte. The client packet size is 41 bytes (4 byte header, 4 byte MIC, and 
a 32 byte payload including its length byte). At 41 bytes, the client's identification message will be less than 4ms on the wire. A server reply will contain a 1 byte payload and is therefore 13 bytes. The
server's reply will be on the wire for less than 2ms. Given a time window of 1000ms and 4ms for the client to transmit, there are
994ms remaining for servers to reply also given the 2ms on the wire for a server reply. 
Some time should be allowed for a server to detect, receive and process a client request. Rounding the time window
down to 900ms is reasonable.

There is always the opportunity for contention where two or more servers transmit at the same time and therefore
garble the message at the client. Server discover relies on the data link MIC to detect message integrity.

The client keeps track of the valid server replies it receives and notes their generated address.

Once the time window has passed (1 second from the client's perspective), the client will determine if it needs
to re-issue an identify message. It will do so if any invalid MICs were received, or if any of the server generated
addresses conflict with each other. Prior to re-issuing an identify message, those MICs that were valid and the
corresponding server generated addresses are distinct, are added to the bit field.

The discovery process continues until there are no more invalid MICs and no more address conflicts. Modelling has
shown that the worst-case scenario should be 12 iterations given 255 servers. In practice, server discovery 
often completes over 5 seconds.

Once the discovery process completes, a key can be shared to each server to be used for subsequent
encryption. The message format and timing of this key delivery is left as an application concern, but in general,
it should be deilvered as the first message to a new server to avoid the use of the well known key used throughout
discovery.

## Software Update

Software updates are supported by broadcasting packets of chunked software, along with an address of 0x00 and a port of 0x01.
Before any packets are sent, nominated servers (generally all) are broadcast a prepare-update command that contains a [semver](https://semver.org)-style version number and 
a byte with a bit set for each port to which the update applies. In addition, a key for the purposes of
broadcasting to the servers is included and known as the "update key". This key is generated for an entire update and avoids bad actors communicating 
untrusted software updates given that the client already knows the encryption keys for each one of its servers (see [Server Discovery]).

Note that although we broadcast to each server using an address of 0 (the broadcast address), only servers that are able to decrypt
the messages will be able to handle the update request. Other servers will drop requests they are unable to decrypt.

No reply is expected from these prepare-update commands. This simplifies the client logic and also speeds up the requests as no time
must pass other than the time taken to transmit the request plus some time reserved for a server's processing e.g. 10ms is reasonable
for a microcontroller.

Each server decides whether a prepare-update command is applicable to the ports that it supports and whether the version number
signifies a later release.

Finally, the prepare-update command includes the expected number of bytes in the update so that each server knows when the update
completes and can then act accordingly e.g. reboot with new firmware.

The subsequent firmware broadcast packets contain a byte offset and the update bytes at that offset, encrypted with the update key.
Byte offsets are conveyed starting at 0.

An application-specific "flush delay" is typically determined that allows servers some period of time to perform operations such
as, in the case of microcontrollers, writing their update buffer to flash. For example, an nRF52840 microcontroller takes 85ms to
write 4KiB of data to its flash where 4KiB is also the minimum amount of data that can be written. In this instance, the client
should pause for each 4KiB of update data sent, perhaps for 100ms with a microcontroller based server.

The flush delay is always awaited once all update broadcast completes. This provides enough time for the final bytes to be processed
by the server.

If a server misses an update message then it will ignore all subsequent ones by dropping the shared key and thus becoming ineligible
to receive the update.

If a server updates its firmware as a consequence of this broadcast then it is also expected to emit an application-specific
event signifying the version of the firmware it now has. The details of this event are outside of the flip-flop
specification.

### Signing

The data protocol also accomodates a `signed` field. When set then the last n bytes of the update are to be interpreted as the
digital signature. The length of the digital signature and its type are agreed between the client and server and outside of this 
specification. It is expected that a software update should not proceed given the presence of signed data not verifying.

## Why flip-flop?

Reason #1: data flow between a client and server "flip flops" i.e. the protocol is designed to be only be in one of two states of flow where either the client is sending and servers are receiving, or a server is sending and a client is receiving.

Reason #2: We thought it was catchy.

Reason #3: We like Flip Flops (otherwise known as "Thongs"!).

![Flip Flops!](Australia_Day_Thongs.jpg "Australia Day Flip Flops!")


[Image courtesy of Wikimedia and licensed under CC BY-SA 4.0](https://commons.wikimedia.org/wiki/Category:Flip-flops_(footwear)#/media/File:Australia_Day_Thongs.tiff).

## Contribution policy

Contributions via GitHub pull requests are gladly accepted from their original author. Along with any pull requests, please state that the contribution is your original work and that you license the work to the project under the project's open source license. Whether or not you state this explicitly, by submitting any copyrighted material via pull request, email, or other means you agree to license the material under the project's open source license and warrant that you have the legal authority to do so.

## License

This code is open source software licensed under the [Apache-2.0 license](./LICENSE).

© Copyright [Titan Class P/L](https://www.titanclass.com.au/), 2021
