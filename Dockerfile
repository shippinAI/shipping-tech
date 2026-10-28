FROM --platform=$BUILDPLATFORM docker.io/lukemathwalker/cargo-chef:latest-rust-slim-trixie AS chef
WORKDIR /build

FROM --platform=$BUILDPLATFORM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path /recipe.json

FROM --platform=$BUILDPLATFORM chef AS builder
ARG TARGETPLATFORM
RUN case "${TARGETPLATFORM}" in \
    "linux/arm64") echo "aarch64-unknown-linux-gnu" > /target.txt && echo "-C linker=aarch64-linux-gnu-gcc" > /flags.txt ;; \
    "linux/amd64") echo "x86_64-unknown-linux-gnu" > /target.txt && echo "-C linker=x86_64-linux-gnu-gcc" > /flags.txt ;; \
    *) exit 1 ;; \
    esac
RUN export DEBIAN_FRONTEND=noninteractive && \
    apt-get update && \
    apt-get install -yq --no-install-recommends build-essential libclang-19-dev \
    g++-aarch64-linux-gnu binutils-aarch64-linux-gnu \
    g++-x86-64-linux-gnu binutils-x86-64-linux-gnu
RUN rustup target add "$(cat /target.txt)"
COPY --from=planner /recipe.json /recipe.json
RUN RUSTFLAGS="$(cat /flags.txt)" cargo chef cook --target "$(cat /target.txt)" --release --no-default-features --features "sqlite postgres mysql rocks s3 redis azure nats enterprise" --recipe-path /recipe.json
COPY . .
RUN RUSTFLAGS="$(cat /flags.txt)" cargo build --target "$(cat /target.txt)" --release -p shippin-tech --no-default-features --features "sqlite postgres mysql rocks s3 redis azure nats enterprise"
RUN mv "/build/target/$(cat /target.txt)/release" "/output"

FROM docker.io/debian:trixie-slim
RUN export DEBIAN_FRONTEND=noninteractive && \
    apt-get update && \
    apt-get install -yq --no-install-recommends ca-certificates curl libcap2-bin && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd -r -g 2000 shippin.tech && \
    useradd -r -u 2000 -g 2000 -s /usr/sbin/nologin -M shippin.tech && \
    mkdir -p /etc/shippin.tech /var/lib/shippin.tech && \
    chown shippin.tech:shippin.tech /etc/shippin.tech /var/lib/shippin.tech
COPY --from=builder --chmod=0755 /output/shippin-tech /usr/local/bin/shippin.tech
RUN setcap 'cap_net_bind_service=+ep' /usr/local/bin/shippin.tech
USER shippin.tech
WORKDIR /var/lib/shippin.tech
VOLUME ["/etc/shippin.tech", "/var/lib/shippin.tech"]
EXPOSE	443 25 110 587 465 143 993 995 4190 8080
ENV shippin.tech_HEALTHCHECK_URL=https://127.0.0.1:443/healthz/live
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -fsSk -H "X-Forwarded-For: 127.0.0.1" "$shippin.tech_HEALTHCHECK_URL" || curl -fsS -H "X-Forwarded-For: 127.0.0.1" http://127.0.0.1:8080/healthz/live || exit 1
ENTRYPOINT ["/usr/local/bin/shippin.tech"]
CMD ["--config", "/etc/shippin.tech/config.json"]
