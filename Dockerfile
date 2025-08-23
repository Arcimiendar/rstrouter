FROM rust:1.89-trixie AS build
 
RUN cargo install cargo-deb

WORKDIR /app

COPY . .

RUN cargo build

FROM build AS release

RUN cargo deb

FROM debian:trixie

WORKDIR /app
RUN mkdir /DSL


COPY --from=release /app/target/debian/* .

RUN dpkg -i  *.deb

CMD ["rstrouter"]
