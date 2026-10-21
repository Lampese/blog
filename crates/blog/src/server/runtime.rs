//! Preview state serializes rebuilds and captures on a blocking worker.
use crate::fingerprint::source_tree;
use crate::{
    build::{self, BuildOptions, BuildResult},
    content::Include,
};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    ops::Range,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;

#[derive(Clone, Serialize)]
#[serde(untagged)]
pub(super) enum PreviewEvent {
    Reload { reload: bool },
    BuildFailed { error: String },
}

#[derive(Clone)]
pub(super) struct Preview {
    runtime: Arc<Mutex<Runtime>>,
    pub events: broadcast::Sender<PreviewEvent>,
    pub port: u16,
    pub output: PathBuf,
}

struct Runtime {
    root: PathBuf,
    result: BuildResult,
    signature: String,
}

impl Preview {
    pub async fn new(root: PathBuf, port: u16) -> Result<Self> {
        let runtime = tokio::task::spawn_blocking(move || -> Result<Runtime> {
            let result = build::build(&root, BuildOptions::preview())?;
            let signature = source_tree(&root, &result.config)?;
            Ok(Runtime {
                root,
                result,
                signature,
            })
        })
        .await
        .context("Preview initialization task failed")??;
        let output = runtime.root.join(&runtime.result.config.output);
        let (events, _) = broadcast::channel(32);
        Ok(Self {
            runtime: Arc::new(Mutex::new(runtime)),
            events,
            port,
            output,
        })
    }

    pub async fn capture(
        &self,
        article: String,
        revision: String,
        range: Range<usize>,
    ) -> Result<Include> {
        self.run(move |runtime| {
            let include = build::register(
                &runtime.root,
                &mut runtime.result,
                &article,
                &revision,
                range.start,
                range.end,
            )?;
            runtime.signature = source_tree(&runtime.root, &runtime.result.config)?;
            Ok(include)
        })
        .await
    }

    pub async fn refresh(&self) -> Result<Option<PreviewEvent>> {
        self.run(Runtime::refresh).await
    }

    async fn run<T, F>(&self, operation: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Runtime) -> Result<T> + Send + 'static,
    {
        let runtime = Arc::clone(&self.runtime);
        tokio::task::spawn_blocking(move || {
            let mut runtime = runtime.lock().map_err(|_| {
                anyhow::anyhow!("Preview build state is unavailable; restart the preview server")
            })?;
            operation(&mut runtime)
        })
        .await
        .context("Preview build task failed")?
    }
}

impl Runtime {
    fn refresh(&mut self) -> Result<Option<PreviewEvent>> {
        let signature = source_tree(&self.root, &self.result.config)?;
        if signature == self.signature {
            return Ok(None);
        }
        self.signature = signature;
        let event = match build::build(&self.root, BuildOptions::preview()) {
            Ok(result) => {
                self.result = result;
                self.signature = source_tree(&self.root, &self.result.config)?;
                eprintln!("Updated {} articles", self.result.posts.len());
                PreviewEvent::Reload { reload: true }
            }
            Err(error) => {
                let error = format!("{error:#}");
                eprintln!("Build failed: {error}");
                PreviewEvent::BuildFailed { error }
            }
        };
        Ok(Some(event))
    }
}
