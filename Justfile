default_path := 'data/measurements.txt'
default_num := '1_000_000_000' 

test_path := 'data/10m_measurements.txt'
test_num := '10_000_000'

build BIN:
    RUSTFLAGS="-C target-feature=+aes -C target-feature=+avx2" cargo build --release --bin={{BIN}}

generate NUM=default_num PATH=default_path:
    RUSTFLAGS="-C target-feature=+aes" cargo run --release --bin=create_measurements {{NUM}} > {{PATH}}

run BIN PATH=default_path: 
    RUSTFLAGS="-C target-feature=+avx2" cargo run --release --bin={{BIN}} {{PATH}}

flamegraph BIN: (generate test_num test_path)
    RUSTFLAGS="-C target-feature=+avx2" cargo flamegraph --output=profiling/flamegraph_{{BIN}}.svg --release --bin={{BIN}} -- {{test_path}} 1> /dev/null

bench BIN NUM=test_num DATA=test_path: (generate NUM DATA) (build BIN)
    hyperfine --warmup=5 './target/release/{{BIN}} {{DATA}} 1> /dev/null'

callgrind BIN: (build BIN)
    valgrind \
        --tool=callgrind \
        --callgrind-out-file=./profiling/callgrind_{{BIN}}.out \
        --collect-jumps=yes \
        --collect-systime=yes \
        --branch-sim=yes \
        --simulate-cache=yes \
        ./target/release/{{BIN}} \
        "$PWD/{{test_path}}"
    rm -f ./profiling/callgrind_{{BIN}}_demangled.out
    rustfilt -i ./profiling/callgrind_{{BIN}}.out -o ./profiling/callgrind_{{BIN}}_demangled.out

