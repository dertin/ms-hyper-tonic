export JEMALLOC_SYS_WITH_MALLOC_CONF="background_thread:true,narenas:1,tcache:false,dirty_decay_ms:0,muzzy_decay_ms:0,abort_conf:true" 

valgrind --leak-check=full --show-leak-kinds=all -s ./target/debug/ms-worker