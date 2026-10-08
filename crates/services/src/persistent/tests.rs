// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::*;
use nfs_storage::{GarageSlots, MemoryRepository};
use serde_json::json;

// Synthetic columns except the required garage projection; no captured values.
pub(crate) fn content() -> Json {
    let tables: Vec<_> = REQUIRED.iter().map(|&(table, secondary, count)| {
        let columns: Vec<_> = (0..count).map(|i| {
            let name = if table == GARAGE { GARAGE_COLUMNS[i].to_string() } else { format!("Column{i}") };
            json!({"key":key(&name), "name":name, "value_type":if table == GARAGE {"string"} else {"int"},"default":"0"})
        }).collect();
        json!({"name":table,"key":key(table),"secondary_key":secondary,"using_stores":secondary.is_none(),"write_whole_rows":false,"columns":columns})
    }).collect();
    json!({"version":1,"build_sha256":BUILD,"source":{"path":"synthetic-test","sha256":"00".repeat(32)},"tables":tables})
}
pub(crate) fn catalog() -> Catalog {
    Catalog::from_json(&content()).unwrap()
}
fn empty(repo: &dyn InventoryRepository, who: AccountId) -> Snapshot {
    repo.open(who).unwrap();
    repo.apply(
        who,
        &Batch {
            id: 1,
            expected_generation: 0,
            ops: vec![Op::SetGarage(GarageSlots::default())],
        },
        Timestamp(1),
    )
    .unwrap();
    repo.snapshot(who).unwrap()
}
#[test]
fn strict_asset_schema_hashes_types_and_complete_required_set() {
    assert_eq!(key("ProgressionObjective"), 2251697951);
    assert_ne!(key("Active"), key("active"));
    let input = content();
    let parsed = Catalog::from_json(&input).unwrap();
    assert_eq!(parsed.schemas.len(), 16);
    assert_eq!(
        parsed
            .schemas
            .values()
            .map(|s| s.defaults.len())
            .sum::<usize>(),
        55
    );
    for change in 0..9 {
        let mut bad = input.clone();
        match change {
            0 => {
                bad["tables"].as_array_mut().unwrap().pop();
            }
            1 => bad["tables"][1] = bad["tables"][0].clone(),
            2 => bad["tables"][0]["key"] = json!(0),
            3 => bad["tables"][0]["columns"][0]["key"] = json!(0),
            4 => bad["tables"][0]["columns"][0]["value_type"] = json!("bool"),
            5 => bad["tables"][0]["secondary_key"] = json!("Wrong"),
            6 => bad["tables"][0]["write_whole_rows"] = json!(true),
            7 => bad["build_sha256"] = json!("unverified"),
            _ => bad["tables"][0]["captured_rows"] = json!([]),
        }
        assert!(Catalog::from_json(&bad).is_err(), "case{change}");
    }
}
#[test]
fn tables_initialize_once_preserve_sparse_progress_and_isolate_accounts() {
    let repo = MemoryRepository::default();
    let who = AccountId::from_owned_config([2; 16]).unwrap();
    let catalog = catalog();
    let initial = empty(&repo, who);
    assert_eq!(catalog.view(&initial), Err(Error::Config));
    let (snapshot, loaded) = catalog
        .ensure_loaded(&repo, who, initial, Timestamp(2))
        .unwrap();
    assert_eq!(
        (
            snapshot.tables.len(),
            loaded.tables.len(),
            loaded.stores.len()
        ),
        (15, 16, 33)
    );
    assert!(!snapshot.tables.contains_key(&key(GARAGE)));
    assert_eq!(
        loaded.tables[&key(GARAGE)].rows[&0][&key("PrimaryVehicleItem")],
        Value::String("0".into())
    );
    assert_eq!(snapshot.generation, 2);
    assert_eq!(
        catalog
            .ensure_loaded(&repo, who, snapshot.clone(), Timestamp(9))
            .unwrap(),
        (snapshot.clone(), loaded)
    );
    let store_key = key("RepValuesTable");
    let changed = Table {
        rows: BTreeMap::from([(0, Row::from([(key("Column0"), Value::Int(731))]))]),
    };
    repo.apply(
        who,
        &Batch {
            id: 7,
            expected_generation: 2,
            ops: vec![
                Op::SetTable(store_key, changed.clone()),
                Op::SetTable(key("TutorialTable"), Table::default()),
            ],
        },
        Timestamp(3),
    )
    .unwrap();
    let (preserved, view) = catalog
        .ensure_loaded(&repo, who, repo.snapshot(who).unwrap(), Timestamp(4))
        .unwrap();
    assert_eq!(preserved.tables[&store_key], changed);
    assert_eq!(view.stores[&(store_key, key("Column0"))], 731);
    assert_eq!(view.stores[&(store_key, key("Column1"))], 0);
    assert!(view.tables[&key("TutorialTable")].rows.is_empty());
    let other = AccountId::from_owned_config([3; 16]).unwrap();
    let (_, view) = catalog
        .ensure_loaded(&repo, other, empty(&repo, other), Timestamp(4))
        .unwrap();
    assert_eq!(view.stores[&(store_key, key("Column0"))], 0);
}
#[test]
fn invalid_rows_missing_tables_and_duplicate_garage_prevent_completion() {
    let catalog = catalog();
    let repo = MemoryRepository::default();
    let who = AccountId::from_owned_config([4; 16]).unwrap();
    let (valid, _) = catalog
        .ensure_loaded(&repo, who, empty(&repo, who), Timestamp(2))
        .unwrap();
    for change in 0..5 {
        let mut bad = valid.clone();
        let table = bad.tables.get_mut(&key("RepValuesTable")).unwrap();
        match change {
            0 => {
                table
                    .rows
                    .get_mut(&0)
                    .unwrap()
                    .insert(key("Column0"), Value::String("7".into()));
            }
            1 => {
                table.rows.insert(1, Row::new());
            }
            2 => {
                bad.tables.remove(&key("TutorialTable"));
            }
            3 => {
                bad.tables.insert(key(GARAGE), Table::default());
            }
            _ => bad.garage = None,
        }
        assert_eq!(catalog.view(&bad), Err(Error::Config));
    }
}
#[test]
fn dynamic_rows_use_typed_defaults_without_overwriting_sparse_saved_values() {
    let mut input = content();
    input["tables"][1]["columns"][0]["value_type"] = json!("float");
    input["tables"][1]["columns"][0]["default"] = json!("1.5");
    input["tables"][1]["columns"][1]["value_type"] = json!("string");
    input["tables"][1]["columns"][1]["default"] = json!("local");
    let catalog = Catalog::from_json(&input).unwrap();
    let repo = MemoryRepository::default();
    let who = AccountId::from_owned_config([5; 16]).unwrap();
    let (mut snapshot, _) = catalog
        .ensure_loaded(&repo, who, empty(&repo, who), Timestamp(2))
        .unwrap();
    let table = snapshot
        .tables
        .get_mut(&key("PersistentDataTable"))
        .unwrap();
    table
        .rows
        .insert(u64::MAX, Row::from([(key("Column2"), Value::Int(-15))]));
    let view = catalog.view(&snapshot).unwrap();
    let row = &view.tables[&key("PersistentDataTable")].rows[&u64::MAX];
    assert_eq!(row[&key("Column0")], Value::Float(1.5f32.to_bits()));
    assert_eq!(row[&key("Column1")], Value::String("local".into()));
    assert_eq!(row[&key("Column2")], Value::Int(-15));
    assert!(!view.has_cell(key("PersistentDataTable"), u64::MAX, key("Column0")));
    assert!(!view.has_cell(key("PersistentDataTable"), u64::MAX, key("Column1")));
    assert!(view.has_cell(key("PersistentDataTable"), u64::MAX, key("Column2")));
    assert_eq!(
        snapshot.tables[&key("PersistentDataTable")].rows[&u64::MAX].len(),
        1
    );
    input["tables"][1]["columns"][1]["default"] = json!("x".repeat(MAX_STRING_BYTES));
    let large_defaults = Catalog::from_json(&input).unwrap();
    snapshot
        .tables
        .get_mut(&key("PersistentDataTable"))
        .unwrap()
        .rows = (0..100).map(|id| (id, Row::new())).collect();
    assert_eq!(large_defaults.view(&snapshot), Err(Error::Bounds));
}
