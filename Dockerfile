FROM rust:1.82 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p three_dcf_service

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/three_dcf_service /usr/local/bin/3dcf-service
EXPOSE 8000
ENV BIND_ADDR=0.0.0.0:8000
CMD ["/usr/local/bin/3dcf-service"]
