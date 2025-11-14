use live_queue::main_entry;
use tracing_subscriber;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    main_entry().await;
}
