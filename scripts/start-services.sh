./killall.sh

cargo clean && cargo build

./target/debug/ms-worker &
./target/debug/ms-executor

# sudo apt install smem
# watch smem -t -k -P "^./target/debug/ms-executor"
# watch smem -t -k -P "^./target/debug/ms-worker"
