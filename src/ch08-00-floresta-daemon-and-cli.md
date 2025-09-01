# Running Floresta: Daemon and CLI

So far in the Floresta journey we have mostly learned about the two core library crates: `floresta-chain` and `floresta-wire`.

In [Chapter 1](ch01-00-project-overview.md) we went over the common library (`floresta-common`), the three extra libraries (`floresta-watch-only`, `floresta-compact-filters`, and `floresta-electrum`) and the `floresta` meta-crate. We also mentioned `floresta-rpc` and `floresta-node`, the libraries powering our two current binaries.

This chapter puts it all together leveraging the assembler library `floresta-node`, which provides:

- The `Florestad` struct: the "orchestrator" that starts the node, spawning asynchronous tasks for each subsystem (the `UtreexoNode`, an optional JSON-RPC server, the Electrum server, etc.). It also sends the stop signal when requested to, as the main loop in `florestad` keeps checking for a `Ctrl+C` signal.

- The `Config` type: a single configuration struct used to bootstrap `Florestad`.

## Florestad

The `florestad` binary builds a `Config` from the CLI arguments, conditionally turns the process into a background daemon (if the `daemon` CLI argument is `true`, which is not the default value), and hands control to `Florestad::start` to spawn the node.

```rust
# // Path: ../florestad/src/main.rs
#
fn main() {
    #[cfg(feature = "tokio-console")]
    {
        // Initialize tokio-console for debugging
        console_subscriber::init();
    }

    let params = Cli::parse();
    let config = Config {
        // Setting the config from the CLI arguments
        // ...
        # disable_dns_seeds: params.connect.is_some() || params.disable_dns_seeds,
        # network: params.network,
        # debug: params.debug,
        # data_dir: params.data_dir.clone(),
        # cfilters: !params.no_cfilters,
        # proxy: params.proxy,
        # assume_utreexo: !params.no_assume_utreexo,
        # connect: params.connect,
        # wallet_xpub: params.wallet_xpub,
        # config_file: params.config_file,
        # #[cfg(unix)]
        # log_to_file: params.log_to_file || params.daemon,
        # #[cfg(not(unix))]
        # log_to_file: params.log_to_file,
        # assume_valid: params.assume_valid,
        # log_to_stdout: true,
        # #[cfg(feature = "zmq-server")]
        # zmq_address: params.zmq_address,
        # #[cfg(feature = "json-rpc")]
        # json_rpc_address: params.rpc_address,
        # generate_cert: params.generate_cert,
        # wallet_descriptor: params.wallet_descriptor,
        # filters_start_height: params.filters_start_height,
        # user_agent: format!("/Floresta:{}/", env!("GIT_DESCRIBE")),
        # assumeutreexo_value: None,
        # electrum_address: params.electrum_address,
        # enable_electrum_tls: params.enable_electrum_tls,
        # electrum_address_tls: params.electrum_address_tls,
        # tls_cert_path: params.tls_cert_path,
        # tls_key_path: params.tls_key_path,
        # allow_v1_fallback: params.allow_v1_fallback,
        # backfill: !params.no_backfill,
    };

    #[cfg(unix)]
    if params.daemon {
        // Daemonizing the process
        // ...
        # let mut daemon = Daemonize::new();
        # if let Some(pid_file) = params.pid_file {
            # daemon = daemon.pid_file(pid_file);
        # }
        # daemon.start().expect("Failed to daemonize");
    }

    let _rt = tokio::runtime::Builder::new_multi_thread()
        // Setting the runtime
        // ...
        # .enable_all()
        # .worker_threads(4)
        # .max_blocking_threads(2)
        # .thread_keep_alive(Duration::from_secs(60))
        # .thread_name("florestad")
        # .build()
        # .unwrap();

    let signal = Arc::new(RwLock::new(false));
    let _signal = signal.clone();

    _rt.spawn(async move {
        // This is used to signal the runtime to stop gracefully.
        // It will be set to true when we receive a Ctrl-C or a stop signal.
        tokio::signal::ctrl_c().await.unwrap();
        let mut sig = signal.write().await;
        *sig = true;
    });

    let florestad = Florestad::from(config);
    _rt.block_on(async {
        florestad.start().await.unwrap_or_else(|e| {
            eprintln!("Failed to start florestad: {e}");
            exit(1);
        });

        // wait for shutdown
        loop {
            if florestad.should_stop().await || *_signal.read().await {
                info!("Stopping Florestad");
                florestad.stop().await;
                let _ = timeout(Duration::from_secs(10), florestad.wait_shutdown()).await;
                break;
            }

            sleep(Duration::from_secs(5)).await;
        }
    });

    # // drop them outside the async block, so we won't cause a nested drop of the runtime
    # // due to the rpc server, causing a panic.
    drop(florestad);
    drop(_rt);
}
```

You can see we first spawn a process that waits for the `Ctrl+C` signal, and when it's read this task writes `true` to the signal variable (an `Arc<RwLock<bool>>`). Then the loop executes `florestad.stop().await;`, which simply replays the signal to the `UtreexoNode`, so it will "know" when to shut down (we saw how `UtreexoNode::new` required a kill signal in [Chapter 6.2](ch06-02-utreexonode-config-and-builder.md)).

You may also have noticed that the first line of the main function is a feature-gated call to `console_subscriber::init();`. This sets up a tracing subscriber for [tokio-console](https://github.com/tokio-rs/console), an official utility from the Tokio project that lets you inspect all running async tasks in real time. This tool is super valuable for debugging complex async applications like Floresta. You can learn more about its usage for Floresta in the [Floresta doc folder](https://github.com/vinteumorg/Floresta/blob/master/doc/run.md#using-tokio-console).

Finally, you can also take a look at the `florestad/cli.rs` file to see the exact CLI arguments that are supported, or run `cargo run --bin florestad -- --help` to see the help message. One particularly useful option is `--proxy`, which routes P2P traffic (and DNS seed lookups) through a SOCKS5 proxy, such as Tor:

```bash
# start the daemon with the Tor proxy
florestad --proxy 127.0.0.1:9050
```

You may also want to disable `Assume-Utreexo`, the UTXO set snapshot sync we discussed in [Chapter 5](ch05-00-advanced-chain-validation-methods.md#trusted-utxo-set-snapshots), which is enabled by default:

```bash
# start the daemon from the genesis block
florestad --no-assume-utreexo
```

It's highly recommended to take a look at the [Floresta doc folder](https://github.com/vinteumorg/Floresta/blob/master/doc), where many more details about building, running and testing Floresta are available.

## Floresta-CLI

`floresta-cli` is a thin CLI that talks to `florestad`'s JSON-RPC server. It uses a simple JSON-RPC client that can call any method defined by the `FlorestaRPC` trait from `floresta-rpc`.

```rust
# // Path: floresta-cli/src/rpc.rs
#
/// A trait specifying all possible methods for floresta's json-rpc
pub trait FlorestaRPC {
    /// Get the BIP158 filter for a given block height
    ///
    /// BIP158 filters are a compact representation of the set of transactions in a block,
    /// designed for efficient light client synchronization. This method returns the filter
    /// for a given block height, encoded as a hexadecimal string.
    /// You need to have enabled block filters by setting the `blockfilters=1` option
    fn get_block_filter(&self, height: u32) -> Result<String>;
    // ...
```

Many of these `FlorestaRPC` methods are an adaptation of the standard RPC methods from Bitcoin Core, although behavior may change as Floresta doesn't store the UTXO set and is fully pruned. We also have utreexo-specific methods, like `get_roots`. You can check the available RPC methods by running `cargo run --bin floresta-cli -- --help`.

The `floresta-cli` client is implemented as a simple wrapper around the `jsonrpc` library:

```rust
# // Path: floresta-cli/src/jsonrpc_client.rs
#
// Define a Client struct that wraps a jsonrpc::Client
#[derive(Debug)]
pub struct Client(jsonrpc::Client);
```

To use the `floresta-cli` client, you need to first start `florestad` with the `json-rpc` feature, which is enabled by default. Then you can run any RPC commands like:

```bash
# Rescan from height 100 to 200
floresta-cli rescanblockchain 100 200

# Get the current blockchain info
floresta-cli getblockchaininfo
```

Again, for more details about the `floresta-cli` usage and RPC commands, you can check the [Floresta doc folder](https://github.com/vinteumorg/Floresta/blob/master/doc).
