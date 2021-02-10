FROM ubuntu:groovy as builder
ENV TZ=Europe/Paris

ADD . /src
WORKDIR /src

RUN ln -snf /usr/share/zoneinfo/$TZ /etc/localtime && echo $TZ > /etc/timezone && \
    apt-get update && \
    apt-get install -y \
        clang \
        curl \
        gcc \
        llvm \
        libavcodec-dev \
        libavdevice-dev \
        libavfilter-dev \
        libavformat-dev \
        libavresample-dev \
        libavutil-dev \
        libclang1 \
        libpython3.8 \
        libssl-dev \
        pkg-config \
        python3 \
        && \
    ln -s /usr/lib/x86_64-linux-gnu/libpython3.8.so.1.0 /usr/lib/x86_64-linux-gnu/libpython3.8.so && \
    curl https://sh.rustup.rs -sSf | \
    sh -s -- --default-toolchain nightly -y && \
    . $HOME/.cargo/env && \
    cargo build --verbose --release && \
    cargo install --path .

FROM ubuntu:groovy
COPY --from=builder /root/.cargo/bin/transcript_worker /usr/bin

RUN apt update && \
    apt install -y \
    ca-certificates \
    libavcodec58 \
    libavdevice58 \
    libavfilter7 \
    libavformat58 \
    libavresample4 \
    libavutil56 \
    libpython3.8 \
    libssl1.1 \
    python3

ENV AMQP_QUEUE job_transcript
CMD transcript_worker
