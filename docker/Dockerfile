FROM ubuntu:latest

RUN apt-get -y update
RUN apt-get -y upgrade
RUN apt-get install -y --no-install-recommends curl build-essential sqlite3 libsqlite3-dev libssl-dev ca-certificates
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
