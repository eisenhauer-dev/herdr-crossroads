mod discovery;
mod palette;
mod persistence;

pub(crate) use discovery::auto_choice;
#[cfg(test)]
pub(crate) use discovery::{auto_choice_at, saved_choice};
pub(crate) use palette::{Theme, NAMES};
#[cfg(test)]
pub(crate) use persistence::load;
pub(crate) use persistence::{load_diagnostics, save};
