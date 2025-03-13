## Reading and Writing Messages

Let's first learn about the `MessageActor` we created just before instantiating the `Peer` type, tasked with reading messages from the corresponding peer.

### TCP Message Actor

The `MessageActor` type is a simple struct that wraps the transport reader and communicates to the `Peer` via an unbound channel.

Filename: p2p_wire/peer.rs

```rust
# // Path: floresta-wire/src/p2p_wire/peer.rs
#
pub struct MessageActor<R: AsyncRead + Unpin + Send> {
    pub transport: ReadTransport<R>,
    pub sender: UnboundedSender<ReaderMessage>,
}
```

```rust
# // Path: floresta-wire/src/p2p_wire/peer.rs
#
pub fn create_actors<R: AsyncRead + Unpin + Send>(
    transport: ReadTransport<R>,
) -> (UnboundedReceiver<ReaderMessage>, MessageActor<R>) {
    // Open an unbound channel to communicate read peer messages
    let (actor_sender, actor_receiver) = unbounded_channel();

    // Initialize the actor with the `actor_sender` and the transport reader
    let actor = MessageActor {
        transport,
        sender: actor_sender,
    };

    // Return the `actor_receiver` (to receive P2P messages from the actor), and the actor
    (actor_receiver, actor)
}
```

This `MessageActor` implements a `run` method, which independently handles all incoming messages from the corresponding peer, and sends them to the `Peer` type.

Note that the messages of the channel between `MessageActor` and `Peer` are of type `ReaderMessage`. Let's briefly see what is this type, which is also defined in _peer.rs_.

```rust
# // Path: floresta-wire/src/p2p_wire/peer.rs
#
pub enum ReaderMessage {
    Block(UtreexoBlock),
    Message(NetworkMessage),
    Error(PeerError),
}
```

- `UtreexoBlock` is a type defined in `floresta-chain` that wraps the `bitcoin::Block` type as well as the utreexo data needed for validation (proofs and spent UTXOs).
- `NetworkMessage` is a type from the `bitcoin` crate (used here for all messages that are not `ReaderMessage::Block`).
- `PeerError` is the unified error type for the `Peer` struct (similar to how `WireError` is the error type for `UtreexoNode`).

### Reading Messages

The `run` method simply invokes the `inner` method, and if it fails we notify the error to the `Peer`. The `inner` method is responsible for continuously reading messages from the transport via `read_message` and sending them to the `Peer`.

```rust
# // Path: floresta-wire/src/p2p_wire/peer.rs
#
async fn inner(&mut self) -> std::result::Result<(), PeerError> {
    loop {
        match self.transport.read_message().await? {
            UtreexoMessage::Standard(msg) => {
                self.sender.send(ReaderMessage::Message(msg))?;
            }
            UtreexoMessage::Block(block) => {
                self.sender.send(ReaderMessage::Block(block))?;
            }
        }
    }
}

pub async fn run(mut self) -> Result<()> {
    if let Err(err) = self.inner().await {
        self.sender.send(ReaderMessage::Error(err))?;
    }
    Ok(())
}
```

We see two kinds of messages that we get from the transport component: standard messages (`bitcoin::p2p::message::NetworkMessage`) and block messages (the `UtreexoBlock` we have mentioned).

The `read_message` method, implemented in _transport.rs_ will actually read all the data from the peer, using either the v1 or v2 transport protocols.

### Writing Messages

In order to write messages via the transport, we use the following `write` method on `Peer`:

```rust
# // Path: floresta-wire/src/p2p_wire/peer.rs
#
pub async fn write(&mut self, msg: NetworkMessage) -> Result<()> {
    debug!("Writing {} to peer {}", msg.command(), self.id);
    self.writer.write_message(msg).await?;
    Ok(())
}
```

Once again, here we delegate on a `write_message` method on the transport writer component. This method serializes the data and sends it via the respective transport protocol.

And that's all about how we read and write P2P messages! Next, we'll explore a few `Peer` methods and how it handles network messages.
