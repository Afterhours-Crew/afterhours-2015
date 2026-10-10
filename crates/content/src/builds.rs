// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Build profiles: identifiers that belong to one executable build.
//!
//! Every supported build has one JSON file in `crates/content/builds/` (see the
//! README there). The files are embedded at compile time and validated
//! strictly; [`profile_for`] selects one by executable digest.
use crate::{BuildIdentity, Error};
use serde_json::Value;
use std::collections::BTreeMap;

pub const FORMAT: &str = "nfs-build-profile";
pub const VERSION: u64 = 1;

/// `(file name, contents)` of every profile.
const PROFILES: &[(&str, &str)] = &[(
    "nfs16-92aa6ff4b5f8d0f0.json",
    include_str!("../builds/nfs16-92aa6ff4b5f8d0f0.json"),
)];

fn hex64(value: &Value, what: &str) -> Result<String, Error> {
    value
        .as_str()
        .filter(|s| s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        .map(str::to_owned)
        .ok_or_else(|| Error::Content(format!("build profile {what} is not a lower-case SHA-256")))
}

/// Parse and validate one profile document.
pub fn parse(file_name: &str, text: &str) -> Result<BuildIdentity, Error> {
    let bad = |what: &str| Error::Content(format!("build profile {file_name}: {what}"));
    let value: Value = serde_json::from_str(text).map_err(|_| bad("not JSON"))?;
    let object = value.as_object().ok_or_else(|| bad("not an object"))?;
    const FIELDS: [&str; 7] = [
        "format",
        "version",
        "description",
        "executable",
        "executable_sha256",
        "world_mac_template",
        "blueprint_class_ids",
    ];
    if object.len() != FIELDS.len() || FIELDS.iter().any(|f| !object.contains_key(*f)) {
        return Err(bad("unexpected or missing fields"));
    }
    if value["format"] != FORMAT || value["version"] != VERSION {
        return Err(bad("unsupported format or version"));
    }
    if value["description"].as_str().is_none_or(str::is_empty) {
        return Err(bad("description"));
    }
    let executable = value["executable"]
        .as_str()
        .filter(|e| *e == crate::EXECUTABLE)
        .ok_or_else(|| bad("executable name"))?;
    let executable_sha256 = hex64(&value["executable_sha256"], "executable_sha256")?;
    let expected = format!(
        "{}-{}.json",
        executable.trim_end_matches(".exe").to_lowercase(),
        &executable_sha256[..16]
    );
    if file_name != expected {
        return Err(bad("file name does not match the executable digest"));
    }
    let template = value["world_mac_template"]
        .as_object()
        .filter(|t| t.len() == 2)
        .ok_or_else(|| bad("world_mac_template"))?;
    let rva = template
        .get("rva")
        .and_then(Value::as_str)
        .and_then(|s| s.strip_prefix("0x"))
        .and_then(|s| u32::from_str_radix(s, 16).ok())
        .ok_or_else(|| bad("world_mac_template.rva"))?;
    let template_sha256 = hex64(
        template.get("sha256").unwrap_or(&Value::Null),
        "world_mac_template.sha256",
    )?;
    let ids = value["blueprint_class_ids"]
        .as_object()
        .ok_or_else(|| bad("blueprint_class_ids"))?;
    let mut class_ids = BTreeMap::new();
    for (class, id) in ids {
        let id = id
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .filter(|id| *id != 0)
            .ok_or_else(|| bad("blueprint class ID"))?;
        if !class.ends_with("Blueprint") || class_ids.values().any(|v| *v == id) {
            return Err(bad(
                "blueprint class IDs must name Blueprint classes and be unique",
            ));
        }
        class_ids.insert(class.clone(), id);
    }
    Ok(BuildIdentity {
        executable_sha256,
        mac_template_rva: rva,
        mac_template_sha256: template_sha256,
        blueprint_class_ids: class_ids,
    })
}

/// Every embedded profile.
pub fn profiles() -> Result<Vec<BuildIdentity>, Error> {
    PROFILES
        .iter()
        .map(|(name, text)| parse(name, text))
        .collect()
}

/// The profile for an executable digest, if that build is known.
pub fn profile_for(executable_sha256: &str) -> Result<Option<BuildIdentity>, Error> {
    Ok(profiles()?
        .into_iter()
        .find(|p| p.executable_sha256 == executable_sha256))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_profile_is_valid_and_unique() {
        let profiles = profiles().unwrap();
        assert_eq!(profiles.len(), PROFILES.len());
        let mut digests: Vec<_> = profiles.iter().map(|p| &p.executable_sha256).collect();
        digests.sort();
        digests.dedup();
        assert_eq!(digests.len(), profiles.len());
    }

    #[test]
    fn supported_build_has_a_profile() {
        let profile = profile_for(nfs_services::SUPPORTED_BUILD_SHA256)
            .unwrap()
            .unwrap();
        assert_eq!(profile, BuildIdentity::supported());
        assert_eq!(profile.blueprint_class_ids["RaceVehicleBlueprint"], 3410);
        assert!(profile_for(&"0".repeat(64)).unwrap().is_none());
    }

    #[test]
    fn malformed_profiles_are_rejected() {
        let (name, text) = PROFILES[0];
        let base: Value = serde_json::from_str(text).unwrap();
        let mut cases = Vec::new();
        let mut extra = base.clone();
        extra["unexpected"] = Value::Bool(true);
        cases.push((name, extra));
        let mut version = base.clone();
        version["version"] = 2.into();
        cases.push((name, version));
        let mut digest = base.clone();
        digest["executable_sha256"] = "ABC".into();
        cases.push((name, digest));
        let mut rva = base.clone();
        rva["world_mac_template"]["rva"] = "23e49c0".into();
        cases.push((name, rva));
        let mut class = base.clone();
        class["blueprint_class_ids"]["NotABlueprintData"] = 1.into();
        cases.push((name, class));
        let mut duplicate = base.clone();
        duplicate["blueprint_class_ids"]["ObjectBlueprint"] = 3392.into();
        cases.push((name, duplicate));
        cases.push(("nfs16-0000000000000000.json", base.clone()));
        for (file, value) in cases {
            assert!(parse(file, &value.to_string()).is_err(), "{file}: {value}");
        }
        assert!(parse(name, &base.to_string()).is_ok());
    }
}
