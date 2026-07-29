use crate::run::item::PathItem;
pub type MMState<'a, 'b> = matchmaker::render::MMState<'a, 'b, PathItem, ()>;
