pub mod config;
pub mod cors;
pub mod csp;
pub mod error;
pub mod health;
pub mod note;
pub mod status;
pub mod store;

use config::Config;
use store::Store;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub store: Store,
}
