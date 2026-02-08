# Profiling the Tel Compiler

This document describes how to generate flamegraphs and profile the Tel compiler.

## Quick Summary

The benchmarks show excellent performance:
- **75,000 functions**: 1.26s
- **107,000 functions**: 1.82s
- **150,000 functions**: 2.58s

## Generating Flamegraphs

### Method 1: Using perf (Linux only, requires permissions)

1. **Set permissions** (one-time setup):
   ```bash
   # Temporary (until reboot):
   sudo sysctl kernel.perf_event_paranoid=-1

   # Permanent:
   echo 'kernel.perf_event_paranoid=-1' | sudo tee -a /etc/sysctl.conf
   ```

2. **Build and profile**:
   ```bash
   cd sandbox
   cargo build --example profile_run --release
   perf record -F 99 -g --call-graph dwarf ./target/release/examples/profile_run
   ```

3. **Generate flamegraph**:
   ```bash
   # Install flamegraph tools
   git clone https://github.com/brendangregg/FlameGraph

   # Convert perf data to flamegraph
   perf script | ./FlameGraph/stackcollapse-perf.pl | ./FlameGraph/flamegraph.pl > flamegraph.svg
   ```

4. **View**:
   ```bash
   firefox flamegraph.svg
   # or
   google-chrome flamegraph.svg
   ```

### Method 2: Using samply (easier, cross-platform)

1. **Install samply**:
   ```bash
   cargo install samply
   ```

2. **Profile**:
   ```bash
   cd sandbox
   samply record cargo run --example profile_run --release
   ```

   This will automatically open a browser with an interactive flamegraph!

### Method 3: Using cargo-flamegraph

1. **Install**:
   ```bash
   cargo install flamegraph
   ```

2. **Generate flamegraph**:
   ```bash
   cd sandbox
   cargo flamegraph --example profile_run -o flamegraph.svg
   ```

3. **View**:
   ```bash
   firefox flamegraph.svg
   ```

## Profile Target

The `profile_run` example runs 10 iterations of compiling a 107k function project (7k base, 30k mid, 70k leaf functions) with realistic dependency structure.

## What to Look For

When analyzing flamegraphs, look for:

1. **Hotspots** - Wide bars indicate where time is spent
2. **Parse/Resolve/Execute phases** - Should see the 3-phase pipeline
3. **DashMap operations** - Concurrent hashmap for dependency tracking
4. **Async-lazy initialization** - Single initialization of shared dependencies
5. **File I/O** - Reading .telsb files (likely bottleneck)

## Expected Bottlenecks

Based on benchmark results, the primary bottleneck is **file generation**, not compilation:
- Generating 150k temp files takes ~2.5s
- Actual compilation is very fast due to excellent caching

For profiling compilation itself (not file I/O), consider using in-memory file generation or caching generated projects.

## Benchmark HTML Reports

Detailed criterion benchmark reports are available at:
```
target/criterion/compile_project/report/index.html
```

These include:
- Time series plots
- Distribution violin plots
- Regression analysis
- Comparison with previous runs
