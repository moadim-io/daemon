//! Prompt composition, slug/shell helpers, and the single-line tmux launch command builder.

use std::collections::BTreeMap;

use crate::paths::{routine_compiled_prompt_path, routine_scheduled_log_path};
use crate::routine_storage::read_local_env;

use super::agents::AgentCommand;
use super::model::Routine;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "command_part2.rs"]
mod command_part2;
pub(crate) use command_part2::*;

#[path = "command_prompt.rs"]
mod command_prompt;
pub(crate) use command_prompt::*;

#[path = "command_repositories.rs"]
mod command_repositories;
pub(crate) use command_repositories::*;

#[path = "command_path_resolution.rs"]
mod command_path_resolution;
pub(crate) use command_path_resolution::*;

#[path = "command_system_prompt.rs"]
mod command_system_prompt;
pub(crate) use command_system_prompt::system_prompt_stmts;

include!("command_builder.rs");
#[cfg(test)]
#[path = "command_tests.rs"]
mod command_tests;

#[cfg(test)]
#[path = "command_bin_resolution_tests.rs"]
mod command_bin_resolution_tests;

#[cfg(test)]
#[path = "command_placeholder_tests.rs"]
mod command_placeholder_tests;
