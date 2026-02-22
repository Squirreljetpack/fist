pub mod colors;
pub mod path;
pub mod serde;
pub mod size;
pub mod text;

#[macro_export]
macro_rules! arr {
    ( $( $x:expr ),* $(,)? ) => {
        {
            arrayvec::ArrayVec::from_iter([$($x),*])
        }
    };
}
