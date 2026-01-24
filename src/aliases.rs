use crate::run::item::PathItem;
use matchmaker::nucleo::Indexed;

pub type MMState<'a, 'b> = matchmaker::render::MMState<'a, 'b, Indexed<PathItem>, PathItem>;
