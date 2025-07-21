## Apendix A: FlatChainStore Architecture

As introduced in [Chapter 2](ch02-01-the-chainstore-trait.md#flatchainstore-and-kvchainstore), the default Floresta `ChainStore` implementation, which is in charge of persisting the pruned blockchain and state data, is `FlatChainStore`.

The "flat" in the name reflects that the store consists solely of raw `.bin` files, where the serialized data is simply written at different locations. Then, we create a [memory-map](https://en.wikipedia.org/wiki/Memory-mapped_file) that allows us to read and write to these files as if they were in memory.

### On-Disk Layout

`FlatChainStore` takes advantage of two observations about the blockchain data that our pruned node stores:

1. **Header lookups by height** are trivially positional in a flat file. If the raw file is just a header vector, we can fetch the header at any height `h` by reading the vector element at that position (i.e., doing `headers[h]` but with a memory-mapped file).

2. **Header lookups by hash** can work the same way, but with the intermediate step of getting the corresponding height (via a hash → height map). Once we have the height we can index the flat file vector as before.

Leveraging these facts, `FlatChainStore` splits the chain data across five plain files:

```rust
chaindata/
  ├─ headers.bin        # mmap‑ed vector<HashedDiskHeader>
  ├─ fork_headers.bin   # mmap‑ed vector<HashedDiskHeader> for fork chains
  ├─ blocks_index.bin   # mmap‑ed vector<u32>, accessed via a hash‑map linking block hashes to heights
  │
  ├─ accumulators.bin   # plain file (roots blob, var‑len records)
  └─ metadata.bin       # mmap‑ed Metadata struct (version, checksums, file lengths...)
```

#### Headers

We store the headers as a `HashedDiskHeader`, which consists of a `DiskBlockHeader` (the Floresta type we saw in [Chapter 2](ch02-02-bestchain-and-diskblockheader.md#diskblockheader)) and its `BlockHash`, along with the file position and length of the corresponding accumulator.

```rust
# // Path: floresta-chain/src/pruned_utreexo/flat_chain_store.rs
#
#[repr(C)]
#[derive(Debug, Copy, Clone)]
/// To avoid having to sha256 headers every time we retrieve them, we store the hash along with
/// the header, so we just need to compare the hash to know if we have the right header
struct HashedDiskHeader {
    /// The actual header with contextually relevant information
    header: DiskBlockHeader,

    /// The hash of the header
    hash: BlockHash,

    /// Where in the accumulator file this block's accumulator is
    acc_pos: u32,

    /// The length of the block's accumulator
    acc_len: u32,
}
```

Each header can be accessed via simple pointer arithmetics, given the pointer at the start of the file and the desired header index: `header_ptr = start_ptr + index * sizeof(HashedDiskHeader)`. Depending on the header chain status (mainchain or fork), we will store the header on either `headers.bin` or `fork_headers.bin`.

- In the `headers.bin` file, the file index is the same as the height.
- However, in `fork_headers.bin`, the file index is different from the height, as the fork block headers can be from multiple chains.

#### Blocks Index

Then we have the blocks index, the persistent block hash → index hash-map. We use a short (non-cryptographic) hash function to map a `BlockHash` to a `u32` index. Any collision is solved by looking up the `HashedDiskHeader` with the found index, and checking whether the stored hash is the hash we want. Otherwise, if the found index is not the desired one and is not a vacant position, we linearly probe the next position.

```rust
# // Path: floresta-chain/src/pruned_utreexo/flat_chain_store.rs
#
# /// Returns the position inside the hash map where a given hash should be
# ///
# /// This function computes the short hash for the block hash and looks up the position inside
# /// the index map. If the found index fetches the header we are looking for, return this bucket.
# /// Otherwise, we continue incrementing the short hash until we either find the record or a
# /// vacant position. If you're adding a new entry, call this function (it will return a vacant
# /// position) and write the height there.
# unsafe fn hash_map_find_pos(
    # &self,
    # block_hash: BlockHash,
    # get_header_by_index: impl Fn(Index) -> Result<HashedDiskHeader, FlatChainstoreError>,
# ) -> Result<IndexBucket, FlatChainstoreError> {
    # let mut hash = Self::index_hash_fn(block_hash) as usize;
    #
    # // Retrieve the base pointer to the start of the memory-mapped index
    # let base_ptr = self.index_map.as_ptr() as *mut Index;
    #
    # // Since the size is a power of two `2^k`, subtracting one gives a 0b111...1 k-bit mask
    # let mask = self.index_size - 1;
    #
    # for _ in 0..self.index_size {
        # // Obtain the bucket's address by adding the masked hash to the base pointer
        # // SAFETY: the masked hash is lower than the `index_size`
        # let entry_ptr = base_ptr.add(hash & mask);
        #
        # // If this is the first time we've accessed this pointer, this candidate index is 0
        # let candidate_index = *entry_ptr;
        // ...

        // If the header at `candidate_index` matches `block_hash`, this is the target bucket
        let file_header = get_header_by_index(candidate_index)?;
        if file_header.hash == block_hash {
            return Ok(IndexBucket::Occupied {
                ptr: entry_ptr,
                header: file_header.header,
            });
        }

        // If we find an empty index, this bucket is where the entry would be added
        // Note: The genesis block doesn't reach this point, as its header hash is matched
        if candidate_index.is_empty() {
            return Ok(IndexBucket::Empty { ptr: entry_ptr });
        }

        // If no match and bucket is occupied, continue probing the next bucket
        hash = hash.wrapping_add(1);
        // ...
    # }
    #
    # // If we reach here, it means the index is full. We should re-hash the map
    # Err(FlatChainstoreError::IndexIsFull)
# }
```

Thanks to the blocks index, we can fetch a header via its hash (hash → height → header):
1. We get the index for that `BlockHash`, which is tagged to indicate whether the block is mainchain or in a fork.
2. Then, we use the index to fetch the corresponding header from `headers.bin` or `fork_headers.bin`.

In fact, since the hash-map fetches the file `HashedDiskHeader` to verify this index is the correct one, and not a collision, the hash → height function returns the header "for free."

#### Accumulators and Metadata

The `accumulators.bin` file follows a similar approach as the two header files, except each accumulator has a variable length encoding. We have a distinct accumulator after processing each block, so we can store all of them sequentially and, in the rare case of a reorg, roll back to a previous accumulator (i.e., the accumulator for the fork point). We reorg our accumulator state simply by truncating the `accumulators.bin` file, and then continue appending the new block accumulators.

Because file access is only required for loading the last accumulator state (at node startup) and for reorging the chain, we don't really need to memory-map this file.

To be more precise, storing all the accumulators is not required nor ideal, especially for ancient blocks that become harder and harder to reorg. The optimal solution here is a sparse forest, where we keep fewer and fewer accumulators as we go back in block height. We only keep all the accumulators for the last few blocks because deeper reorgs are exponentially less likely. In the rare case of a deep reorg, we would still be able to recover the fork point accumulator by re-processing the blocks after the last accumulator that we have.

Finally, we have the `metadata.bin` file, which stores the serialized `Metadata` struct. This metadata contains the `FlatChainStore` magic number and version, the allocated lengths for the rest of files, and stuff like a checksum for them, to verify integrity each time we load the store.

### LRU Cache

Furthermore, to speed up repetitive header lookups, `FlatChainStore` maintains a least-recently-used (LRU) cache that maps block hashes to headers for quick access. The default size for the cache is 10,000 block headers.

Below is the actual type definition, where `MmapMut` is the mutable memory-mapped file type from the [`memmap2` crate](https://crates.io/crates/memmap2), `BlockIndex` is a wrapper over the memory-mapped index file (that handles the block hash → index mapping), and `LruCache` comes from the [`lru` crate](https://crates.io/crates/lru).

```rust
# // Path: floresta-chain/src/pruned_utreexo/flat_chain_store.rs
#
pub struct FlatChainStore {
    /// The memory map for our headers
    headers: MmapMut,

    /// The memory map for our metadata
    metadata: MmapMut,

    /// The memory map for our block index
    block_index: BlockIndex,

    /// The memory map for our fork files
    fork_headers: MmapMut,

    /// The file containing the accumulators for each blocks
    accumulator_file: File,

    /// A LRU cache for the last n blocks we've touched
    cache: Mutex<LruCache<BlockHash, DiskBlockHeader>>,
}
```

### How big can it get? *(Spoiler: comfortably under 1GB)*

Below we calculate the worst case size scenario that we could reach in a few decades.

| File               | Grows with                                             | Size                          | Why that cap is safe until ~2055                                                                           |
|--------------------|--------------------------------------------------------|-------------------------------|------------------------------------------------------------------------------------------------------------|
| `headers.bin`      | 1×`HashedDiskHeader`/block                             | **320MiB** (2.5M × 128B)      | 2.5M blocks reached by the year ~2055; if we need more capacity we just mmap a bigger sparse file.         |
| `blocks_index.bin` | Fixed bucket array                                     | **40MiB** (10M × 4B)          | Load factor ~0.3 even at 3M blocks. Should we approach 0.7 we trigger a single re‑hash into a larger map.  |
| `accumulators.bin` | 1×(roots blob and a `u64` leaves count) each 32 blocks | **80MiB** (2.5M / 32 × 1032B) | Assumes a mean of 32 roots (absolute worst case is 64 roots) *and* keeping one accumulator each 32 blocks. |
| `fork_headers.bin` | 1×`HashedDiskHeader`/fork block                        | **2MiB** (16k × 128B)         | Plenty of room for fork headers storage; we can prune fork headers that are deep enough.                   |

**Real‑world footprint as of July 2025**: **177MiB**.

> **Bottom‑line:** The worst case for the next few decades is around 442MiB, meaning we can assume less than 500MiB for the foreseeable future – well below the RAM of any budget laptop.
