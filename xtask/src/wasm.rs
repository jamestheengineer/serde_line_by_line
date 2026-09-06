//! Builds the browser modules (decision D1).
//!
//! Two of them. `playground` holds the micro-examples and is loaded wherever
//! one appears; `expander` holds `serde_derive` itself and is several times
//! the size, so it is fetched only on pages that show an expansion. Merging
//! them would make every example page pay for an expander it never calls.
//!
//! Output lands in `app/static/wasm/`, which the site generator copies. The
//! two steps are kept separate on purpose: `cargo site` must work without a
//! wasm toolchain, and the reader degrades to a clear message when the module
//! is absent.

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

const TARGET: &str = "wasm32-unknown-unknown";

/// The wasm packages, in build order.
const MODULES: &[&str] = &["playground", "expander"];

pub fn run(repo: &Path) -> Result<()> {
    let expected = pinned_version(repo, "playground")?;
    let actual = cli_version()?;

    // D1's accepted cost. A mismatch produces confusing failures deep in the
    // generated glue, so it is checked up front with an actionable message.
    if expected != actual {
        bail!(
            "wasm-bindgen version mismatch\n  \
             playground/Cargo.toml pins  {expected}\n  \
             wasm-bindgen CLI is         {actual}\n\n\
             Install the matching CLI:\n  \
             cargo install wasm-bindgen-cli --version {expected}"
        );
    }
    // Both modules load into the same page context, so a version skew between
    // them is a glue mismatch at runtime rather than a build failure.
    for module in MODULES {
        let pinned = pinned_version(repo, module)?;
        if pinned != expected {
            bail!(
                "{module}/Cargo.toml pins wasm-bindgen {pinned}, \
                 but playground/Cargo.toml pins {expected}"
            );
        }
    }

    let out = repo.join("app").join("static").join("wasm");
    std::fs::create_dir_all(&out)?;
    for module in MODULES {
        build_module(repo, module, &out, &expected)?;
    }
    println!("wrote {}", out.display());

    verify(repo)?;
    Ok(())
}

fn build_module(repo: &Path, package: &str, out: &Path, bindgen: &str) -> Result<()> {
    println!("building {package} for {TARGET} (wasm-bindgen {bindgen})");
    let status = Command::new(env!("CARGO"))
        .current_dir(repo)
        .args([
            "build",
            "--package",
            package,
            "--release",
            "--target",
            TARGET,
        ])
        .status()
        .context("running cargo build")?;
    if !status.success() {
        bail!("cargo build failed for {package}");
    }

    let wasm = repo
        .join("target")
        .join(TARGET)
        .join("release")
        .join(format!("{package}.wasm"));
    if !wasm.is_file() {
        bail!("expected {} to exist after the build", wasm.display());
    }

    let status = Command::new("wasm-bindgen")
        .args(["--target", "web", "--no-typescript", "--out-dir"])
        .arg(out)
        .arg(&wasm)
        .status()
        .context("running wasm-bindgen (is wasm-bindgen-cli on PATH?)")?;
    if !status.success() {
        bail!("wasm-bindgen failed for {package}");
    }

    for name in [format!("{package}_bg.wasm"), format!("{package}.js")] {
        let path = out.join(&name);
        let size = std::fs::metadata(&path)
            .with_context(|| format!("{} was not produced", path.display()))?
            .len();
        println!("  {name:<22} {:>7.1} KB", size as f64 / 1024.0);
    }
    Ok(())
}

/// Runs every example out of the module just built and diffs it against the
/// transcript `cargo test` asserts.
///
/// CI has always compiled the examples for the browser and run them natively —
/// two facts that do not add up to the one that matters, which is that the
/// output a reader sees on the site is the output the explanation claims. A
/// `#[cfg(target_arch)]` branch, or anything that formats differently on
/// wasm32, would have slipped straight through.
///
/// Skipped, loudly, without node. Skipping leaves the wasm build exactly as
/// verified as it was before this existed.
fn verify(repo: &Path) -> Result<()> {
    if Command::new("node").arg("--version").output().is_err() {
        println!("  ! node not found — skipping the wasm output check");
        return Ok(());
    }

    run_smoke(
        repo,
        "wasm-smoke.mjs",
        include_str!("wasm_smoke.mjs"),
        "the playground's output does not match the committed transcripts",
    )?;
    run_smoke(
        repo,
        "expand-smoke.mjs",
        include_str!("expand_smoke.mjs"),
        "the browser's expansions do not match expand/expected.txt",
    )?;
    Ok(())
}

fn run_smoke(repo: &Path, name: &str, script: &str, failure: &str) -> Result<()> {
    let harness = repo.join("target").join(name);
    let source = script.replace("__REPO__", &repo.display().to_string());
    std::fs::write(&harness, source).with_context(|| format!("writing {}", harness.display()))?;

    let out = Command::new("node")
        .arg(&harness)
        .output()
        .context("running node")?;
    print!("{}", String::from_utf8_lossy(&out.stdout));
    if !out.status.success() {
        eprint!("{}", String::from_utf8_lossy(&out.stderr));
        bail!("{failure}");
    }
    Ok(())
}

/// Reads the exact pin from a module's `Cargo.toml` so there is one source of
/// truth for the version.
fn pinned_version(repo: &Path, package: &str) -> Result<String> {
    let path = repo.join(package).join("Cargo.toml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("wasm-bindgen") {
            if let Some(start) = rest.find("\"=") {
                let tail = &rest[start + 2..];
                if let Some(end) = tail.find('"') {
                    return Ok(tail[..end].to_string());
                }
            }
            bail!(
                "{package}/Cargo.toml must pin wasm-bindgen exactly, e.g. \
                 wasm-bindgen = \"=0.2.127\"; found: {line}"
            );
        }
    }
    bail!("no wasm-bindgen dependency found in {}", path.display())
}

fn cli_version() -> Result<String> {
    let out = Command::new("wasm-bindgen")
        .arg("--version")
        .output()
        .context(
            "could not run `wasm-bindgen` — install it with \
             `cargo install wasm-bindgen-cli --version <pinned>`",
        )?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace()
        .nth(1)
        .map(str::to_string)
        .with_context(|| format!("could not parse wasm-bindgen version from {text:?}"))
}
