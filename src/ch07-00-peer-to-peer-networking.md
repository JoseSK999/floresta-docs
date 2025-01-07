# Peer-to-Peer Networking

In the previous chapter, we learned how `UtreexoNode` opens connections, although we didn't dive into the low-level networking details. We mentioned that each peer connection is handled by the `Peer` type, keeping the peer networking logic separate from `UtreexoNode`.

In this chapter, we will explore the details of `Peer` operations, beginning with the low-level logic for opening connections (i.e. `Peer` creation).

## Peer Creation

Recall that in [the `open_connection` method](ch06-03-opening-connections.md#open-connection) on `UtreexoNode` we call either `UtreexoNode::open_proxy_connection` or `UtreexoNode::open_non_proxy_connection`, depending on the `self.socks5` proxy option. It's within these two functions that the `Peer` is created. Let's first learn how the direct TCP connection is opened!

The `open_non_proxy_connection` function will first retrieve the peer's network address and port from the provided `LocalAddress` and attempt to establish a TCP connection using `TcpStream::connect` (from the `tokio` crate). If successful, it enables the `TCP_NODELAY` option to reduce latency by disabling Nagle's algorithm.

> Nagle's Algorithm is a TCP feature designed to improve network efficiency by combining small data packets into larger ones before sending them. While this reduces overhead, it can introduce delays for latency-sensitive applications. The `nodelay` option disables Nagle's Algorithm, ensuring data is sent immediately without waiting to batch packets, making it ideal for real-time communication.

Next, the function splits the TCP stream into a reader and a writer using `tokio::io::split`. The reader, of type `ReadHalf`, is used for receiving data, while the writer, of type `WriteHalf`, is used for sending data.

It then sets up a **TCP stream actor**, that is, an independent component that reads incoming messages. The actor is effectively a stream reader wrapper.

```rust
# // Path: floresta-wire/src/p2p_wire/node.rs
#
pub(crate) async fn open_non_proxy_connection(
    kind: ConnectionKind,
    peer_id: usize,
    address: LocalAddress,
    requests_rx: UnboundedReceiver<NodeRequest>,
    peer_id_count: u32,
    mempool: Arc<RwLock<Mempool>>,
    network: bitcoin::Network,
    node_tx: UnboundedSender<NodeNotification>,
    user_agent: String,
) -> Result<(), WireError> {
    let address = (address.get_net_address(), address.get_port());
    let stream = TcpStream::connect(address).await?;

    stream.set_nodelay(true)?;
    let (reader, writer) = tokio::io::split(stream);

    let (cancellation_sender, cancellation_receiver) = tokio::sync::oneshot::channel();
    let (actor_receiver, actor) = create_tcp_stream_actor(reader);
    tokio::spawn(async move {
        tokio::select! {
            _ = cancellation_receiver => {}
            _ = actor.run() => {}
        }
    });

    // Use create_peer function instead of manually creating the peer
    Peer::<WriteHalf>::create_peer(
        peer_id_count,
        mempool,
        network,
        node_tx.clone(),
        requests_rx,
        peer_id,
        kind,
        actor_receiver,
        writer,
        user_agent,
        cancellation_sender,
    )
    .await;

    Ok(())
}
```

This actor is obtained via the `create_tcp_stream_actor` function, implemented in _p2p_wire/peer.rs_, which returns the actor receiver (to get the peer messages) and actor instance, of type `TcpStreamActor`. **The actor is spawned as a separate asynchronous task**, ensuring it runs independently to handle incoming data.

Very importantly, the actor for a peer must be closed when the connection finalizes, and this is why we have an additional one-time-use channel, used by the `Peer` type to send a cancellation signal (i.e. "_Peer connection is closed, so we don't need to listen to the peer anymore_"). The `tokio::select` macro ensures that the async actor task is dropped whenever a cancellation signal is received from `Peer`.

Finally, the `Peer` instance is created using the `Peer::create_peer` function. The communication channels (internal and over the P2P network) that the `Peer` uses are:

- The node sender (`node_tx`): to send messages to `UtreexoNode`.
- The requests receiver (`requests_rx`): to receive requests from `UtreexoNode` that will be sent to the peer.
- The `actor_receiver`: to receive peer messages.
- The TCP stream `writer`: to send messages to the peer.
- The `cancellation_sender`: to close the TCP reader actor.

By the end of this function, a fully initialized `Peer` is ready to manage communication with the connected peer via TCP (writing side) and via `TcpStreamActor` (reading side), as well as communicating with `UtreexoNode`.

### Proxy Connection

The `open_proxy_connection` is pretty much the same, except we get the TCP stream writer and reader from the proxy connection instead. The proxy setup is handled by the `Socks5StreamBuilder::connect` method, implemented in _p2p_wire/socks_.

```rust
# // Path: floresta-wire/src/p2p_wire/node.rs
#
pub(crate) async fn open_proxy_connection(
    proxy: SocketAddr,
    // ...
    # kind: ConnectionKind,
    # mempool: Arc<RwLock<Mempool>>,
    # network: bitcoin::Network,
    # node_tx: UnboundedSender<NodeNotification>,
    # peer_id: usize,
    # address: LocalAddress,
    # requests_rx: UnboundedReceiver<NodeRequest>,
    # peer_id_count: u32,
    # user_agent: String,
) -> Result<(), Socks5Error> {
    let addr = match address.get_address() {
        // Convert to a SOCKS5 address
        # AddrV2::Cjdns(addr) => Socks5Addr::Ipv6(addr),
        # AddrV2::I2p(addr) => Socks5Addr::Domain(addr.into()),
        # AddrV2::Ipv4(addr) => Socks5Addr::Ipv4(addr),
        # AddrV2::Ipv6(addr) => Socks5Addr::Ipv6(addr),
        # AddrV2::TorV2(addr) => Socks5Addr::Domain(addr.into()),
        # AddrV2::TorV3(addr) => Socks5Addr::Domain(addr.into()),
        # AddrV2::Unknown(_, _) => {
            # return Err(Socks5Error::InvalidAddress);
        # }
    };

    let proxy = TcpStream::connect(proxy).await?;
    // Set up the SOCKS5 proxy stream
    let stream = Socks5StreamBuilder::connect(proxy, addr, address.get_port()).await?;

    let (reader, writer) = tokio::io::split(stream);

    let (cancellation_sender, cancellation_receiver) = tokio::sync::oneshot::channel();
    let (actor_receiver, actor) = create_tcp_stream_actor(reader);
    tokio::spawn(async move {
        tokio::select! {
            _ = cancellation_receiver => {}
            _ = actor.run() => {}
        }
    });

    Peer::<WriteHalf>::create_peer(
        // Same as before
        # peer_id_count,
        # mempool,
        # network,
        # node_tx,
        # requests_rx,
        # peer_id,
        # kind,
        # actor_receiver,
        # writer,
        # user_agent,
        # cancellation_sender,
    )
    .await;
    Ok(())
}
```

## Recap of Channels

Let's do a brief recap of the channels we have opened for internal node message passing:

- **Node Channel** (`Peer` -> `UtreexoNode`)
  - `Peer` sends via `node_tx`
  - `UtreexoNode` receives via `NodeCommon.node_rx`

- **Requests Channel** (`UtreexoNode` -> `Peer`)
  - `UtreexoNode` sends via each `LocalPeerView.channel`, stored in `NodeCommon.peers`
  - `Peer` receives via its `requests_rx`

- **TCP Actor Channel** (`TcpStreamActor` -> `Peer`)
  - `TcpStreamActor` sends via `actor_sender`
  - `Peer` receives via `actor_receiver`

- **Cancellation Signal Channel** (`Peer` -> `UtreexoNode`)
  - `Peer` sends the signal via `cancellation_sender` at the end of the connection
  - `UtreexoNode` receives it via `cancellation_receiver`

`UtreexoNode` sends requests via the **Request Channel** to the `Peer` component (which then forwards them to the peer via TCP), `Peer` receives the result or other peer messages via the **Actor Channel**, and then it notifies `UtreexoNode` via the **Node Channel**. When the peer connection is closed, `Peer` uses the **Cancellation Signal Channel** to allow the TCP actor listening to the peer to be closed as well.

Next, we'll explore how messages are read and sent in the P2P network!
