FROM lukemathwalker/cargo-chef:latest-rust-1.72.0 AS chef
WORKDIR /app

FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path chef-recipe.json

FROM chef as builder
# Kind of unfortunate, but we must copy this local dependency until it's properly released
COPY --from=planner /app/tir-engine tir-engine
COPY --from=planner /app/chef-recipe.json chef-recipe.json
RUN cargo chef cook --release --recipe-path chef-recipe.json
COPY . .
RUN apt-get update -y && apt-get install -y --no-install-recommends protobuf-compiler
RUN cargo build --release

FROM ubuntu:22.04 as runtime
ARG PORT=50051
ARG OPENAI_SK
WORKDIR /app
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends wget ca-certificates openssl \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/* 
    
COPY --from=builder /app/target/release/tir-engine-grpc /usr/local/bin
ENV PORT $PORT
ENV OPENAI_SK $OPENAI_SK
ENTRYPOINT ["/usr/local/bin/tir-engine-grpc"]
