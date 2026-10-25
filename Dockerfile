# check=skip=FromPlatformFlagConstDisallowed
# Builds the app for the reMarkable Paper Pro (aarch64 Linux) and runs all tests on Linux.
#
#   docker build --platform linux/arm64 --output type=local,dest=dist .
#
# The result is dist/sudoku/: the AppLoad folder (external.manifest.json, icon.png and the static
# executable `sudoku`). Copy it to /home/root/xovi/exthome/appload/ on the tablet.
FROM --platform=linux/arm64 rust:1.97-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
# Download and compile the dependencies first, so that a change of the source does not do this again.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin && echo 'fn main() {}' > src/bin/sudoku.rs && echo 'fn main() {}' > src/bin/snapshot.rs \
 && echo '' > src/lib.rs && cargo build --release --locked && cargo test --release --locked --no-run \
 && rm -rf src
COPY src src
COPY tests tests
COPY images images
COPY assets assets
COPY INVARIANTS.md INVARIANTS.md
COPY scripts scripts
RUN touch src/lib.rs src/bin/*.rs
# All tests must pass on Linux. Then make the static executable.
RUN cargo test --release --locked
# The debug build checks all state rules of INVARIANTS.md after each input (invariants::check_app).
RUN cargo test --locked
RUN cargo build --release --locked --bin sudoku \
 && file_info="$(head -c 4 target/release/sudoku | od -An -c)" && echo "built: $file_info"
COPY appload/sudoku /out/sudoku
RUN cp target/release/sudoku /out/sudoku/sudoku && chmod 755 /out/sudoku/sudoku

FROM scratch AS export
COPY --from=build /out /
