pub mod credentials;
mod discovery;
pub mod pgpass;
pub mod variables;
mod workspace;

pub use discovery::{default_dbeaver_home, discover};
