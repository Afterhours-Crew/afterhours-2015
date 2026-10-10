// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Installation fingerprint: the executable, both layouts, both catalogues and
//! every superbundle TOC. Bundle manifests and archives are not hashed; a
//! change to them changes a TOC or catalogue.
use crate::{BUILDER_REVISION, EXECUTABLE, Error, FORMAT, Options, VERSION, sha256_hex};
use sha2::Digest;
use std::io::Read;
use std::path::Path;

const MAX_EXECUTABLE_BYTES: u64 = 512 << 20;

/// One hashed input file, relative to the installation root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Input {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

/// Digest over the builder revision and every input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fingerprint {
    pub digest: String,
    pub inputs: Vec<Input>,
}

fn hash_file(root: &Path, relative: &str, limit: u64) -> Result<Option<Input>, Error> {
    let path = root.join(relative);
    let mut file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if file.metadata()?.len() > limit {
        return Err(Error::Content(format!("{relative} exceeds its size bound")));
    }
    let mut hasher = sha2::Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut bytes = 0u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        bytes += n as u64;
    }
    Ok(Some(Input {
        path: relative.into(),
        sha256: nfs_frostbite::hex(&hasher.finalize()),
        bytes,
    }))
}

/// Fingerprint an installation after checking its executable.
pub fn fingerprint(root: &Path, options: &Options) -> Result<Fingerprint, Error> {
    let executable = hash_file(root, EXECUTABLE, MAX_EXECUTABLE_BYTES)?
        .ok_or_else(|| Error::Content(format!("{EXECUTABLE} is missing")))?;
    if executable.sha256 != options.identity.executable_sha256 {
        return Err(Error::UnsupportedExecutable {
            sha256: executable.sha256,
        });
    }
    let container_limit = options.limits.max_container_bytes as u64;
    let mut inputs = vec![executable];
    let mut required = vec!["Data/layout.toc".to_owned(), "Data/cas.cat".to_owned()];
    let optional = ["Update/Patch/Data/layout.toc", "Update/Patch/Data/cas.cat"];
    let install = nfs_frostbite::install::Installation::open(root, options.limits)?;
    for superbundle in install.superbundles() {
        if superbundle.contains("..") {
            return Err(Error::Content("superbundle path".into()));
        }
        required.push(format!("Data/{superbundle}.toc"));
    }
    for relative in &required {
        // A superbundle may legitimately have no base TOC; the reader skips it.
        if let Some(input) = hash_file(root, relative, container_limit)? {
            inputs.push(input);
        } else if !relative.ends_with(".toc") || relative == "Data/layout.toc" {
            return Err(Error::Content(format!("{relative} is missing")));
        }
    }
    let patched: Vec<String> = optional
        .iter()
        .map(|s| s.to_string())
        .chain(
            install
                .superbundles()
                .iter()
                .map(|sb| format!("Update/Patch/Data/{sb}.toc")),
        )
        .collect();
    for relative in &patched {
        if let Some(input) = hash_file(root, relative, container_limit)? {
            inputs.push(input);
        }
    }
    let mut text = format!("{FORMAT}\nversion {VERSION}\nbuilder {BUILDER_REVISION}\n");
    for input in &inputs {
        text.push_str(&format!(
            "{}\t{}\t{}\n",
            input.path, input.sha256, input.bytes
        ));
    }
    Ok(Fingerprint {
        digest: sha256_hex(text.as_bytes()),
        inputs,
    })
}
