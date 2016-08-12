# techempower1

An implementation of the TechEmpower [JSON serialization][bench] benchmark. This
simple HTTP server simply creates an HTTP response from a JSON document and
sends it back to the client.

[bench]: https://www.techempower.com/benchmarks/#section=data-r12&hw=peak&test=json

First, run the server

```
$ cargo run --release
```

Then, make a connection

```
$ curl http://localhost:8080/json
```
