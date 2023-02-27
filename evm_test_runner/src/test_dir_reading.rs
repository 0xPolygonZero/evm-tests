//! Logic to read in all tests from the parsed test directory. Processes these
//! files into easy to use structs for running tests.
//!
//! Note that there are three "levels" in the test directory:
//! - Test group (eg. "GeneralStateTests") (Note: likely will only ever be one).
//! - Test sub-group (eg. "stCreate2").
//! - Sub-group test (eg. "CREATE2_Bounds.test")

// High code duplication. Difficult to reduce, but may want to tackle later.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use common::{
    config::GENERATION_INPUTS_DEFAULT_OUTPUT_DIR,
    types::{ParsedTest, TestVariantRunInfo},
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
) -> anyhow::Result<Vec<ParsedTestGroup>> {
    let (mut groups, mut join_set, mut read_dirs) =
        parse_dir_init(Path::new(parsed_tests_path)).await?;

    while let Some(entry) = read_dirs.next().await {
        let entry = entry?;

        if !entry.file_type().await?.is_dir() {
            continue;
        }

        join_set.spawn(parse_test_group(entry.path(), filter_str.clone()));
    }

    wait_for_task_to_finish_and_push_to_vec(&mut join_set, &mut groups).await?;

    Ok(groups)
}

async fn parse_test_group(
    path: PathBuf,
    filter_str: Option<String>,
) -> anyhow::Result<ParsedTestGroup> {
    info!("Reading in test group {:?}...", path);
    let (mut sub_groups, mut join_set, mut read_dirs) = parse_dir_init(&path).await?;

    while let Some(entry) = read_dirs.next().await {
        let entry = entry?;

        if !entry.file_type().await?.is_dir() {
            continue;
        }

        join_set.spawn(parse_test_sub_group(entry.path(), filter_str.clone()));
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
) -> anyhow::Result<ParsedTestSubGroup> {
    trace!("Reading in test subgroup {:?}...", path);
    let (mut tests, mut join_set, mut read_dirs) = parse_dir_init(&path).await?;

    while let Some(entry) = read_dirs.next().await {
        let entry = entry?;
        let file_path = entry.path();

        // Reject test if the filter string does not match.
        if let Some(ref filter_str) = filter_str && !file_path.to_str().map_or(false, |path_str| path_str.contains(filter_str)) {
            continue;
        }

        join_set.spawn(parse_test(file_path));
    }

    wait_for_task_to_finish_and_extend_vec(&mut join_set, &mut tests).await?;

    Ok(ParsedTestSubGroup {
        name: get_file_stem(&path)?,
        tests,
    })
}

async fn parse_test(path: PathBuf) -> anyhow::Result<Vec<Test>> {
    trace!("Reading in {:?}...", path);

    let parsed_test_bytes = fs::read(&path).await?;
    let parsed_test: ParsedTest = serde_cbor::from_slice(&parsed_test_bytes)
        .unwrap_or_else(|_| panic!("Unable to parse the test {:?} (bad format)", path));

    let test_variants = parsed_test.get_test_variants();

    let root_test_name = get_file_stem(&path)?;
    let t_name_f: Box<dyn Fn(usize) -> String> = match test_variants.len() {
        1 => Box::new(|_| root_test_name.clone()),
        _ => Box::new(|i| format!("{}_{}", root_test_name, i)),
    };

    Ok(test_variants
        .into_iter()
        .enumerate()
        .map(|(i, info)| Test {
            name: t_name_f(i),
            info,
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
