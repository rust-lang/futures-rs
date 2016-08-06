# futures-minihttp

This crate is an implementation of a vastly simplified version of a "server
framework" to showcase the performance of futures when applied with HTTP. This
is not intended to be a production-ready HTTP framework, but rather just a demo
of what futures can do.

The "hello world" [available in this repository][singlethread] is an
implementation of the [TechEmpower "plaintext" benchmark][techem]. It's also
available in a [multithreaded version][multithread] for those on Unix.

[singlethread]: https://github.com/alexcrichton/futures-rs/blob/master/futures-minihttp/src/bin/singlethread.rs
[techem]: https://www.techempower.com/benchmarks/#section=data-r12&hw=peak&test=plaintext
[multithread-unix]: https://github.com/alexcrichton/futures-rs/blob/master/futures-minihttp/src/bin/multithread-unix.rs

Current HTTP features implemented by this crate are:

* HTTP/1.1 pipelining
* Zero-copy parsing of HTTP requests
* Efficient buffer management of incoming requests and outgoing responses

A table of the current performance numbers in requests/second is available
below, but please keep in mind that like all benchmark numbers these should be
taken with a grain of salt. The purpose there is to show that frameworks
themselves have as little overhead as possible, and some of them can probably
still be further optimized! Any PRs to the implementation are of course quite
welcome!

|   program                     | pipelined  | singlethread, no pipeline |
|-------------------------------|------------|---------------------------|
| [minihttp][multithread-unix]  | 1966297.54 | 127934.89                 |
| [rapidoid (Java)][rapidoid]   | 1701426.67 |       N/A                 |
| [fasthttp (Go)][fasthttp]     | 1489868.35 |  92024.56                 |
| [iron]                        |        N/A | 124592.49                 |
| [hyper]                       |        N/A |  91475.84                 |
| [Go][go-std]                  |  191548.57 |  47585.99                 |
| [node]                        |  131511.36 |  12149.08                 |

[fasthttp]: https://github.com/TechEmpower/FrameworkBenchmarks/tree/master/frameworks/Go/fasthttp
[hyper]: https://github.com/aturon/async-benches/blob/master/techempower-6/hyper-master/src/main.rs
[iron]: https://github.com/aturon/async-benches/blob/master/techempower-6/iron/src/main.rs
[rapidoid]: https://github.com/TechEmpower/FrameworkBenchmarks/tree/master/frameworks/Java/rapidoid
[go-std]: https://github.com/TechEmpower/FrameworkBenchmarks/tree/master/frameworks/Go/go-std
[node]: https://github.com/TechEmpower/FrameworkBenchmarks/tree/master/frameworks/JavaScript/nodejs

The benchmark for minihttp is for the `multithread-unix.rs` script for the first
column and the `singlethread` for the second. The numbers were all collected on
Linux Ubuntu 8-core machine. Note that absolute numbers should be taken with a
grain of salt, but relative numbers should be fairly consistent across setups.

The command to generate these numbers was:

```
wrk --script ./pipelined_get.lua \
  --latency -d 30s -t 40 -c 760 \
  http://127.0.0.1:8080/plaintext -- $pipeline
```

For the pipelined column the value of `$pipeline` was 32, and for the
singlethread no pipeline column it was 1.

Also note that iron/hyper currently have a bug with pipelining, explaining the
N/A, and I couldn't figure out how to get rapidoid to run on one thread,
explaining that N/A.
