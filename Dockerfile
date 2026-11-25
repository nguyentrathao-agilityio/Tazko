FROM node:24-bookworm-slim AS web
RUN npm install --global pnpm@11.1.1
WORKDIR /src
COPY design/tokens.css design/tokens.css
COPY web/package.json web/pnpm-lock.yaml web/
RUN cd web && pnpm install --frozen-lockfile
COPY web web
RUN cd web && pnpm run build

FROM rust:1-bookworm AS server
WORKDIR /src
COPY . .
COPY --from=web /src/web/dist web/dist
ENV TAZKO_SKIP_WEB_BUILD=1
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --home-dir /var/lib/tazko --shell /usr/sbin/nologin tazko
COPY --from=server /src/target/release/tazko /usr/local/bin/tazko
COPY --chmod=755 deploy/railway/start.sh /usr/local/bin/tazko-start
ENV TAZKO_HOST=::
EXPOSE 8791
CMD ["/usr/local/bin/tazko-start"]
