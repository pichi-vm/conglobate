// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    context::CapturedContext,
    document::YamlDocument,
    path::{ContextDirectory, ContextPath},
    plan::{Plan, Reference},
    request::CommitIdentity,
};
use std::path::PathBuf;

/// Discover inputs with the same committed context and planner as a build.
/// This operation needs neither a lock nor cache, mounts, or execution inputs.
#[derive(Debug, clap::Args)]
pub(crate) struct Inspect {
    #[arg(long)]
    repository: PathBuf,
    #[arg(long)]
    commit: String,
    #[arg(long, default_value = ".")]
    context: PathBuf,
    #[arg(long)]
    plan: PathBuf,
}

impl Inspect {
    pub(crate) fn inputs(&self) -> anyhow::Result<crate::contract::build::Inputs> {
        let commit = CommitIdentity::try_from(self.commit.as_str())?;
        let context = ContextDirectory::try_from(self.context.as_path())?;
        let path = ContextPath::try_from(self.plan.as_path())?;
        let mut context = CapturedContext::capture(&self.repository, &commit, &context)?;
        let file = context.read(&path)?;
        let plan = Plan::try_from(YamlDocument::from(file.text()?))?;
        Ok(crate::contract::build::Inputs {
            images: plan
                .order()
                .iter()
                .filter_map(|name| match plan.tasks()[name].from() {
                    Reference::Remote(reference) => Some(reference.as_ref().to_owned()),
                    Reference::Local(_) => None,
                })
                .collect(),
        })
    }

    pub(crate) fn run(&self) -> anyhow::Result<()> {
        let inputs = self.inputs()?;
        serde_json::to_writer(std::io::stdout().lock(), &inputs)?;
        Ok(())
    }
}
