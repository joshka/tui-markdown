use std::panic;

use color_eyre::{
    config::{EyreHook, HookBuilder, PanicHook},
    eyre,
};

use crate::tui;

/// This replaces the standard color_eyre panic and error hooks with hooks that
/// restore the terminal before printing the panic or error.
pub fn install_hooks() -> color_eyre::Result<()> {
    let (panic_hook, eyre_hook) = HookBuilder::default()
        .panic_section(
            "NOTE: Please check the TODO section in the README for a list of missing features.",
        )
        .into_hooks();
    install_panic_hook(panic_hook);
    install_eyre_hook(eyre_hook)?;
    Ok(())
}

fn install_panic_hook(hook: PanicHook) {
    // convert from a color_eyre PanicHook to a standard panic hook
    let hook = hook.into_panic_hook();
    panic::set_hook(Box::new(move |panic_info| {
        tui::restore().unwrap();
        hook(panic_info);
    }));
}

fn install_eyre_hook(hook: EyreHook) -> color_eyre::Result<()> {
    // convert from a color_eyre EyreHook to a eyre ErrorHook
    let hook = hook.into_eyre_hook();
    eyre::set_hook(Box::new(
        move |error: &(dyn std::error::Error + 'static)| {
            tui::restore().unwrap();
            hook(error)
        },
    ))?;

    Ok(())
}
