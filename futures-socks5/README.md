# futures-socks5

An implementation of a SOCKSv5 proxy server built on top of `futures-mio`.

[![Build Status](https://travis-ci.org/alexcrichton/futures-rs.svg?branch=master)](https://travis-ci.org/alexcrichton/futures-rs)
[![Build status](https://ci.appveyor.com/api/projects/status/yl5w3ittk4kggfsh?svg=true)](https://ci.appveyor.com/project/alexcrichton/futures-rs)

## Usage

First, run the server

```
$ cargo run
   ...
Listening for socks5 proxy connections on 127.0.0.1:8080
```

Then in a separate window you can test out the proxy:

```
$ export https_proxy=socks5h://localhost:8080
$ curl -v https://www.google.com
```

# License

`futures-socks5` is primarily distributed under the terms of both the MIT
license and the Apache License (Version 2.0), with portions covered by various
BSD-like licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.

