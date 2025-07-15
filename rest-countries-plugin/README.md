# Rest Countries Cache

Throughput cache for countries via a RESTful API

## How to build
```
cargo build
```

## How to run
Use [cargo-pike](https://git.picodata.io/picodata/plugin/cargo-pike)
```
cargo pike run --topology topology.toml --data-dir ./tmp --picodata-path /home/gs0xa19f2/Documents/PicodataAndRust/picodata-25.2.3/target/debug/picodata
```

## Config
### TTL
Specifies a number of seconds before record is expired and deleted

### Timeout
Specifies a number of second for HTTP call to countries
