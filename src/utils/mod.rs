pub mod colors;
pub mod formatter;
pub mod path;
pub mod serde;
pub mod string;
pub mod text;

#[macro_export]
macro_rules! arr {
    ( $( $x:expr ),* $(,)? ) => {
        {
            arrayvec::ArrayVec::from_iter([$($x),*])
        }
    };
}

pub fn strip_arg<U: AsRef<std::ffi::OsStr>>(args: &mut Vec<U>) -> bool {
    let mut found = false;
    let mut past_double_dash = false;

    args.retain(|arg| {
        if past_double_dash {
            return true;
        }

        if arg.as_ref() == "--" {
            past_double_dash = true;
            return true;
        }

        if arg.as_ref() == "--no-heading" {
            found = true;
            return false; // remove it
        }

        true
    });

    found
}
