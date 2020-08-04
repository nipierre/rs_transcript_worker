FROM rust:1.45-stretch as builder

ADD . /src
WORKDIR /src

RUN apt-get update && \
    apt-get install -y libssl-dev && \
    rustup toolchain install nightly && \
    rustup default nightly && \
    cargo build --verbose --release && \
    cargo install --path .

FROM debian:stretch
COPY --from=builder /usr/local/cargo/bin/tranfer_worker /usr/bin

RUN apt update && apt install -y libssl1.1 ca-certificates

ENV AMQP_QUEUE job_transcript
CMD transcript_worker
