# check=skip=FromPlatformFlagConstDisallowed
# Builds the app for the reMarkable Paper Pro (aarch64 Linux, musl). This build does not run the
# tests: Dockerfile.test runs them.
#
#   docker build --platform linux/arm64 --output type=local,dest=dist .
#
# The result is dist/sudoku/: the AppLoad folder (manifest.json, resources.rcc, backend/entry,
# external.manifest.json, icon.png and the static executable `sudoku`; REQUIREMENTS.md §14.4). Copy it
# to /home/root/xovi/exthome/appload/ on the tablet.
# To set the version in the Settings panel (default "dev"), add --build-arg SUDOKU_VERSION=<version>.

# The files of the AppLoad folder with the QML frontend (REQUIREMENTS.md §14.4): the Qt tool `rcc`
# makes resources.rcc from the QML (appload/make-folder.sh). Alpine 3.22 gives a fixed version of Qt.
# This stage keeps the Qt packages out of the Rust stage.
FROM --platform=linux/arm64 alpine:3.22 AS frontend
RUN apk add --no-cache qt6-qtbase-dev
COPY appload /appload
RUN sh /appload/make-folder.sh /out/sudoku

FROM --platform=linux/arm64 rust:1.97-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
# Download and compile the dependencies first, so that a change of the source does not do this again.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin && for b in sudoku sudoku-colors snapshot video bench; do echo 'fn main() {}' > src/bin/$b.rs; done \
 && echo '' > src/lib.rs && cargo build --release --locked --bin sudoku && rm -rf src
COPY src src
# The files that the executable contains (include_bytes! and include_str!).
COPY assets assets
COPY images images
COPY tests/fixtures tests/fixtures
COPY video/demo-full.txt video/demo-full.txt
RUN touch src/lib.rs src/bin/*.rs
# The version in the Settings panel. release.yml gives pre-<sha> or v<version>.
ARG SUDOKU_VERSION=dev
RUN SUDOKU_VERSION="$SUDOKU_VERSION" cargo build --release --locked --bin sudoku \
 && file_info="$(head -c 4 target/release/sudoku | od -An -c)" && echo "built: $file_info"
COPY --from=frontend /out/sudoku /out/sudoku
RUN cp target/release/sudoku /out/sudoku/sudoku && chmod 755 /out/sudoku/sudoku /out/sudoku/backend/entry

FROM scratch AS export
COPY --from=build /out /
