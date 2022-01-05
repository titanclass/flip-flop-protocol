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

## RS485 Link Layer

A simplified link layer protocol is also provided by this project so that flip-flop can be used where IP networks are not present e.g. with serial communications such as RS-485. This data layer provides a server address, a server port, an opaque variable length payload, and a CRC for error checking.

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
