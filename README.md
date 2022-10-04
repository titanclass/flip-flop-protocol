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

A simplified data link layer protocol is also provided by this project so that flip-flop can be used where IP networks are not present e.g. with serial communications such as RS-485. This data layer provides a server address for up to 255 devices, 8 server ports per device, an opaque variable length payload, and a encryption for authentication and error checking.

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
