use dringos::pzemv1::Pzem;
use std::ops::Deref;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, RwLock};
use std::time::Duration;
mod energy_switch;
const IDLE_TIME_CONSIDER_DONE_S: u64 = 30;

pub struct DryerMachine {
    pzem: dringos::pzemv3::Pzem,
    switch: energy_switch::EnergySwitch,
    state: State,
}

#[derive(Debug)]
pub enum State {
    Off(OffState),
    On(OnState),
}

#[derive(Debug)]
pub struct OffState {
    pzem: dringos::pzemv3::Pzem,
    switch: energy_switch::EnergySwitch,
}

impl OffState {
    pub fn turn_on(mut self) -> Result<OnState, std::io::Error> {
        self.switch.turn_on()?;
        self.pzem.reset_consumed_energy()?;
        Ok(OnState {
            pzem: self.pzem,
            switch: self.switch,
            energy_wh: 0,
        })
    }
}

impl OnState {
    pub fn get_consumed_energy(&mut self) -> Result<u32, std::io::Error> {
        let data = self.pzem.read_data()?;
        Ok(data.energy_wh)
    }
    pub fn reset_consumed_energy(&mut self) -> Result<(), std::io::Error> {
        self.pzem.reset_consumed_energy()?;
        Ok(())
    }
    pub fn turn_off(mut self) -> Result<OffState, std::io::Error> {
        self.switch.turn_off()?;
        Ok(OffState {
            pzem: self.pzem,
            switch: self.switch,
        })
    }
}

#[derive(Debug)]
pub struct OnState {
    pzem: dringos::pzemv3::Pzem,
    switch: energy_switch::EnergySwitch,
    energy_wh: u32,
}

impl DryerMachine {
    pub fn get_state(&self) -> State {
        // let read_guard = self.state.read().unwrap();
        // read_guard.deref().clone()
        unimplemented!()
    }
    pub fn reset_energy_counter(&mut self) -> Result<(), std::io::Error> {
        self.pzem.reset_consumed_energy()?;
        unimplemented!()
    }
}
