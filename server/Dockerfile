# Wildforge game server. Build from the repository root so content/ is in
# the context:
#
#   docker build -f server/Dockerfile -t wildforge-server .
#   docker run -p 7777:7777 -v wildforge-data:/data wildforge-server

FROM golang:1.27-alpine AS build
WORKDIR /src
COPY server/go.mod server/go.sum ./
RUN go mod download
COPY server/ ./
RUN CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o /out/wildforge-server ./cmd/wildforge-server \
 && mkdir -p /out/data

FROM gcr.io/distroless/static-debian12:nonroot
LABEL org.opencontainers.image.title="wildforge-server" \
      org.opencontainers.image.description="Authoritative game server for Wildforge" \
      org.opencontainers.image.source="https://github.com/villetacore/wildforge" \
      org.opencontainers.image.licenses="MIT"
WORKDIR /app
COPY --from=build /out/wildforge-server /app/wildforge-server
COPY content/ /app/content/
COPY --from=build --chown=65532:65532 /out/data /data
ENV WILDFORGE_ADDR=:7777 \
    WILDFORGE_CONTENT=/app/content \
    WILDFORGE_DATA=/data/users.json
VOLUME ["/data"]
EXPOSE 7777
USER nonroot:nonroot
ENTRYPOINT ["/app/wildforge-server"]
