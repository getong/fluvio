mod list;
pub use list::SmartModuleHubListOpts;
mod download;
pub use download::{SmartModuleDownloadHubOpts, download_local, download_cluster};

use std::sync::Arc;
use std::fmt::Debug;

use clap::Parser;
use anyhow::Result;

use fluvio_extension_common::Terminal;
use fluvio::Fluvio;

use crate::client::ClientCmd;

/// List available SmartModules in the hub
#[derive(Debug, Parser)]
pub enum SmartModuleHubSubCmd {
    /// List available SmartModules
    #[command(name = "list")]
    List(SmartModuleHubListOpts),

    /// Download SmartModules - locally or to cluster (default)
    #[command(name = "download")]
    Download(SmartModuleDownloadHubOpts),
}

impl SmartModuleHubSubCmd {
    pub async fn process<O: Terminal + Debug + Send + Sync>(self, out: Arc<O>) -> Result<()> {
        match self {
            SmartModuleHubSubCmd::List(opts) => opts.process(out).await,
            SmartModuleHubSubCmd::Download(opts) => {
                opts.process_client(out, &Fluvio::connect().await?).await
            }
        }
    }
}
