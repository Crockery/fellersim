use super::*;
use std::collections::BTreeSet;

include!("support/mod.rs");

mod accuracy;
mod benchmark;
mod contracts;
mod damage_traits;
mod engine;
mod golden;
mod healing;
mod listener_order;
mod preparation;
mod shared_bonuses;
mod stat_traits;
mod weapons;

mod heroes {
    mod ardeos;
    mod elarion;
    mod gunde;
    mod mara;
    mod rime;
    mod tariq;
}
