FROM rust:1.75-slim-bullseye
WORKDIR /app

RUN ls -a /root

RUN touch /root/.dotfile \
    touch /root/.secret \
    && echo "secret" > /root/.secret \
    && mkdir /root/.config \
    && mkdir /root/.config/dotfolder \
    && touch /root/.config/dotfile

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/home/root/app/target

COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./src ./src
COPY ./target/CACHEDIR.TAG ./target/CACHEDIR.TAG

RUN cargo install cargo-nextest --locked
RUN cargo build

CMD cargo nextest run --cargo-quiet
