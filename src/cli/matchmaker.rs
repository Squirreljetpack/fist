use matchmaker::{
    MatchError, MatchResultExt, Matchmaker, SSS, Selector,
    nucleo::{Indexed, Render, Worker},
};

use crate::cli::SubTool;

pub async fn mm_get<T: SSS + Render + Clone>(
    items: impl IntoIterator<Item = T>
) -> Result<T, MatchError> {
    let worker = Worker::new_single_column();
    worker.append(items);
    let selector = Selector::new(Indexed::identifier);
    let mm = Matchmaker::new(worker, selector);

    mm.pick_default().await.abort().first()
}

impl Render for SubTool {
    fn as_str(&self) -> std::borrow::Cow<'_, str> {
        self.to_string().into()
    }
}
