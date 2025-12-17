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

| Binary | `naive_1` |
| -- | -- |
| Mean running time | 1.350s (+/- 0.021s) | 

Straight forward implementation using `std::collections::HashMap` and a `Stats` record keeping ongoing stats, with the final mean computed at the end.

Surprisingly, I/O is not the bottleneck. It's the `HashMap`, with call to `HashMap::get_mut` taking up 41% of the running time. Cutting this down is, then, our main objective for now.
