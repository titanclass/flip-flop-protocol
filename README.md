# flip-flop-protocol

*A single-client/multi-server event-bus protocol suitable for half-duplex communications.*

Flip-flop is an OSI-style application layer protocol optimised for half-duplex communication where a single client may command one of a number of servers. The server matching the address of a command is then expected to respond with an event. Communication is expected to be "best-effort" and the lower levels control the level of guarantees in terms of delivvery.

Commands instruct a server to do something, normally resulting in an event. All commands convey an offset to the last event that the client received so that a server knows the next event it should reply with. Commands are use-definable.

A server maintains a history of events which may or may not be in relation to the processing of a command received. Events are designated with an offset and are user-definable. Events also convey a time delta relative to the time at being served to diminish the effects of clock drift betweeen a client and server. A client may then normalise an event's time with its own clock.

A server replies to a command with the next "nearest" event. The "nearest" event is where its offset is greater than the command's last offset.

Offsets are held as an unsigned 32 bit integer and may overflow to zero. In the situation of having overflowed, a client must forget all prior events and a server must ensure that any important events are re-sent. A client may detect this
situation by checking whether the received offset is less than or equal to the one it has.

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