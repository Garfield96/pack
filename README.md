# Pack

> **Warning**: This package manager is experimental, and it should exclusively be used within the provided Docker image. Usage outside a sandboxed environment can lead to **system corruption**.

## Build Docker Image
```sh
docker build -t pack docker/
```
## Run Docker
```sh
docker run --rm -it -v $(pwd):/pack pack
```

## Populate DB

To familiarize this package manager with the packages already installed on the system, various files must be imported:
```sh
cargo run -- populate /var/lib/dpkg/status
cargo run -- populate -i /var/lib/apt/extended_states
cargo run -- update   # Updates metadata
```
Metadata (available packages) updates are also possible using a local file:
```sh
cargo run -- populate -a <file containing available packages>
```
## Install package
```sh
cargo run -- install <deb package file>
```

## Purge package
```sh
cargo run -- install <deb package name>
```