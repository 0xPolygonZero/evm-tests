//! Logic to read in all tests from the parsed test directory. Processes these
//! files into easy to use structs for running tests.
//!
//! Note that there are three "levels" in the test directory:
//! - Test group (eg. "GeneralStateTests") (Note: likely will only ever be one).
//! - Test sub-group (eg. "stCreate2").
//! - Sub-group test (eg. "CREATE2_Bounds.test")

// High code duplication. Difficult to reduce, but may want to tackle later.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{anyhow, Context};
use common::{
    config::{GENERATION_INPUTS_DEFAULT_OUTPUT_DIR, MAIN_TEST_DIR},
    types::{ParsedTestManifest, TestVariantRunInfo, VariantFilterType},
};
use log::{info, trace};
use tokio::{
    fs::{self, read_dir},
    task::JoinSet,
};
use tokio_stream::{wrappers::ReadDirStream, StreamExt};

#[derive(Debug)]
pub(crate) struct ParsedTestGroup {
    pub(crate) name: String,
    pub(crate) sub_groups: Vec<ParsedTestSubGroup>,
}

#[derive(Debug)]
pub(crate) struct ParsedTestSubGroup {
    pub(crate) name: String,
    pub(crate) tests: Vec<Test>,
}

#[derive(Debug)]
pub(crate) struct Test {
    pub(crate) name: String,
    pub(crate) info: TestVariantRunInfo,
}

pub(crate) fn get_default_parsed_tests_path() -> anyhow::Result<PathBuf> {
    std::env::current_dir()?
        .ancestors()
        .map(|ancestor| {
            let mut buf = ancestor.to_path_buf();
            buf.push(GENERATION_INPUTS_DEFAULT_OUTPUT_DIR);
            buf.push(MAIN_TEST_DIR);
            buf
        })
        .find(|path| path.exists())
        .ok_or_else(|| {
            anyhow!(
                "Unable to find {} in cwd ancestry. Have you run the parser binary?",
                GENERATION_INPUTS_DEFAULT_OUTPUT_DIR
            )
        })
}

/// Reads in all parsed tests from the given parsed test directory.
pub(crate) async fn read_in_all_parsed_tests(
    parsed_tests_path: &Path,
    filter_str: Option<String>,
    variant_filter: Option<VariantFilterType>,
    blacklist: Option<Arc<HashSet<String>>>,
) -> anyhow::Result<Vec<ParsedTestGroup>> {
    let (mut groups, mut join_set, mut read_dirs) =
        parse_dir_init(Path::new(parsed_tests_path)).await?;

    while let Some(entry) = read_dirs.next().await {
        let entry = entry?;

        if !entry.file_type().await?.is_dir() {
            continue;
        }

        join_set.spawn(parse_test_group(
            entry.path(),
            filter_str.clone(),
            variant_filter.clone(),
            blacklist.clone(),
        ));
    }

    wait_for_task_to_finish_and_push_to_vec(&mut join_set, &mut groups).await?;

    Ok(groups)
}

async fn parse_test_group(
    path: PathBuf,
    filter_str: Option<String>,
    variant_filter: Option<VariantFilterType>,
    blacklist: Option<Arc<HashSet<String>>>,
) -> anyhow::Result<ParsedTestGroup> {
    info!("Reading in test group {:?}...", path);
    let (mut sub_groups, mut join_set, mut read_dirs) = parse_dir_init(&path).await?;

    while let Some(entry) = read_dirs.next().await {
        let entry = entry?;

        if !entry.file_type().await?.is_dir() {
            continue;
        }

        join_set.spawn(parse_test_sub_group(
            entry.path(),
            filter_str.clone(),
            variant_filter.clone(),
            blacklist.clone(),
        ));
    }

    wait_for_task_to_finish_and_push_to_vec(&mut join_set, &mut sub_groups).await?;

    Ok(ParsedTestGroup {
        name: get_file_stem(&path)?,
        sub_groups,
    })
}

async fn parse_test_sub_group(
    path: PathBuf,
    filter_str: Option<String>,
    variant_filter: Option<VariantFilterType>,
    blacklist: Option<Arc<HashSet<String>>>,
) -> anyhow::Result<ParsedTestSubGroup> {
    trace!("Reading in test subgroup {:?}...", path);
    let (mut tests, mut join_set, mut read_dirs) = parse_dir_init(&path).await?;

    while let Some(entry) = read_dirs.next().await {
        let entry = entry?;
        let file_path = entry.path();

        if test_is_not_in_filter_str(&filter_str, &file_path) {
            continue;
        }

        join_set.spawn(parse_test(
            file_path,
            variant_filter.clone(),
            blacklist.clone(),
        ));
    }

    wait_for_task_to_finish_and_extend_vec(&mut join_set, &mut tests).await?;

    Ok(ParsedTestSubGroup {
        name: get_file_stem(&path)?,
        tests,
    })
}

fn blacklisted(blacklist: Option<&HashSet<String>>, t_name: &str) -> bool {
    blacklist.is_some_and(|b_list| b_list.contains(t_name))
}

fn test_is_not_in_filter_str(filter_str: &Option<String>, file_path: &Path) -> bool {
    filter_str.as_ref().is_some_and(|f_str| {
        file_path
            .to_str()
            .is_some_and(|p_str| !p_str.contains(f_str))
    })
}

async fn parse_test(
    path: PathBuf,
    variant_filter: Option<VariantFilterType>,
    blacklist: Option<Arc<HashSet<String>>>,
) -> anyhow::Result<Vec<Test>> {
    trace!("Reading in {:?}...", path);

    let parsed_test_bytes = fs::read(&path).await?;
    let parsed_test: ParsedTestManifest = serde_cbor::from_slice(&parsed_test_bytes)
        .unwrap_or_else(|_| panic!("Unable to parse the test {:?} (bad format)", path));

    let v_out = parsed_test.into_filtered_variants(variant_filter);

    let blacklist_ref = blacklist.as_deref();
    Ok(v_out
        .variants
        .into_iter()
        .filter_map(|info| {
            let name = info.variant_name.clone();
            (!blacklisted(blacklist_ref, &name)).then_some(Test { name, info })
        })
        .collect())
}

async fn wait_for_task_to_finish_and_push_to_vec<T: 'static>(
    join_set: &mut JoinSet<anyhow::Result<T>>,
    out_vec: &mut Vec<T>,
) -> anyhow::Result<()> {
    wait_for_task_to_finish_and_apply_elem_to_vec(join_set, out_vec, |v, elem| v.push(elem)).await
}

async fn wait_for_task_to_finish_and_extend_vec<T: 'static>(
    join_set: &mut JoinSet<anyhow::Result<Vec<T>>>,
    out_vec: &mut Vec<T>,
) -> anyhow::Result<()> {
    wait_for_task_to_finish_and_apply_elem_to_vec(
        join_set,
        out_vec,
        |v: &mut Vec<T>, elems: Vec<T>| v.extend(elems.into_iter()),
    )
    .await
}

async fn wait_for_task_to_finish_and_apply_elem_to_vec<
    T: 'static,
    U: 'static,
    F: Fn(&mut Vec<T>, U),
>(
    join_set: &mut JoinSet<anyhow::Result<U>>,
    out_vec: &mut Vec<T>,
    apply_f: F,
) -> anyhow::Result<()> {
    while let Some(h) = join_set.join_next().await {
        apply_f(
            out_vec,
            h.with_context(|| "Getting the result from a join vec")??,
        );
    }

    Ok(())
}

/// Helper function to reduce code duplication.
///
/// Initializes variables that are common for each level of directory parsing.
async fn parse_dir_init<T, U>(path: &Path) -> anyhow::Result<(Vec<T>, JoinSet<U>, ReadDirStream)> {
    let output = Vec::new();
    let join_set = JoinSet::new();
    let read_dirs = ReadDirStream::new(
        read_dir(path)
            .await
            .with_context(|| format!("Creating a directory stream for path {:?}", path))?,
    );

    Ok((output, join_set, read_dirs))
}

fn get_file_stem(path: &Path) -> anyhow::Result<String> {
    let res = path
        .file_stem()
        .with_context(|| "Unable to get file stem")?
        .to_string_lossy()
        .to_string();
    Ok(res)
}
