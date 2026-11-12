#!/usr/bin/env just --justfile

main_crate := 'twkb'
# How to call the current just executable. Note that just_executable() may have `\` in Windows paths, so we need to quote it.
just := quote(just_executable())
# cargo-binstall needs a workaround due to caching when used in CI
binstall_args := if env('CI', '') != '' {'--no-confirm --no-track --disable-telemetry'} else {''}
# The PostGIS of `just test-postgis`, and of CI
postgis_image := 'postgis/postgis:18-3.6-alpine'
# location of the coverage output, used by CI
coverage_lcov := 'target/llvm-cov/lcov.info'
# location of the jscpd copy/paste detection reports, used by CI
cpd_output := 'target/jscpd'

# if running in CI, treat warnings as errors by setting CARGO_BUILD_WARNINGS to 'deny' unless it is already set
# Use `CI=true just ci-test` to run the same tests as in GitHub CI.
# Use `just env-info` to see the current value of CARGO_BUILD_WARNINGS
ci_mode := if env('CI', '') != '' {'1'} else {''}
export CARGO_BUILD_WARNINGS := env('CARGO_BUILD_WARNINGS', if ci_mode == '1' {'deny'} else {'warn'})
export RUST_BACKTRACE := env('RUST_BACKTRACE', if ci_mode == '1' {'1'} else {'0'})

@_default:
    {{just}} --list

# Time writing and reading TWKB against WKB with criterion
bench *args:
    cargo bench --bench twkb {{args}}

# Build the project
build:
    cargo build --workspace --all-features --all-targets

# Quick compile without building a binary
check:
    cargo check --workspace --all-features --all-targets

# Generate LCOV coverage report for CI to upload to codecov.io
ci-coverage: env-info && \
        (_coverage '--lcov' '--output-path' quote(coverage_lcov))
    rm -rf {{quote(parent_directory(coverage_lcov))}}
    mkdir -p {{quote(parent_directory(coverage_lcov))}}

# Find copy/pasted code, marking clones absent from base_ref as new, and write a Markdown summary for the PR comment
ci-cpd base_ref='origin/main':  (assert-cmd 'jq') (cpd '--reporters' 'console,json' '--output' cpd_output '--baseline-from-ref' base_ref)
    jq -r --arg base {{quote(base_ref)}} -f .github/jscpd-summary.jq {{quote(cpd_output / 'jscpd-report.json')}} > {{quote(cpd_output / 'summary.md')}}
    cat {{quote(cpd_output / 'summary.md')}} >> "${GITHUB_STEP_SUMMARY:-/dev/stdout}"

# Run all tests as expected by CI
ci-test: env-info test-fmt clippy test test-doc test-examples && assert-git-is-clean

# Compile default features with minimal dependencies on the configured MSRV
ci-test-msrv:
    {{just}} ci_mode=0 env-info _check-msrv-default
    {{just}} assert-git-is-clean

# Set toolchain and run ci-test-msrv
ci-test-msrv-with-toolchain:
    RUSTUP_TOOLCHAIN="$({{just}} get-msrv)" {{just}} ci-test-msrv

# Clean all build artifacts
clean:
    cargo clean
    rm -f Cargo.lock

# Run cargo clippy to lint the code, and the fuzz targets, a workspace of their own
clippy *args:
    cargo clippy --workspace --all-features --all-targets {{args}}
    cargo clippy --manifest-path fuzz/Cargo.toml --all-targets {{args}}

# Generate and open the HTML coverage report
coverage:  (_coverage '--open')

# Clean, collect, and aggregate coverage using the requested report arguments
_coverage *report_args:  (cargo-install 'cargo-llvm-cov')
    cargo llvm-cov clean --workspace
    cargo llvm-cov --no-report --workspace --all-features --all-targets
    cargo llvm-cov report --include-build-script {{report_args}}

# Find copy/pasted code with jscpd, configured in .jscpd.json. See `just cpd --help` for all options
cpd *args:  (cargo-install 'jscpd' 'jscpd --bin jscpd' '--bin-dir={bin}{binary-ext}')
    jscpd {{args}}

# Build and open code documentation
docs *args='--open':
    DOCS_RS=1 cargo doc --no-deps {{args}} --workspace --all-features

# Print environment info
env-info:
    @echo "Running for '{{main_crate}}' crate {{if ci_mode == '1' {'in CI mode'} else {'in dev mode'} }} on {{os()}} / {{arch()}}"
    @echo "PWD {{justfile_directory()}}"
    {{just}} --version
    rustc --version
    cargo --version
    rustup --version
    @echo "CARGO_BUILD_WARNINGS='$CARGO_BUILD_WARNINGS'"
    @echo "RUST_BACKTRACE='$RUST_BACKTRACE'"

# Fuzz a target with cargo-fuzz and nightly for some seconds (`just fuzz write 600`); the read corpus starts from compat's cases
fuzz target='read' seconds='60':  (cargo-install 'cargo-fuzz')
    #!/usr/bin/env bash
    set -euo pipefail
    cd fuzz
    if [[ {{quote(target)}} == read && ! -d corpus/read ]]; then
        # Each case's TWKB, one file each
        TWKB_CORPUS_DIR="$PWD/corpus/read" cargo test --manifest-path ../Cargo.toml -p twkb-compat --test compat write_fuzz_corpus \
            -- --ignored --quiet
    fi
    cargo +nightly fuzz run --target "$(rustc -vV | sed -n 's/^host: //p')" {{quote(target)}} -- \
        -max_total_time={{seconds}} -timeout=5 -rss_limit_mb=2048

# Reformat all code `cargo fmt`. If nightly is available, use it for better results
fmt:
    #!/usr/bin/env bash
    set -euo pipefail
    if (rustup toolchain list | grep nightly && rustup component list --toolchain nightly | grep rustfmt) &> /dev/null; then
        echo 'Reformatting Rust code using nightly Rust fmt to sort imports'
        cargo +nightly fmt --all -- --config imports_granularity=Module,group_imports=StdExternalCrate
        cargo +nightly fmt --all --manifest-path fuzz/Cargo.toml -- --config imports_granularity=Module,group_imports=StdExternalCrate
    else
        echo 'Reformatting Rust with the stable cargo fmt.  Install nightly with `rustup install nightly` for better results'
        cargo fmt --all
        cargo fmt --all --manifest-path fuzz/Cargo.toml
    fi

# Reformat all Cargo.toml files using cargo-sort
fmt-toml *args:  (cargo-install 'cargo-sort')
    cargo sort --workspace --grouped {{args}}

# Get a package field from the metadata
get-crate-field field package=main_crate:  (assert-cmd 'jq')
    @cargo metadata --no-deps --format-version 1 | jq -e -r '.packages | map(select(.name == "{{package}}")) | first | .{{field}} // error("Field \"{{field}}\" is missing in Cargo.toml for package {{package}}")'

# Get the minimum supported Rust version (MSRV) for the crate
get-msrv package=main_crate:  (get-crate-field 'rust_version' package)

# Find the minimum supported Rust version (MSRV), update Cargo.toml, and test minimal dependencies
msrv:  (cargo-install 'cargo-msrv')
    cargo msrv find --write-msrv --ignore-lockfile -- {{just}} _check-msrv-default

# Compile the crate's default features using a dynamically generated minimal Cargo.lock
_check-msrv-default:  (cargo-install 'cargo-minimal-versions') (cargo-install 'cargo-hack')
    #!/usr/bin/env bash
    set -euo pipefail
    # cargo-msrv probes with rustup, but nested cargo subcommands may otherwise
    # fall back to the default Cargo and emit flags unsupported by the candidate rustc.
    toolchain="$(rustc --version | cut -d' ' -f2)"
    export RUSTUP_TOOLCHAIN="$toolchain"
    export CARGO="$(rustup which --toolchain "$toolchain" cargo)"
    cargo minimal-versions check --direct --package {{main_crate}}

# Run cargo-release
release *args='':  (cargo-install 'release-plz')
    release-plz {{args}}

# Check semver compatibility with prior published version. Install it with `cargo install cargo-semver-checks`
semver *args:  (cargo-install 'cargo-semver-checks')
    cargo semver-checks --all-features {{args}}

# Run all tests
test:
    cargo test --workspace --all-features --all-targets
    cargo test --doc --workspace --all-features

# Check every case against PostGIS in one batch, in a Docker container on `port` that is started if it is not running
test-postgis port='5436':  (assert-cmd 'docker')
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -z "$(docker ps -q --filter name='^twkb-postgis$')" ]]; then
        docker rm -f twkb-postgis > /dev/null 2>&1 || true
        docker run -d --rm --name twkb-postgis -e POSTGRES_PASSWORD=postgres -p {{port}}:5432 {{postgis_image}} > /dev/null
    fi
    for _ in $(seq 60); do
        docker exec twkb-postgis pg_isready -U postgres -h 127.0.0.1 -q && break
        sleep 1
    done
    {{just}} test-postgis-at "postgresql://postgres:postgres@localhost:{{port}}/postgres"

# Check every case against the PostGIS at a connection string, in one batch
test-postgis-at url:
    TWKB_POSTGIS_URL={{quote(url)}} cargo test -p twkb-compat --test compat against_postgis::against_postgis -- --ignored --exact --nocapture

# Stop the PostGIS container of `just test-postgis`
stop-postgis:
    docker rm -f twkb-postgis

# Test documentation generation
test-doc:  (docs '')

# Run the command-line examples
test-examples:
    cargo run --quiet --example encode -- 'LINESTRING(-71.0366718 42.3734493,-71.0367619 42.373312)' --precision 7
    cargo run --quiet --example encode -- 0101000000000000000000f03f0000000000000040
    cargo run --quiet --example encode -- 'POINT Z (1.25 2.5 3)' --precision 1,0 --size --bbox
    cargo run --quiet --example decode -- e20002fbd7baa505bab38d9403890eb915
    cargo run --quiet --example decode -- 04070b0004020402000200020404

# Test code formatting
test-fmt: && (fmt-toml '--check' '--check-format')
    cargo fmt --all -- --check
    cargo fmt --all --manifest-path fuzz/Cargo.toml -- --check

# Find unused dependencies. Uses `cargo-udeps`
udeps:  (cargo-install 'cargo-udeps')
    cargo +nightly udeps --workspace --all-features --all-targets

# Update all dependencies, including breaking changes. Requires nightly toolchain (install with `rustup install nightly`)
update:  (cargo-install 'cargo-upgrade' 'cargo-edit')
    cargo upgrade --incompatible --compatible ignore
    cargo update

# Ensure that a certain command is available
[private]
assert-cmd command:
    @if ! type {{command}} > /dev/null; then \
        echo "Command '{{command}}' could not be found. Please make sure it has been installed on your computer." ;\
        exit 1 ;\
    fi

# Make sure the git repo has no uncommitted changes. Fails if CI envvar is set.
[private]
assert-git-is-clean:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -n "$(git status --porcelain --untracked-files=all)" ]; then
        >&2 echo "::error::git repo is not clean. Make sure compilation and tests artifacts are in the .gitignore, and no repo files are modified."
        if [[ "{{ci_mode}}" == "1" ]]; then
            >&2 echo "::group::git status"
            git status
            >&2 echo "::endgroup::"
            >&2 echo "::group::git diff (tracked changes)"
            git add . --intent-to-add
            git --no-pager diff
            >&2 echo "::endgroup::"
            exit 1
        else
            >&2 echo "git repo is not clean, but not failing because CI mode is not enabled."
        fi
    fi

# Check if a certain Cargo command is installed, and install it if needed
[private]
cargo-install $COMMAND $INSTALL_CMD='' $BINSTALL_EXTRA_ARGS='' *args='':
    #!/usr/bin/env bash
    set -euo pipefail
    unset CARGO_BUILD_WARNINGS
    if ! command -v $COMMAND > /dev/null; then
        echo "$COMMAND could not be found. Installing..."
        if ! command -v cargo-binstall > /dev/null; then
            set -x
            cargo install ${INSTALL_CMD:-$COMMAND} --locked {{args}}
            { set +x; } 2>/dev/null
        else
            set -x
            cargo binstall ${INSTALL_CMD:-$COMMAND} {{binstall_args}} $BINSTALL_EXTRA_ARGS --locked
            { set +x; } 2>/dev/null
        fi
    fi
