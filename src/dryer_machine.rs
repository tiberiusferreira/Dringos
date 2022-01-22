use crate::dryer_machine::energy_switch::EnergySwitch;
use dringos::pzemv1::Pzem;
use std::ops::Deref;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, RwLock};
use std::time::Duration;

mod energy_switch;
const IDLE_TIME_CONSIDER_DONE_S: u64 = 30;

#[derive(Debug)]
pub struct OffState {
    pzem: dringos::pzemv3::Pzem,
    switch: energy_switch::EnergySwitch,
}

impl OffState {
    pub fn new() -> OffState {
        let usb_port = "/dev/ttyUSB0";
        let port = serialport::new(usb_port, 9600)
            .timeout(Duration::from_millis(2000))
            .open()
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Cant connect to usb: {}", e.to_string()),
                )
            })
            .unwrap_or_else(|e| panic!("Cannot open `{}`: {}.", usb_port, e));
        let mut pzem = dringos::pzemv3::Pzem::new(port);
        Self {
            pzem,
            switch: EnergySwitch::new(),
        }
    }
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
