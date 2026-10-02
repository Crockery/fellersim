mod apl;
mod ardeos;
mod cast;
mod elarion;
mod elarion_swing;
mod gunde;
mod gunde_swing;
mod impact;
mod mara;
mod rime;
mod tariq;
mod tariq_swing;

pub(crate) use ardeos::{ArdeosEvent, ArdeosState};
pub(crate) use elarion::{ElarionEvent, ElarionState};
pub(crate) use gunde::{GundeEvent, GundeState};
pub(crate) use mara::{MaraEvent, MaraState};
pub(crate) use rime::{RimeEvent, RimeState, RimeTriggeredDamageSource};
pub(crate) use tariq::{TariqEvent, TariqState};

#[derive(Debug)]
pub(crate) enum HeroState {
    Ardeos(ArdeosState),
    Rime(Box<RimeState>),
    Tariq(TariqState),
    Elarion(ElarionState),
    Mara(MaraState),
    Gunde(GundeState),
}

impl HeroState {
    pub(crate) fn ardeos(&self) -> &ArdeosState {
        let Self::Ardeos(state) = self else {
            panic!("Ardeos state is only available to the Ardeos runtime");
        };
        state
    }

    pub(crate) fn ardeos_mut(&mut self) -> &mut ArdeosState {
        let Self::Ardeos(state) = self else {
            panic!("Ardeos state is only available to the Ardeos runtime");
        };
        state
    }

    pub(crate) fn rime(&self) -> &RimeState {
        let Self::Rime(state) = self else {
            panic!("Rime state is only available to the Rime runtime");
        };
        state
    }

    pub(crate) fn rime_mut(&mut self) -> &mut RimeState {
        let Self::Rime(state) = self else {
            panic!("Rime state is only available to the Rime runtime");
        };
        state
    }

    pub(crate) fn tariq(&self) -> &TariqState {
        let Self::Tariq(state) = self else {
            panic!("Tariq state is only available to the Tariq runtime");
        };
        state
    }

    pub(crate) fn tariq_mut(&mut self) -> &mut TariqState {
        let Self::Tariq(state) = self else {
            panic!("Tariq state is only available to the Tariq runtime");
        };
        state
    }

    pub(crate) fn elarion(&self) -> &ElarionState {
        let Self::Elarion(state) = self else {
            panic!("Elarion state is only available to the Elarion runtime");
        };
        state
    }

    pub(crate) fn elarion_mut(&mut self) -> &mut ElarionState {
        let Self::Elarion(state) = self else {
            panic!("Elarion state is only available to the Elarion runtime");
        };
        state
    }

    pub(crate) fn mara(&self) -> &MaraState {
        let Self::Mara(state) = self else {
            panic!("Mara state is only available to the Mara runtime");
        };
        state
    }

    pub(crate) fn mara_mut(&mut self) -> &mut MaraState {
        let Self::Mara(state) = self else {
            panic!("Mara state is only available to the Mara runtime");
        };
        state
    }

    pub(crate) fn gunde(&self) -> &GundeState {
        let Self::Gunde(state) = self else {
            panic!("Gunde state is only available to the Gunde runtime");
        };
        state
    }

    pub(crate) fn gunde_mut(&mut self) -> &mut GundeState {
        let Self::Gunde(state) = self else {
            panic!("Gunde state is only available to the Gunde runtime");
        };
        state
    }
}
