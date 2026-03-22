pub mod daemon;
pub mod edit;
pub mod history;
pub mod info;
pub mod lsp;
pub mod read;
pub mod redo;
pub mod undo;

pub use daemon::execute as daemon;
pub use edit::execute as edit;
pub use history::execute as history;
pub use info::execute as info;
pub use lsp::execute as lsp;
pub use read::execute as read;
pub use redo::execute as redo;
pub use undo::execute as undo;
