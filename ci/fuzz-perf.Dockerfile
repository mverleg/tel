
FROM mverleg/rust_nightly_musl_base:nodeps_2024-02-17_42

# Copy the code (all except .dockerignore).
COPY ./ ./

# Build
RUN find . -name target -prune -o -type f &&\
    touch -c build.rs src/main.rs src/lib.rs &&\
    cargo build --release --all-features --locked

# Run test module
RUN find . -executable -type f &&\
    ./target/x86_64-unknown-linux-musl/release/tel-testing
