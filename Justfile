default_path := 'data/measurements.txt'
default_num := '1_000_000_000' 

test_path := 'data/10m_measurements.txt'
test_num := '10_000_000'

build BIN:
    cargo build --release --bin={{BIN}}

generate NUM=default_num PATH=default_path:
    cargo run --release --bin=create_measurements {{NUM}} > {{PATH}}

run BIN PATH=default_path: 
    cargo run --release --bin={{BIN}} {{PATH}}

flamegraph BIN: (generate test_num test_path)
    cargo flamegraph --output=profiling/flamegraph_{{BIN}}.svg --release --bin={{BIN}} -- {{test_path}} 1> /dev/null

bench BIN: (generate test_num test_path) (build BIN)
    hyperfine --warmup=3 './target/release/{{BIN}} {{test_path}} 1> /dev/null'

