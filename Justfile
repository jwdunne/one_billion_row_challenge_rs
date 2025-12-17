flamegraph BIN:
    cargo flamegraph --output=profiling/flamegraph_{{BIN}}.svg --release --bin={{BIN}} -- data/10m_measurements.txt 1> /dev/null

bench BIN:
    hyperfine --warmup=3 'cargo run --release --bin={{BIN}} data/10m_measurements.txt 1> /dev/null'
