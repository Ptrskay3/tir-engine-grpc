# TIR Engine gRPC wrapper

### Be advised!

This is just experimental, and relies on a fork of `tir-engine`.

## Setup

Make sure you pull the `tir-engine` as a git submodule after cloning this repository:

```sh
git submodule init
git submodule update --remote --recursive --no-single-branch
```

For gRPC, you'll need a protobuf compiler:

```sh
sudo apt update && sudo apt upgrade -y
sudo apt install -y protobuf-compiler libprotobuf-dev
```

Run as normal with:

```sh
cargo r
```

It'll expose a gRPC server at localhost:50051.

## Docker:

```sh
docker build -t <user_name>/<image_name> -f Dockerfile  .
docker run --network host --env OPENAI_SK=<secret> --env PORT=50052 <user_name>/<image_name>
```

where `OPENAI_SK` is required, the `PORT` is optional, and defaults to 50051.
