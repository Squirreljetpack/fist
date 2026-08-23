use matchmaker::{
    MatchError, MatchResultExt, Matchmaker, SSS,
    nucleo::{Render, Worker},
};

use crate::cli::SubTool;

pub async fn mm_get<T: SSS + Render + Clone>(
    items: impl IntoIterator<Item = T>
) -> Result<T, MatchError> {
    let worker = Worker::new_single_column();
    worker.append(items);
    let mm = Matchmaker::new_on_cloneable(worker);

    mm.pick_default().await.abort().first()
}

impl Render for SubTool {
    fn as_str(&self) -> std::borrow::Cow<'_, str> {
        self.to_string().into()
    }
}
