// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! End-to-end tests over constructed installations; no game data is used.
use super::*;
use nfs_frostbite::synthetic::{Asset, Field, PartitionWriter, write_installation};
use nfs_services::item_content::ItemContent;
use nfs_services::persistent::{Catalog, required_tables};
use std::sync::atomic::{AtomicU32, Ordering};

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "nfs-content-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const TEMPLATE_RVA: u32 = 0x1010;

/// A minimal PE image with one `.data` section holding a 64-byte template.
fn executable(template: &[u8; 64]) -> Vec<u8> {
    let mut image = vec![0u8; 0x400];
    image[..2].copy_from_slice(b"MZ");
    image[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    image[0x40..0x44].copy_from_slice(b"PE\0\0");
    image[0x46..0x48].copy_from_slice(&1u16.to_le_bytes());
    let section = 0x40 + 24;
    image[section..section + 5].copy_from_slice(b".data");
    for (i, word) in [0x1000u32, 0x1000, 0x200, 0x200].iter().enumerate() {
        image[section + 8 + i * 4..section + 12 + i * 4].copy_from_slice(&word.to_le_bytes());
    }
    image[0x210..0x250].copy_from_slice(template);
    image
}

fn guid(seed: u8) -> [u8; 16] {
    [seed; 16]
}
fn reference(seed: u8) -> ([u8; 16], [u8; 16]) {
    (guid(seed), guid(seed ^ 0x80))
}

/// One item partition (`seed`) with the common item fields and extras.
fn item(
    seed: u8,
    class: &str,
    sub_items: Vec<u8>,
    additional: Vec<u8>,
    extra: Vec<(&'static str, Field)>,
) -> Asset {
    let mut fields = vec![
        ("Name", Field::CString(format!("Items/{class}{seed}"))),
        ("BuyPrice", Field::Int(350)),
        ("SellPrice", Field::Int(210)),
        ("Quantity", Field::Int(-1)),
        (
            "InventoryOwnershipLevel",
            Field::Enum(
                "InventoryOwnershipLevelType",
                "InventoryOwnershipLevelType_Purchasable",
                2,
            ),
        ),
        ("Deprecated", Field::Bool(false)),
        ("RequiresOwner", Field::Bool(true)),
        ("UsesScope", Field::Bool(false)),
        ("Purchasable", Field::Bool(true)),
        ("Optional", Field::Bool(seed.is_multiple_of(2))),
        (
            "SubItems",
            Field::Imports(sub_items.into_iter().map(reference).collect()),
        ),
        (
            "AdditionalItems",
            Field::Imports(additional.into_iter().map(reference).collect()),
        ),
    ];
    fields.extend(extra);
    let (partition, instance) = reference(seed);
    Asset {
        name: format!("items/{}{seed}", class.to_lowercase()),
        partition: PartitionWriter::default().build(partition, class, instance, fields),
    }
}

fn vector(x: f32, y: f32, z: f32) -> Field {
    Field::Struct(
        "Vec3",
        vec![
            ("x", Field::Float(x)),
            ("y", Field::Float(y)),
            ("z", Field::Float(z)),
        ],
    )
}

fn table(index: u8, name: &str, secondary: bool, count: usize) -> Asset {
    let columns = (0..count)
        .map(|i| {
            if name == "GarageItemsTable" {
                let names = [
                    "PrimaryVehicleItem",
                    "Vehicle1",
                    "Vehicle2",
                    "Vehicle3",
                    "Vehicle4",
                ];
                vec![
                    ("Name", Field::CString(names[i].into())),
                    (
                        "DataType",
                        Field::Enum("PersistentPropertyType", "PersistentPropertyType_String", 2),
                    ),
                    ("DefaultValue", Field::CString("0".into())),
                ]
            } else {
                vec![
                    ("Name", Field::CString(format!("{name}Column{i}"))),
                    (
                        "DataType",
                        Field::Enum("PersistentPropertyType", "PersistentPropertyType_Int", 0),
                    ),
                    ("DefaultValue", Field::CString(String::new())),
                ]
            }
        })
        .collect();
    Asset {
        name: format!("tables/{}", name.to_lowercase()),
        partition: PartitionWriter::default().build(
            [0xA0 ^ index; 16],
            "PersistentTableAsset",
            [0x50 ^ index; 16],
            vec![
                ("Name", Field::CString(format!("Tables/{name}"))),
                ("TableName", Field::CString(name.into())),
                (
                    "Properties",
                    Field::Structs("PersistentPropertyInfo", columns),
                ),
                ("HasSecondaryKey", Field::Bool(secondary)),
                ("WriteWholeRows", Field::Bool(false)),
            ],
        ),
    }
}

struct Install {
    dir: TempDir,
    identity: BuildIdentity,
    template: [u8; 64],
}

/// Vehicle item 3; stored field order differs from the wire order on purpose.
fn vehicle(plate: &str) -> Asset {
    item(
        3,
        "RaceVehicleItemData",
        vec![],
        vec![2],
        vec![
            ("DefaultPaint", vector(0.025, 0.11, 0.6)),
            ("Material", vector(0.76, 0.73, 1.0)),
            ("TopSpeedMPH", Field::Int(155)),
            ("WindowTint", Field::Float(0.9)),
            ("DefaultLicensePlateText", Field::CString(plate.into())),
            ("MaxHP", Field::Int(327)),
            ("MaxTorque", Field::Int(397)),
            ("ZeroToSixty", Field::Float(4.79)),
            ("ZeroToOneHundred", Field::Float(10.97)),
            ("QuarterMileMPH", Field::Int(110)),
            ("QuarterMileTime", Field::Float(13.26)),
            ("VehicleItemFlags", Field::Int(0)),
        ],
    )
}

fn assets() -> Vec<Asset> {
    let mut assets = vec![
        item(
            1,
            "SpoilerItemData",
            vec![],
            vec![],
            vec![("DownforceSetting", Field::Int(5))],
        ),
        item(2, "BodyKitItemData", vec![1], vec![], vec![]),
        vehicle("GHOST"),
        item(4, "TimedDiscountItemData", vec![], vec![], vec![]),
        item(
            5,
            "LiveryCustomizationItemData",
            vec![],
            vec![],
            vec![
                ("ByteVaultDataId", Field::Int(0)),
                ("LibraryPresetId", Field::Int(-1)),
                ("DynamicFlags", Field::Int(0)),
            ],
        ),
    ];
    assets.push(Asset {
        name: items::ITEM_SYSTEM.into(),
        partition: PartitionWriter::default().build(
            guid(0x10),
            "ItemSystemData",
            guid(0x11),
            vec![
                ("Name", Field::CString("Items/GameItemSystem".into())),
                ("Items", Field::Imports((1..=5).map(reference).collect())),
                (
                    "Inventory",
                    Field::Imports(vec![reference(3), reference(1)]),
                ),
            ],
        ),
    });
    for (i, (name, secondary, count)) in required_tables().iter().enumerate() {
        assets.push(table(i as u8, name, secondary.is_some(), *count));
    }
    assets
}

fn install_with(assets: &[Asset]) -> Install {
    let dir = TempDir::new();
    let template: [u8; 64] = std::array::from_fn(|i| (i * 3 + 1) as u8);
    let image = executable(&template);
    write_installation(&dir.0.join("game"), &image, assets).unwrap();
    let identity = BuildIdentity {
        executable_sha256: sha256_hex(&image),
        mac_template_rva: TEMPLATE_RVA,
        mac_template_sha256: sha256_hex(&template),
    };
    Install {
        dir,
        identity,
        template,
    }
}

impl Install {
    fn game(&self) -> PathBuf {
        self.dir.0.join("game")
    }
    fn cache(&self) -> PathBuf {
        self.dir.0.join("cache")
    }
    fn options(&self) -> Options {
        Options {
            identity: self.identity.clone(),
            limits: Limits::default(),
        }
    }
    fn prepare(&self) -> Result<Prepared, Error> {
        prepare(&self.game(), &self.cache(), &self.options())
    }
}

#[test]
fn builds_outputs_the_service_loaders_accept() {
    let install = install_with(&assets());
    let prepared = install.prepare().unwrap();
    assert!(prepared.built);
    assert_eq!(
        prepared.directory,
        install.cache().join(&prepared.fingerprint.digest)
    );
    let items = ItemContent::load(&prepared.path(Kind::ItemContent)).unwrap();
    let json = items.to_json();
    let definitions = json["definitions"].as_array().unwrap();
    assert_eq!(definitions.len(), 5);
    let by_class = |class: &str| {
        definitions
            .iter()
            .find(|d| d["class"] == class)
            .unwrap()
            .clone()
    };
    let spoiler = by_class("SpoilerItemData");
    assert_eq!(spoiler["guid"], nfs_frostbite::hex(&guid(1 ^ 0x80)));
    assert_eq!(spoiler["buy_price"], 350);
    assert_eq!(spoiler["quantity"], 4_294_967_295u64);
    assert_eq!(spoiler["ownership_level"], 2);
    assert_eq!(spoiler["flags"]["optional"], false);
    assert_eq!(spoiler["default_derived_hex"], "05000000");
    let body_kit = by_class("BodyKitItemData");
    assert_eq!(body_kit["default_derived_hex"], "");
    assert_eq!(body_kit["sub_items"][0], spoiler["guid"]);
    let vehicle = by_class("RaceVehicleItemData");
    let mut expected = Vec::new();
    for v in [0.025f32, 0.11, 0.6, 0.76, 0.73, 1.0, 0.9] {
        expected.extend(v.to_bits().to_le_bytes());
    }
    expected.extend(b"GHOST\0\0\0");
    for v in [
        327u32,
        397,
        4.79f32.to_bits(),
        10.97f32.to_bits(),
        155,
        110,
        13.26f32.to_bits(),
        0,
    ] {
        expected.extend(v.to_le_bytes());
    }
    assert_eq!(
        vehicle["default_derived_hex"],
        nfs_frostbite::hex(&expected)
    );
    assert_eq!(vehicle["additional_items"][0], body_kit["guid"]);
    assert!(by_class("TimedDiscountItemData")["default_derived_hex"].is_null());
    assert_eq!(
        by_class("LiveryCustomizationItemData")["default_derived_hex"],
        "00000000ffffffff00000000"
    );
    assert_eq!(
        json["initial_definitions"],
        serde_json::json!([vehicle["guid"], spoiler["guid"]])
    );
    let tables = Catalog::load(&prepared.path(Kind::PersistentContent)).unwrap();
    drop(tables);
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(prepared.path(Kind::PersistentContent)).unwrap())
            .unwrap();
    assert_eq!(
        document["tables"].as_array().unwrap().len(),
        required_tables().len()
    );
    assert_eq!(document["source"]["sha256"], prepared.fingerprint.digest);
    assert_eq!(
        std::fs::read(prepared.path(Kind::WorldMacTemplate)).unwrap(),
        install.template
    );
}

#[test]
fn second_prepare_reuses_and_a_damaged_entry_is_rebuilt() {
    let install = install_with(&assets());
    let first = install.prepare().unwrap();
    let second = install.prepare().unwrap();
    assert!(!second.built);
    assert_eq!(first.directory, second.directory);
    let template = first.path(Kind::WorldMacTemplate);
    std::fs::write(&template, b"tampered").unwrap();
    let third = install.prepare().unwrap();
    assert!(third.built);
    assert_eq!(std::fs::read(template).unwrap(), install.template);
}

#[test]
fn an_entry_with_unexpected_files_is_not_deleted() {
    let install = install_with(&assets());
    let prepared = install.prepare().unwrap();
    std::fs::write(prepared.path(Kind::ItemContent), b"{}").unwrap();
    std::fs::write(prepared.directory.join("notes.txt"), b"keep").unwrap();
    assert!(matches!(install.prepare(), Err(Error::Cache(_))));
    assert!(prepared.directory.join("notes.txt").exists());
}

#[test]
fn a_changed_installation_gets_a_new_entry() {
    let install = install_with(&assets());
    let first = install.prepare().unwrap();
    let mut catalog = std::fs::read(install.game().join("Data/cas.cat")).unwrap();
    catalog.extend([0u8; 32]);
    std::fs::write(install.game().join("Data/cas.cat"), catalog).unwrap();
    let second = install.prepare().unwrap();
    assert!(second.built);
    assert_ne!(first.fingerprint.digest, second.fingerprint.digest);
    assert!(first.directory.exists());
}

#[test]
fn an_unsupported_executable_is_rejected_before_reading_content() {
    let install = install_with(&assets());
    let mut options = install.options();
    options.identity.executable_sha256 = "0".repeat(64);
    assert!(matches!(
        prepare(&install.game(), &install.cache(), &options),
        Err(Error::UnsupportedExecutable { .. })
    ));
    assert!(!install.cache().exists());
}

#[test]
fn a_wrong_template_digest_or_address_fails() {
    let install = install_with(&assets());
    let mut options = install.options();
    options.identity.mac_template_sha256 = "0".repeat(64);
    assert!(matches!(
        prepare(&install.game(), &install.cache(), &options),
        Err(Error::Content(_))
    ));
    let mut options = install.options();
    options.identity.mac_template_rva = 0x1000 + 0x200 - 32;
    assert!(matches!(
        prepare(&install.game(), &install.cache(), &options),
        Err(Error::Content(_))
    ));
}

#[test]
fn missing_or_inconsistent_content_fails_without_publishing() {
    let mut without_system = assets();
    without_system.retain(|a| a.name != items::ITEM_SYSTEM);
    let install = install_with(&without_system);
    assert!(matches!(install.prepare(), Err(Error::Content(_))));
    let entries: Vec<_> = std::fs::read_dir(install.cache()).unwrap().collect();
    assert!(entries.is_empty());

    let mut without_table = assets();
    without_table.retain(|a| a.name != "tables/tickettable");
    assert!(matches!(
        install_with(&without_table).prepare(),
        Err(Error::Content(_))
    ));

    let mut wrong_secondary = assets();
    let (name, secondary, count) = required_tables()[0];
    wrong_secondary.push(table(0, name, secondary.is_none(), count));
    wrong_secondary.swap_remove(
        wrong_secondary
            .iter()
            .position(|a| a.name == format!("tables/{}", name.to_lowercase()))
            .unwrap(),
    );
    assert!(matches!(
        install_with(&wrong_secondary).prepare(),
        Err(Error::Content(_))
    ));

    let mut unknown_class = assets();
    unknown_class[0] = item(1, "HovercraftItemData", vec![], vec![], vec![]);
    assert!(matches!(
        install_with(&unknown_class).prepare(),
        Err(Error::Content(_))
    ));

    let mut long_plate = assets();
    long_plate[2] = vehicle("TOOLONGPLATE");
    match install_with(&long_plate).prepare() {
        Err(Error::Content(message)) => assert!(message.contains("eight bytes"), "{message}"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn dangling_item_reference_is_rejected() {
    let mut dangling = assets();
    dangling[1] = item(2, "BodyKitItemData", vec![99], vec![], vec![]);
    assert!(matches!(
        install_with(&dangling).prepare(),
        Err(Error::Content(_))
    ));
}

#[test]
fn fingerprint_lists_inputs_and_is_stable() {
    let install = install_with(&assets());
    let a = fingerprint(&install.game(), &install.options()).unwrap();
    let b = fingerprint(&install.game(), &install.options()).unwrap();
    assert_eq!(a, b);
    let paths: Vec<_> = a.inputs.iter().map(|i| i.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            EXECUTABLE,
            "Data/layout.toc",
            "Data/cas.cat",
            "Data/Win32/synthetic.toc"
        ]
    );
}
