# ssl-example

A small example of how to host a server which accepts SSL connections. Currently
this example is only compiled against OpenSSL, but other SSL libraries (like
SecureTransport) are also supported.

First, run the server

```
$ cargo run
```

Then, make a connection

```
$ curl --cacert src/server.crt https://localhost:8080/plaintext
```
