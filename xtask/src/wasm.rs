//! Builds the example playground for the browser (decision D1).
//!
//! Output lands in `app/static/wasm/`, which the site generator copies. The
//! two steps are kept separate on purpose: `cargo site` must work without a
//! wasm toolchain, and the reader degrades to a clear message when the module
//! is absent.

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

const TARGET: &str = "wasm32-unknown-unknown";

pub fn run(repo: &Path) -> Result<()> {
    let expected = pinned_version(repo)?;
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

    println!("building playground for {TARGET} (wasm-bindgen {expected})");
    let status = Command::new(env!("CARGO"))
        .current_dir(repo)
        .args([
            "build",
            "--package",
            "playground",
            "--release",
            "--target",
            TARGET,
        ])
        .status()
        .context("running cargo build")?;
    if !status.success() {
        bail!("cargo build failed for the playground");
    }

    let wasm = repo
        .join("target")
        .join(TARGET)
        .join("release")
        .join("playground.wasm");
    if !wasm.is_file() {
        bail!("expected {} to exist after the build", wasm.display());
    }

    let out = repo.join("app").join("static").join("wasm");
    std::fs::create_dir_all(&out)?;

    let status = Command::new("wasm-bindgen")
        .args(["--target", "web", "--no-typescript", "--out-dir"])
        .arg(&out)
        .arg(&wasm)
        .status()
        .context("running wasm-bindgen (is wasm-bindgen-cli on PATH?)")?;
    if !status.success() {
        bail!("wasm-bindgen failed");
    }

    for name in ["playground_bg.wasm", "playground.js"] {
        let path = out.join(name);
        let size = std::fs::metadata(&path)
            .with_context(|| format!("{} was not produced", path.display()))?
            .len();
        println!("  {name:<22} {:>7.1} KB", size as f64 / 1024.0);
    }
    println!("wrote {}", out.display());
    Ok(())
}

/// Reads the exact pin from `playground/Cargo.toml` so there is one source of
/// truth for the version.
fn pinned_version(repo: &Path) -> Result<String> {
    let path = repo.join("playground").join("Cargo.toml");
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
                "playground/Cargo.toml must pin wasm-bindgen exactly, e.g. \
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
