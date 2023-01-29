./killall.sh

cargo clean && cargo build

export JEMALLOC_SYS_WITH_MALLOC_CONF="background_thread:true,narenas:1,tcache:false,dirty_decay_ms:0,muzzy_decay_ms:0,abort_conf:true" 

./target/debug/ms-executor &
./target/debug/ms-worker &

# watch smem -t -k -P "^./ms-executor"
watch smem -t -k -P "^./target/debug/ms-worker"
