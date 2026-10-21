//! Poll the source tree; filesystem and compiler work stays off the async executor.
use super::runtime::Preview;
use std::time::Duration;

pub(super) async fn run(preview: Preview) {
    let mut interval = tokio::time::interval(Duration::from_millis(650));
    loop {
        interval.tick().await;
        match preview.refresh().await {
            Ok(Some(event)) => {
                let _ = preview.events.send(event);
            }
            Ok(None) => {}
            Err(error) => eprintln!("Watch error: {error:#}"),
        }
    }
}
