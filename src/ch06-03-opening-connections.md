## Opening Connections

In this section we are finally going to understand how Floresta connects to peers. The highest-level method for opening connections is `maybe_open_connection`, which will redirect us to other lower-level functionality. Remember that these methods are context-independent.

### When to Open Connections

The `maybe_open_connection` method determines whether the node should establish a new connection to a peer and, if so, calls `create_connection`.

```rust
# // Path: floresta-wire/src/p2p_wire/node.rs
#
pub(crate) async fn maybe_open_connection(&mut self) -> Result<(), WireError> {
    // If the user passes in a `--connect` cli argument, we only connect with
    // that particular peer.
    if self.fixed_peer.is_some() && !self.peers.is_empty() {
        return Ok(());
    }
    // if we need utreexo peers, we can bypass our max outgoing peers limit in case
    // we don't have any utreexo peers
    let bypass = self
        .1
        .get_required_services()
        .has(service_flags::UTREEXO.into())
        && !self.has_utreexo_peers();

    if self.peers.len() < T::MAX_OUTGOING_PEERS || bypass {
        self.create_connection(ConnectionKind::Regular).await;
    }

    Ok(())
}
```

If the user has specified a fixed peer via the `--connect` command-line argument (`self.fixed_peer.is_some()`) and there are already connected peers (`!self.peers.is_empty()`), the method does nothing and exits early. This is because we have already connected to the fixed peer.

Also, if the node needs utreexo-related services (`UTREEXO` service flag) for its specific context, but doesn’t have any peers offering them (`!self.has_utreexo_peers()`), it sets a `bypass` flag to ignore the usual connection limit.

Finally, if the number of peers is below the maximum allowed (`self.peers.len() < T::MAX_OUTGOING_PEERS`) or the `bypass` condition is true, it calls the `create_connection` method to establish a new 'regular' connection to a peer.

The `ConnectionKind` struct that `create_connection` takes as argument is explained below.

### Connection Kinds

```rust
# // Path: floresta-wire/src/p2p_wire/node.rs
#
pub enum ConnectionKind {
    Feeler,
    Regular,
    Extra,
}
```

#### Feeler Connections
Feeler connections are temporary probes used to verify if a peer is still active, regardless of its supported services. These lightweight tests help maintain an up-to-date and reliable pool of peers, ensuring the node can quickly establish connections when needed.

#### Regular Connections
Regular connections are the backbone of a node's peer-to-peer communication. These connections are established with trusted peers or those that meet specific service criteria (e.g., support for Utreexo or compact filters). Regular connections are long-lived and handle the bulk of the node's operations, such as exchanging blocks, headers, transactions, and keeping the node in sync.

#### Extra Connections
Extra connections extend the node’s reach by connecting to additional peers for specialized tasks, such as compact filter requests or fetching Utreexo proofs. These are temporary and created only when extra resources are required.

### Create Connection

`create_connection` gets required services via another method from `UtreexoNode`, gets a peer address (prioritizing the fixed peer if specified), and ensures the peer isn’t already connected.

If no fixed peer is specified, we obtain a suitable peer address (or `LocalAddress`) for connection by calling `self.address_man.get_address_to_connect`. This method takes the required services and a boolean indicating whether a feeler connection is desired. We will explore this method in the next section.

```rust
# // Path: floresta-wire/src/p2p_wire/node.rs
#
pub(crate) async fn create_connection(&mut self, kind: ConnectionKind) -> Option<()> {
    let required_services = self.get_required_services();
    let address = match &self.fixed_peer {
        Some(address) => Some((0, address.clone())),
        None => self
            .address_man
            .get_address_to_connect(required_services, matches!(kind, ConnectionKind::Feeler)),
    };

    # debug!(
        # "attempting connection with address={:?} kind={:?}",
        # address, kind
    # );
    let (peer_id, address) = address?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Defaults to failed, if the connection is successful, we'll update the state
    self.address_man
        .update_set_state(peer_id, AddressState::Failed(now));

    // Don't connect to the same peer twice
    if self
        .0
        .peers
        .iter()
        .any(|peers| peers.1.address == address.get_net_address())
    {
        return None;
    }
    self.open_connection(kind, peer_id, address).await;

    Some(())
}
```

Then we use the obtained `LocalAddress` and the peer identifier as arguments for `open_connection`, as well as the connection kind.

Both the `LocalAddress` type and the `get_address_to_connect` method are implemented in the address manager module (_p2p_wire/address_man.rs_) that we will see in the next section.

### Open Connection

Moving on to `open_connection`, we create a new `unbounded_channel` for sending requests to the `Peer` instance. Recall that the `Peer` component is in charge of actually connecting to the respective peer over the network.

Then, depending on the value of `self.socks5` we will call `UtreexoNode::open_proxy_connection` or `UtreexoNode::open_non_proxy_connection`. Each one of these functions will create a `Peer` instance with the provided data and the channel receiver.

```rust
# // Path: floresta-wire/src/p2p_wire/node.rs
#
pub(crate) async fn open_connection(
    &mut self,
    kind: ConnectionKind,
    peer_id: usize,
    address: LocalAddress,
) {
    let (requests_tx, requests_rx) = unbounded_channel();
    if let Some(ref proxy) = self.socks5 {
        spawn(timeout(
            Duration::from_secs(10),
            Self::open_proxy_connection(
                // Arguments omitted for brevity :P
                # proxy.address,
                # kind,
                # self.mempool.clone(),
                # self.network.into(),
                # self.node_tx.clone(),
                # peer_id,
                # address.clone(),
                # requests_rx,
                # self.peer_id_count,
                # self.config.user_agent.clone(),
            ),
        ));
    } else {
        spawn(timeout(
            Duration::from_secs(10),
            Self::open_non_proxy_connection(
                // Arguments omitted for brevity :P
                # kind,
                # peer_id,
                # address.clone(),
                # requests_rx,
                # self.peer_id_count,
                # self.mempool.clone(),
                # self.network.into(),
                # self.node_tx.clone(),
                # self.config.user_agent.clone(),
            ),
        ));
    }

    let peer_count: u32 = self.peer_id_count;

    self.inflight.insert(
        InflightRequests::Connect(peer_count),
        (peer_count, Instant::now()),
    );

    self.peers.insert(
        peer_count,
        LocalPeerView {
            // Fields omitted for brevity :P
            # address: address.get_net_address(),
            # port: address.get_port(),
            # user_agent: "".to_string(),
            # state: PeerStatus::Awaiting,
            # channel: requests_tx,
            # services: ServiceFlags::NONE,
            # _last_message: Instant::now(),
            # kind,
            # address_id: peer_id as u32,
            # height: 0,
            # banscore: 0,
        },
    );

    self.peer_id_count += 1;
}
```

Last of all, we simply insert the new inflight request (via the `InflightRequests` type) to our tracker `HashMap`, as well as the new peer (via the `LocalPeerView`). Both types are also defined in _p2p_wire/node.rs_, along with `UtreexoNode`, `NodeCommon`, `ConnectionKind`, and a few other types.

### Recap

In this section, we have learned how Floresta establishes peer-to-peer connections, starting with the `maybe_open_connection` method. This method initiates a connection if we aren't already connected to the optional fixed peer and either have fewer connections than `Context::MAX_OUTGOING_PEERS` or lack a peer offering utreexo services.

We explored the three connection types: `Feeler` (peer availability check), `Regular` (core communication), and `Extra` (specialized services). The `create_connection` method selects an appropriate peer address while preventing duplicate connections, and `open_connection` handles the network setup, either via a proxy or directly (internally creating a new `Peer`). Finally, we examined how new connections are tracked using inflight requests and a peer registry, both fields of `NodeCommon`.