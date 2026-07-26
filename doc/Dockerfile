ARG MDBOOK_VERSION=0.4.40

FROM rust:1.83-alpine AS build
ARG MDBOOK_VERSION
# Set to 1 (passed by k8s-deploy.sh --dev) to include the TIPs / draft
# proposals in the build; empty for the public/prod book. See inject-tips.py.
ARG TEL_DEV=""
RUN apk add --no-cache musl-dev python3 \
 && cargo install mdbook --version "${MDBOOK_VERSION}" --locked

WORKDIR /src
COPY book/ book/
WORKDIR /src/book

RUN TEL_DEV="${TEL_DEV}" mdbook build \
 && rm -f out/book.toml out/.gitignore \
 && find out -type d -empty -delete

FROM nginx:1.27-alpine AS serve
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /src/book/out/ /usr/share/nginx/html/
EXPOSE 80
