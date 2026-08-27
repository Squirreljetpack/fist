use matchmaker::{
    HelpFactory, Text, config::HelpDisplayConfig, message::Interrupt, resolve_static_preview,
    utils::text_to_ansi,
};

use crate::run::{FsMatchmaker, register::ExecutionMode};

pub(super) fn register_help_handler(
    mm: &mut FsMatchmaker,
    help_factory: HelpFactory,
    help_config: HelpDisplayConfig,
) {
    mm.register_interrupt_handler(Interrupt::Execute, move |state| {
        if state.discriminant_payload != Some(ExecutionMode::Help.discriminant()) {
            return;
        }
        state.discriminant_payload = None;
        state.clear_interrupt();

        let resolved = resolve_static_preview(&Text::default(), &help_factory, &help_config);
        let ansi_text = text_to_ansi(&resolved);
        let _ = crate::pager::page_reader(std::io::Cursor::new(ansi_text.into_bytes()), true, None);
    });
}
