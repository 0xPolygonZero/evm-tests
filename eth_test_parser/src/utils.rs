use std::process::Command;

use anyhow::Context;

pub(crate) fn run_cmd(cmd: &mut Command) -> anyhow::Result<()> {
    cmd.output().map(|_| ()).with_context(|| {
        format!(
            "Executing the cmd {:?} {:?}",
            cmd.get_program(),
            cmd.get_args()
        )
    })
}
