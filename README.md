Proof of concept using Hyper.rs as http server and Tonic gRPC for communication between microservices.

![alt request flow](request-flow.drawio.svg?raw=true "request flow")


Stress test to analyze memory leak or memory fragmentation in Hyper and Tonic services:

```
./start-services.sh
./run-stress-test.sh
```

use valgrind => 3.20.0
```
./killall.sh
./valgrind-executor.sh
./valgrind-worker.sh

```