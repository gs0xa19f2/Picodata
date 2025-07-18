# Rest Countries Cache

Throughput cache for countries via a RESTful API

## How to build
```
cargo build
```

## How to run
Use [cargo-pike](https://git.picodata.io/picodata/plugin/cargo-pike)

Replace `<path-to-picodata>` with the path to your built Picodata binary (for example, `~/Documents/PicodataAndRust/picodata-25.2.3/target/debug/picodata`):

```
cargo pike run --topology topology.toml --data-dir ./tmp --picodata-path <path-to-picodata>
```

## How to run tests

Before running tests, make sure the directory containing the `picodata` binary is in your `PATH`.  
You can do this by exporting the path variable, replacing `<path-to-picodata-dir>` with the directory** (not the binary itself):

```
export PATH="<path-to-picodata-dir>:$PATH"
```
For example:
```
export PATH="$HOME/Documents/PicodataAndRust/picodata-25.2.3/target/debug:$PATH"
```
Then run tests as usual:
```
cargo test
```

## Config
### TTL
Specifies a number of seconds before record is expired and deleted

### Timeout
Specifies a number of seconds for HTTP call to countries
