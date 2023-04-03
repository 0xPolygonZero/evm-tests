use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::plonky2_runner::TestStatus;

const PASS_STATE_PATH_STR: &str = "test_pass_state.state";

#[derive(Debug, Default)]
pub(crate) struct TestRunEntries(HashMap<String, RunEntry>);

impl TestRunEntries {
    pub(crate) fn write_to_disk(self) {
        let data = self.to_serializable();
        let mut writer = csv::Writer::from_path(PASS_STATE_PATH_STR).unwrap();

        for entry in data {
            writer.serialize(entry).unwrap();
        }
    }

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

    pub(crate) fn update_test_state(&mut self, t_key: &str, state: PassState) {
        let entry = self.0.get_mut(t_key).unwrap_or_else(|| {
            panic!(
                "Tried to update the pass state of the test \"{}\" but it did not exist!",
                t_key
            )
        });

        entry.pass_state = state;
        entry.last_run = Some(chrono::Utc::now());
    }

    pub(crate) fn add_remove_entries_from_upstream_tests<'a>(
        &'a mut self,
        upstream_tests: impl Iterator<Item = &'a str>,
    ) {
        let t_names_that_are_in_upstream: HashSet<_> =
            upstream_tests.map(|s| s.to_string()).collect();

        // Add any new tests that we don't know about.
        for upstream_k in t_names_that_are_in_upstream.iter() {
            if !self.0.contains_key(upstream_k) {
                self.0.insert(upstream_k.clone(), Default::default());
            }
        }

        // Remove any entries that are not longer in upstream.
        for local_k in self.0.keys().cloned().collect::<Vec<_>>() {
            if !t_names_that_are_in_upstream.contains(&local_k) {
                self.0.remove(local_k.as_str());
            }
        }
    }
}

impl From<Vec<SerializableRunEntry>> for TestRunEntries {
    fn from(v: Vec<SerializableRunEntry>) -> Self {
        TestRunEntries(HashMap::from_iter(
            v.into_iter().map(|e| (e.test_name, e.info)),
        ))
    }
}

#[derive(Debug, Deserialize, Default, Serialize)]
pub(crate) enum PassState {
    Passed,
    Failed,
    #[default]
    NotRun,
}

impl From<TestStatus> for PassState {
    fn from(v: TestStatus) -> Self {
        match v {
            TestStatus::Passed => PassState::Passed,
            TestStatus::EvmErr(_) | TestStatus::IncorrectAccountFinalState(_) => PassState::Failed,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct SerializableRunEntry {
    test_name: String,
    info: RunEntry,
}

#[derive(Debug, Deserialize, Default, Serialize)]
struct RunEntry {
    pass_state: PassState,
    last_run: Option<DateTime<Utc>>,
}

pub(crate) fn load_existing_pass_state_from_disk_if_exists_or_create() -> TestRunEntries {
    let mut reader = csv::Reader::from_path(PASS_STATE_PATH_STR).unwrap();
    reader
        .deserialize()
        .map(|r| r.unwrap())
        .collect::<Vec<_>>()
        .into()
}
