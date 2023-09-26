use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use log::info;
use serde::{Deserialize, Serialize};

use crate::plonky2_runner::TestStatus;

const PASS_STATE_PATH_STR: &str = "test_pass_state.csv";

#[derive(Debug, Default)]
pub(crate) struct TestRunEntries(HashMap<String, RunEntry>);

impl TestRunEntries {
    pub(crate) fn write_to_disk(self) {
        println!("Persisting test pass state to disk...");

        let data = self.into_serializable();
        let mut writer = csv::Writer::from_path(PASS_STATE_PATH_STR).unwrap();

        for entry in data {
            writer.serialize(entry).unwrap();
        }
    }

    fn into_serializable(self) -> Vec<SerializableRunEntry> {
        let mut data: Vec<_> = self
            .0
            .into_iter()
            .map(|(test_name, data)| SerializableRunEntry {
                test_name,
                pass_state: data.pass_state,
                last_run: data.last_run,
            })
            .collect();

        data.sort_unstable_by(|e1, e2| e1.test_name.cmp(&e2.test_name));
        data
    }

    pub(crate) fn update_test_state(&mut self, t_key: &str, state: PassState) {
        self.0
            .entry(t_key.to_string())
            .and_modify(|entry| *entry = RunEntry::new(state))
            .or_insert_with(|| RunEntry::new(state));
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

    pub(crate) fn get_tests_that_have_passed(&self) -> impl Iterator<Item = &str> {
        self.0.iter().filter_map(|(name, info)| {
            matches!(info.pass_state, PassState::Passed | PassState::Ignored).then(|| name.as_str())
        })
    }
}

impl From<Vec<SerializableRunEntry>> for TestRunEntries {
    fn from(v: Vec<SerializableRunEntry>) -> Self {
        TestRunEntries(HashMap::from_iter(v.into_iter().map(|e| {
            (
                e.test_name,
                RunEntry {
                    pass_state: e.pass_state,
                    last_run: e.last_run,
                },
            )
        })))
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Default, Serialize)]
pub(crate) enum PassState {
    Passed,
    Ignored,
    Failed,
    #[default]
    NotRun,
}

impl From<TestStatus> for PassState {
    fn from(v: TestStatus) -> Self {
        match v {
            TestStatus::Passed => PassState::Passed,
            TestStatus::Ignored => PassState::Ignored,
            TestStatus::EvmErr(_) | TestStatus::TimedOut => PassState::Failed,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct SerializableRunEntry {
    test_name: String,
    pass_state: PassState,
    last_run: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Default, Serialize)]
struct RunEntry {
    pass_state: PassState,
    last_run: Option<DateTime<Utc>>,
}

impl RunEntry {
    fn new(pass_state: PassState) -> Self {
        Self {
            pass_state,
            last_run: Some(chrono::Utc::now()),
        }
    }
}

pub(crate) fn load_existing_pass_state_from_disk_if_exists_or_create() -> TestRunEntries {
    csv::Reader::from_path(PASS_STATE_PATH_STR)
        .map(|mut reader| {
            info!("Found existing test run state on disk.");

            reader
                .deserialize()
                .map(|r| r.unwrap())
                .collect::<Vec<_>>()
                .into()
        })
        .unwrap_or_else(|_| {
            info!("No existing test run state found.");
            TestRunEntries::default()
        })
}
