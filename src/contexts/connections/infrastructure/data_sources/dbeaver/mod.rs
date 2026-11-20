pub mod credentials;
mod definitions;
mod discovery;
pub mod variables;
mod workspace;

pub use definitions::DbeaverWorkspaceDefinitions;
pub use discovery::{default_dbeaver_home, discover, discover_in};
