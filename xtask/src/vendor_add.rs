//! Adding a source to the pin.
//!
//! Vendoring a crate by hand is three commands and one of them is easy to skip:
//! N1 vendored four sources that way, and the one that matters — proving the
//! archive is the published one before its bytes become the thing every line
//! range is keyed to — leaves no trace when it is skipped. `bump` has verified
//! every byte it wrote since D7. This is the same path for a tree that is not
//! replacing one.
//!
//! It cannot move a version. Re-pinning an existing source without remapping
//! its line ranges is what `cargo xtask bump` exists to prevent, so a source
//! the pin already names is refused here and sent there.

use anyhow::{bail, Context, Result};
use slbl_core::vendor::{self, Role, Source};
use std::path::Path;

pub(crate) struct Opts {
    pub name: String,
    pub version: String,
    pub role: Role,
}

/// `cargo xtask vendor <name>@<version> --role <role>`
pub(crate) fn parse_args(args: &[String]) -> Result<Opts> {
    let mut spec: Option<String> = None;
    let mut role: Option<Role> = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--role" => {
                let raw = it.next().context("--role takes a name")?;
                role = Some(match raw.as_str() {
                    "coverage" => Role::Coverage,
                    "narrative" => Role::Narrative,
                    "glossary" => Role::Glossary,
                    other => bail!(
                        "unknown role {other:?} — the gates know coverage, narrative and \
                         glossary, and a role no gate knows is a build failure"
                    ),
                });
            }
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => spec = Some(other.to_string()),
        }
    }
    let spec = spec.context("usage: cargo xtask vendor <name>@<version> --role <role>")?;
    let (name, version) = spec
        .split_once('@')
        .context("name and version are one argument: serde_json@1.0.151")?;
    Ok(Opts {
        name: name.to_string(),
        version: version.to_string(),
        role: role.context("--role is required: what the gates ask of this source")?,
    })
}

pub(crate) fn run(repo: &Path, opts: &Opts) -> Result<()> {
    let mut pin = vendor::load_pin(repo)?;
    if let Some(existing) = pin.sources.iter().find(|s| s.name == opts.name) {
        bail!(
            "vendor/pin.toml already names {} {}. Moving a version remaps every line \
             range keyed to it: cargo xtask bump --source {} {}",
            existing.name,
            existing.version,
            opts.name,
            opts.version
        );
    }
    let root = vendor::crate_dir(repo, &opts.name, &opts.version);
    if root.exists() {
        bail!("{} already exists", root.display());
    }

    // Verified before a byte of it is vendored, for the reason bump gives: an
    // unverified download is a supply-chain hole dressed up as tooling.
    let staging = repo.join("target").join("vendor-add");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;
    let expected = crate::bump::fetch_index_checksum(&opts.name, &opts.version)?;
    let archive = staging.join(format!("{}-{}.crate", opts.name, opts.version));
    crate::bump::download(&opts.name, &opts.version, &archive)?;
    let actual = crate::bump::sha256_file(&archive)?;
    if actual != expected {
        bail!(
            "checksum mismatch for {}\n  expected {expected}\n  actual   {actual}",
            archive.display()
        );
    }
    println!("verified {} ({actual})", archive.display());

    let unpacked = staging.join("unpacked");
    std::fs::create_dir_all(&unpacked)?;
    crate::bump::extract(&archive, &unpacked)?;
    let from = unpacked.join(vendor::source_id(&opts.name, &opts.version));
    if !from.join("src").is_dir() {
        bail!("{} has no src/ directory", from.display());
    }
    std::fs::rename(&from, &root)
        .with_context(|| format!("moving the tree into {}", root.display()))?;

    // Appended after the last source of its own role, so the pin stays grouped
    // the way its header describes the roles. Within a role the order is the
    // order sources were added, and for `coverage` that order is read back as
    // reading order.
    let source = Source {
        name: opts.name.clone(),
        version: opts.version.clone(),
        role: opts.role,
        crate_sha256: actual,
        src_tree_sha256: vendor::tree_hash_of(&root)?,
    };
    let rank = |r: Role| match r {
        Role::Coverage => 0,
        Role::Narrative => 1,
        Role::Glossary => 2,
    };
    let at = pin
        .sources
        .iter()
        .rposition(|s| rank(s.role) <= rank(opts.role))
        .map_or(0, |i| i + 1);
    pin.sources.insert(at, source);
    std::fs::write(vendor::pin_path(repo), vendor::render_pin(&pin))?;
    let _ = std::fs::remove_dir_all(&staging);

    println!(
        "vendored {} as a {} source",
        vendor::source_id(&opts.name, &opts.version),
        format!("{:?}", opts.role).to_lowercase()
    );
    println!("run `cargo xtask pin` to regenerate NOTICE.md, then `cargo xtask coverage`");
    Ok(())
}
