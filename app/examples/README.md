# Examples

There are two examples here that demonstrate how a client interacts with a server.

The examples use Tokio as their executor, and UDP as the network transport.

To run the examples, both the client and server need to be running. It should not matter which is started up or if either are restarted. To run the example client, cd to the root of the `app` project folder and:

```
cargo run --example client
```

...and for the server:

```
cargo run --example server
```