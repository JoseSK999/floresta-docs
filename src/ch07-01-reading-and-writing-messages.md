## Reading and Writing Messages

Let's first learn about the `TcpStreamActor` we created just before instantiating the `Peer` type, tasked with reading messages from the corresponding peer.

### TCP Stream Actor

The `TcpStreamActor` type is a simple struct that wraps the stream reader and communicates to the `Peer` via an unbound channel.

Filename: p2p_wire/peer.rs

```rust
# // Path: floresta-wire/src/p2p_wire/peer.rs
#
pub struct TcpStreamActor<T: AsyncRead + Unpin> {
    pub stream: T,
    pub sender: UnboundedSender<ReaderMessage>,
}
```

```rust
# // Path: floresta-wire/src/p2p_wire/peer.rs
#
pub fn create_tcp_stream_actor(
    stream: impl AsyncRead + Unpin,
) -> (
    UnboundedReceiver<ReaderMessage>,
    TcpStreamActor<impl AsyncRead + Unpin>,
) {
    // Open an unbound channel to communicate read peer messages
    let (actor_sender, actor_receiver) = unbounded_channel();

    // Initialize the actor with the `actor_sender` and the TCP stream reader
    let actor = TcpStreamActor {
        stream,
        sender: actor_sender,
    };

    // Return the `actor_receiver` (to receive P2P messages from the actor), and the actor
    (actor_receiver, actor)
}
```

This `TcpStreamActor` implements a `run` method, which independently handles all incoming messages from the corresponding peer (via the TCP stream reader), and sends them to the `Peer` type (via the channel).

Note that the messages of the channel between `TcpStreamActor` and `Peer` are of type `ReaderMessage`. Let's briefly see what is this type, which is also defined in _peer.rs_.

```rust
# // Path: floresta-wire/src/p2p_wire/peer.rs
#
pub enum ReaderMessage {
    Block(UtreexoBlock),
    Message(RawNetworkMessage),
    Error(PeerError),
}
```

- `UtreexoBlock` is a type defined in `floresta-chain` that wraps the `bitcoin::Block` type as well as the utreexo data needed for validation (proofs and spent UTXOs).
- `RawNetworkMessage` is a type from the `bitcoin` crate (used here for all messages that are not `ReaderMessage::Block`).
- `PeerError` is the unified error type for the `Peer` struct (similar to how `WireError` is the error type for `UtreexoNode`).

Below we will see the `run` method in action, which listens to the peer via TCP (and sends the read messages to the `Peer` component).

### Reading Messages

```rust
# // Path: floresta-wire/src/p2p_wire/peer.rs
#
pub async fn run(mut self) -> Result<()> {
    let err = self.inner().await;
    if let Err(err) = err {
        self.sender.send(ReaderMessage::Error(err))?;
    }
    Ok(())
}
```

The `run` method simply invokes the `inner` method, and if it fails we notify the error to the `Peer`. Let's see the full `inner` method.

```rust
# // Path: floresta-wire/src/p2p_wire/peer.rs
#
async fn inner(&mut self) -> std::result::Result<(), PeerError> {
    loop {
        let mut data: Vec<u8> = vec![0; 24];

        // Read the header first, so learn the payload size
        self.stream.read_exact(&mut data).await?;
        let header: P2PMessageHeader = deserialize_partial(&data)?.0;

        // Network Message too big
        if header.length > (1024 * 1024 * 32) as u32 {
            return Err(PeerError::MessageTooBig);
        }

        data.resize(24 + header.length as usize, 0);

        // Read everything else
        self.stream.read_exact(&mut data[24..]).await?;

        // Intercept block messages
        if header._command[0..5] == [0x62, 0x6c, 0x6f, 0x63, 0x6b] {
            let mut block_data = vec![0; header.length as usize];
            block_data.copy_from_slice(&data[24..]);

            let message: UtreexoBlock = deserialize(&block_data)?;
            self.sender.send(ReaderMessage::Block(message))?;
        }

        let message: RawNetworkMessage = deserialize(&data)?;
        self.sender.send(ReaderMessage::Message(message))?;
    }
}
```

This method is responsible for continuously reading messages from the TCP stream, processing them, and sending them to the `Peer`. It reads a fixed-size header to determine the payload size, validates the size, then reads the full message.

Special handling is applied for block messages, which are deserialized and sent as `ReaderMessage::Block`. The rest of messages are deserialized and sent as `ReaderMessage::Message`. If an error occurs (e.g., message too large or deserialization failure), it stops and sends a `ReaderMessage::Error`.

### Writing Messages

In order to write messages via the TCP stream, we use the following `write` method on `Peer`:

```rust
# // Path: floresta-wire/src/p2p_wire/peer.rs
#
pub async fn write(&mut self, msg: NetworkMessage) -> Result<()> {
    debug!("Writing {} to peer {}", msg.command(), self.id);
    let data = &mut RawNetworkMessage::new(self.network.magic(), msg);
    let data = serialize(&data);
    self.writer.write_all(&data).await?;
    self.writer.flush().await?;
    Ok(())
}
```

The `NetworkMessage` is another `bitcoin` type, similar to the `RawNetworkMessage`. This type contains the payload, but needs to be converted into a `RawNetworkMessage` in order to be sent through the network.

This method simply performs the conversion, serializes the resulting raw message, and writes it via the TCP stream `writer`.

And that's all about how we read and write P2P messages! Next, we'll explore a few `Peer` methods and how it handles network messages.
