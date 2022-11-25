# solana-staking

cargo build-bpf

solana program deploy target/deploy/staking.so

program_id => set to the tests/test.rs

cargo test-bpf -- --nocapture
