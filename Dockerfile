FROM 192.168.31.96:5000/base/rust:1.79 as builder
WORKDIR /usr/src/cap-warm
COPY . .
RUN cargo build --release

FROM 192.168.31.96:5000/base/debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/cap-warm/target/release/cap-warm /usr/local/bin/cap-warm
ENV RUST_LOG=info
EXPOSE 3000
CMD ["cap-warm"]
