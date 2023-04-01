use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const PASS_STATE_PATH_STR: &str = "test_pass_state.state";

#[derive(Debug, Default)]
pub(crate) struct TestRunEntries(HashMap<String, RunEntry>);

impl TestRunEntries {
    fn to_serializable(self) -> Vec<SerializableRunEntry> {
        self.0
            .into_iter()
            .map(|(test_name, data)| SerializableRunEntry {
                test_name,
                info: RunEntry {
                    pass_state: data.pass_state,
                    last_run: data.last_run,
                },
            })
            .collect()
    }

    fn update_test_state(&mut self, t_key: &str, state: PassState) {
        let entry = self.0.get_mut(t_key).unwrap_or_else(|| {
            panic!(
                "Tried to update the pass state of the test \"{}\" but it did not exist!",
                t_key
            )
        });
        entry.pass_state = state;
        entry.last_run = chrono::Utc::now();
    }
}

impl From<Vec<SerializableRunEntry>> for TestRunEntries {
    fn from(v: Vec<SerializableRunEntry>) -> Self {
        TestRunEntries(HashMap::from_iter(
            v.into_iter().map(|e| (e.test_name, e.info)),
        ))
    }
}

#[derive(Debug, Deserialize, Serialize)]
enum PassState {
    Passed,
    Failed,
    NotRun,
}

#[derive(Debug, Deserialize, Serialize)]
struct SerializableRunEntry {
    test_name: String,
    info: RunEntry,
}

#[derive(Debug, Deserialize, Serialize)]
struct RunEntry {
    pass_state: PassState,
    last_run: DateTime<Utc>,
}

pub(crate) fn load_existing_pass_state_from_disk_if_exists_or_create() -> TestRunEntries {
    let mut reader = csv::Reader::from_path(PASS_STATE_PATH_STR).unwrap();
    reader
        .deserialize()
        .map(|r| r.unwrap())
        .collect::<Vec<_>>()
        .into()
}
