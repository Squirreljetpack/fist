use cba::{StringError, bait::ResultExt};

#[cfg(target_os = "macos")]
pub fn trash(path: &std::path::Path) -> Result<(), StringError> {
    use std::sync::OnceLock;
    use trash::{
        TrashContext,
        macos::{DeleteMethod, TrashContextExtMacos},
    };

    static CTX: OnceLock<TrashContext> = OnceLock::new();

    let ctx = CTX.get_or_init(|| {
        let mut ctx = TrashContext::default();
        ctx.set_delete_method(DeleteMethod::NsFileManager);
        ctx
    });

    ctx.delete(path).cast_()
}

#[cfg(not(target_os = "macos"))]
pub fn trash(path: &std::path::Path) -> Result<(), StringError> {
    trash::delete(path).cast_()
}
