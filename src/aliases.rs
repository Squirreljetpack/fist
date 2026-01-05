use crate::run::item::PathItem;
use matchmaker::nucleo::Indexed;

pub type MMState<'a> = matchmaker::render::MMState<'a, Indexed<PathItem>, PathItem>;
