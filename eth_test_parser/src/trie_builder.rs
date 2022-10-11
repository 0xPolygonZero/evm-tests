//! Module responsible for converting deserialized json tests into
//! plonky2 generation inputs.
//!
//! In other words
//! ```ignore
//! crate::deserialize::TestBody -> plonky2_evm::generation::GenerationInputs
//! ```
use std::{
    collections::HashMap,
    fs::{DirEntry, File},
    io::BufReader,
};

use anyhow::{anyhow, Ok, Result};

use crate::{deserialize::TestBody, fs_scaffolding::get_test_files};

/// Generate an iterator containing the deserialized test bodies (`TestBody`)
/// and their `DirEntry`s.
pub(crate) fn get_deserialized_test_bodies() -> Result<impl Iterator<Item = (DirEntry, TestBody)>> {
    Ok(get_test_files()?.flat_map(|entry| {
        let buf = BufReader::new(File::open(entry.path())?);
        let file_json: HashMap<String, TestBody> = serde_json::from_reader(buf)?;

        // Each test JSON always contains a single outer key containing the test name.
        // The test name is irrelevant for deserialization purposes, so we always drop
        // it.
        let test_body = file_json
            .into_values()
            .next()
            .ok_or_else(|| anyhow!("Empty test found: {:?}", entry))?;

        Ok((entry, test_body))
    }))
}
