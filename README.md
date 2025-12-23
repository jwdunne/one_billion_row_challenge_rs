# 1BRC in Rust

[1BRC repository](https://github.com/gunnarmorling/1brc) - [Blog post](https://www.morling.dev/blog/one-billion-row-challenge/) 

> The challenge: compute the min, mean and max over 1 billion rows, as fast as possible, without dependencies.

An implementation of the 1 billion row challenge in Rust. Although some experiments use external dependencies, these are to inform the final implementations.

## Running the challenge

This project uses:

- [just](https://github.com/casey/just) 
- [hyperfine](https://github.com/sharkdp/hyperfine) 
- [flamegraph](https://github.com/flamegraph-rs/flamegraph)

These tools are not required to generate data and run an attempt. To do so without `just`, read the `Justfile`.

### Generating data

Generate 1 billion rows of data by running:

```
$ just generate
```

To generate smaller data files, `just generate` accepts arguments:

```
$ just generate 10_000_000 data/10m_measurements.txt
```

### Running attempts

To run an attempt on 1 billion rows by default:

```
$ just run naive_1
```

To run an attempt on a smaller data file:

```
$ just run naive_1 data/10m_measurements.txt
```

### Benchmarking

To benchmark an attempt, using 10,000,000 rows:

```
$ just benchmark naive_1
```

### Flamegraphs

To generate a flamegraph in `profiling/flamegraph_{{attempt}}.svg`:

```
$ just flamegraph naive_1
```

## Results

### 1. Naive

| | |
| -- | -- |
| Binary | `naive` |
| Mean running time (10m) | 1.350s (+/- 0.021s) | 

Straight forward implementation using `std::collections::HashMap` and a `Stats` record keeping ongoing stats, with the final mean computed at the end.

Surprisingly, I/O is not the bottleneck. It's the `HashMap`, with call to `HashMap::get_mut` taking up 45.7% of the running time. Cutting this down is our main objective for now.

### 2. `hashbrown`

| | |
| -- | -- |
| Binary | `hashbrown` |
| Mean running time (10m) | 1.096s (+/- 0.010s) |

Rust uses `hashbrown` under the hood, a Rust implementation of Swiss Tables. Despite this, Rust does not provide the `HashMap::entry_ref` API, which allows us to look up a key by `&str`, and make modifications. 

The `hashbrown` crate does provide this - switching to `HashMap::entry_ref` led to a significant improvement in running time.

The flame graph, however, shows we're still spending 41% of our running time on `HashMap::entry_ref`, with ~33% spent comparing slices. 

### 3. Buffering I/O (stack-allocated)

| | |
| -- | -- |
| Binary | `io_stack_buffer` |
| Mean running time (10m) | 815.7ms (+/- 11.3ms) |

We were spending almost 30% of the running time reading the input file line-by-line. Instead, we use a 4MB stack-allocated buffer. This led to a significant drop in total running time, with ~12% of the running time spent on iterating the input file. The logical conclusion of this approach is, of course, memory mapping the entire file. But there are other opportunities in the meantime.

We're now spending 11% of the time converting to a UTF8 string, and then 9.4% of the time on splitting into name and temperature parts. On top of this, we spend 10.5% of the time parsing the temperature string into a float. In total, ~31% is spent on this.

We could also experiment with a larger, heap-allocated buffer to see if we can reduce the 12% further ahead of memory mapping.

### 4. Optimised parsing

| | |
| -- | -- |
| Binary | `parsing` |
| Mean running time (10m) | 452.3ms (+/- 6.0ms) |

Parsing to a UTF-8 string, and then parsing an `f64` from this, is slow. We eliminate this by using a vector of bytes as `HashMap` keys directly (using a slice with `hashbrown::HashMap::entry_mut` so a `Vec` is only allocated on new entries).

We also eliminate parsing to floating point entirely by parsing the bytes to integers multiplied by 10, given all readings are at most 3 digits with a single decimal place.

The flame graph still shows a significant amount of time spent on `HashMap` lookups (~52%). In later attempts, we should explore tries, especially cache-friendly options such as HAT-tries.

In the mean time, there are other, more straight-forward opportunities:

- ~20% of running time spent on splitting lines in the buffer
- ~13% of running time spent finding the `;` byte in each line

Instead, we could process the entire buffer as a stream:

1. Read until `;` byte, which becomes the key
2. Read until `\n` byte, which becomes the reading

### 5. Stream processing

| | |
| -- | -- |
| Binary | `streaming` |
| Mean running time (10m) | 375ms (+/- 4.5ms) |

Splitting the read buffer into lines, and then scanning each line one byte at a time was taking up around 27% of the running time. Implementing a function optimised for searching bytes, 8 at a time using SWAR, and using it to scan for newlines reduced this down to 16% of the total running time - a modest improvement.

With `HashMap` lookups taking a majority of the time, this must our next focus.

### 6. Custom hash table

| | |
| -- | -- |
| Binary | `custom_hash_table` |
| Mean running time (10m) | 380ms (+/- 3.8ms) |

Up to now, `hashbrown::HashMap` has worked fine. But the time spent on lookups continued to dominate throughout. Implementing a custom hash table using open addressing, with algorithms tuned for the problem/inputs, achieves comparable performance overall whilst significantly reducing the time spent on looking up keys (30% vs 52%).

This involved:

- Designing an API to minimise repeat work in the hot loop rather than for generality
- Implementing a hash function optimised for our list of names
- Designing a cache-friendly structure for entry metadata and statistics
- Storing names separately since we don't need them

This gives us room to experiment with other improvements:

- Finding positions of `b';'` and `b'\n'` bytes
- Parsing temperatures to integers
- Memory-mapping the entire file
- Parallelisation across cores
- Instruction-level parallelisation 

### 7. Optimised row reading

| | |
| -- | -- |
| Binary | `reading_rows` |
| Mean running time (10m) | 320ms (+/- 2.3ms) |

The hot loop read lines one at a time, using an optimised implementation of `position`, which searched bytes 8 at a time. Instead, we can identify multiple lines and semicolon separators in one go. This is pretty fast using the SWAR technique we used in `ByteBuffer::byte_position`, but for CPUs with AVX2 support, this is much faster and simpler. With AVX2 intrinsics, identifying line and field separators drops from 25% of the running time, to 3%.

Although we haven't, this gives us freedom to re-order operations for batching e.g:

1. Read multiple lines
2. Hash multiple names
3. Prefetch multiple slots
4. Look up multiple entries
5. Update multiple entries

### 8. Branching (or lack thereof)

| | |
| -- | -- |
| Binary | `branching` |
| Mean running time (10m) | 245ms (+/- 1.6ms) |

The flamegraph showed we still spent a significant amount of time:

1. Computing prefixes
2. Computing hashes
3. Parsing temperatures

Looking at the profiler, we could see a large number of branch mispredictions. We fixed this by eliminating branches from computing the prefix, and doing the same for temperature parsing (thanks to [artsiomkorzun](https://github.com/gunnarmorling/1brc/blob/main/src/main/java/dev/morling/onebrc/CalculateAverage_artsiomkorzun.java)).

For the prefix, we are only ever interested in the first 8 bytes, so we can use that knowledge to construct a mask for the only the bits we're interested in, rather than constructing this via branching. 

The temperature parsing is an incredibly clever piece of code, which I shamelessly lifted from artsiomkorzun (I couldn't come up with on my own). My first attempt branchlessly constructed the temperature int via array access. My second used bit twiddling entirely, although this was ugly and inelegant, with lots of variable shifts, harming performance. The trick used by artsiomkorzun achieves this with zero branching and a very small number of variable bit shifts.

Although impossible to eliminate all branching from the hash computation, we significantly reduced mispredictions by folding 8 byte or smaller and 16 byte or smaller paths into a single path, using bit masking to conditionally mix the suffix without branching.

### 9. Batching

| | |
| -- | -- |
| Binary | `batching` |
| Mean running time (10m) | 236ms (+/- 2.3ms) |

Experimented with processing multiple, independent regions at once, to encourage instruction-level processing. This was a modest success, though we're hitting marginal gains territory right now. I may experiment with the number of interleaved regions - right now it's 4 and it seems we spend roughly half the time in cleanup.

Branching in `Table::lookup` was also eliminated. With the configured table size, probe depth never goes beyond 5.
