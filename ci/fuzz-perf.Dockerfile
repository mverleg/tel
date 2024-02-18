
FROM mverleg/rust_nightly_musl_base:nodeps_2024-02-17_42

# Copy the code (all except .dockerignore).
COPY ./ ./

# Build
RUN find . -name target -prune -o -type f &&\
    touch -c build.rs src/main.rs src/lib.rs &&\
    cargo build --release --all-features --locked

# Update PATH
RUN find . -executable -type f &&\
    PATH=./target/x86_64-unknown-linux-musl/release

# Cli smoke test
RUN cat 'compiler/examples/fizzbuzz.tel' | tel script -i

# Run test module
RUN tel-testing
