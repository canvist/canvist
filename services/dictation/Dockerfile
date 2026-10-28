# The canvist dictation relay: the `canvist-dictation` binary on a small
# Debian base.
#
# Build from the repository root, which the workspace needs:
#
#   docker build -f services/dictation/Dockerfile -t canvist-dictation .

FROM rust:1.96.0-slim-trixie AS build
WORKDIR /src
# The image's toolchain is the one `rust-toolchain.toml` pins; naming it
# stops rustup from installing the toolchain file's editor components.
ENV RUSTUP_TOOLCHAIN=1.96.0 \
	CARGO_TERM_COLOR=never \
	CARGO_INCREMENTAL=0
COPY . .
RUN cargo build --release --locked -p canvist_dictation_relay --bin canvist-dictation \
	&& cp target/release/canvist-dictation /usr/local/bin/canvist-dictation \
	&& rm -rf target

FROM debian:trixie-slim
# Root certificates for the speech providers and polish models.
RUN apt-get update \
	&& apt-get install -y --no-install-recommends ca-certificates \
	&& rm -rf /var/lib/apt/lists/* \
	&& useradd --system --uid 10001 --no-create-home canvist
COPY --from=build /usr/local/bin/canvist-dictation /usr/local/bin/canvist-dictation
USER canvist
ENV CANVIST_LOG_FORMAT=json \
	PORT=8080
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/canvist-dictation"]
