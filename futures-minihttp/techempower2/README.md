# techempower2

An implementation of the TechEmpower [Single query][bench] benchmark. This
simple HTTP server generates a random number, looks up that row in a database,
and then renders the row as JSON.

[bench]: https://www.techempower.com/benchmarks/#section=data-r12&hw=peak&test=db

First you'll need to prepare a Postgres database by running the
[create-postgres-database.sql](https://github.com/TechEmpower/FrameworkBenchmarks/blob/master/config/create-postgres-database.sql)
followed by the
[create-postgres.sql](https://github.com/TechEmpower/FrameworkBenchmarks/blob/master/config/create-postgres.sql)
file.

After, run the server

```
$ cargo run --release
```

Then, make a connection

```
$ curl https://localhost:8080/db
```
