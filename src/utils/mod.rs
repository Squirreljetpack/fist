pub mod categories;
pub mod colors;
pub mod filetypes;
pub mod icons;
pub mod path;
pub mod serde;
pub mod size;
pub mod text;
mod types;
pub use types::{FileCategory, FileType};

#[macro_export]
macro_rules! arr {
    ( $( $x:expr ),* $(,)? ) => {
        {
            arrayvec::ArrayVec::from_iter([$($x),*])
        }
    };
}
