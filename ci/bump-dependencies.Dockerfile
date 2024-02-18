
# GENERATED: This file is automatically updated by 'Bump dependencies', local changes will be overwritten!

FROM mverleg/rust_nightly_musl_base:nodeps_2024-02-17_42

COPY ./ ./

RUN cargo upgrade && cargo update

RUN cargo --offline outdated --exit-code 1
