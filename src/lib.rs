mod cursor;
mod node_span;
mod plugin;
mod style;
mod view;
mod widgets;

pub use cursor::Cursor;
pub use node_span::NodeSpan;
pub use plugin::QuillPlugin;
pub use style::*;

pub use view::*;

// Define the prelude module
pub mod prelude {
    pub use crate::plugin::QuillPlugin;
    pub use crate::style::*;
    pub use crate::view::*;
}
