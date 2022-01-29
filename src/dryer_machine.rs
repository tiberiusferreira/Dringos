use crate::dryer_machine::energy_switch::EnergySwitch;
use std::time::Duration;

mod energy_switch;

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
        let pzem = dringos::pzemv3::Pzem::new(port);
        let mut switch = EnergySwitch::new();
        switch.turn_off();
        Self { pzem, switch }
    }
    pub fn turn_on_and_reset_energy_counter(mut self) -> OnState {
        // if we failed to turn it on, try to turn it off again
        self.switch.turn_on();
        self.pzem
            .reset_consumed_energy()
            .expect("Error resetting consumed energy!");
        OnState {
            pzem: self.pzem,
            switch: self.switch,
        }
    }
}

impl OnState {
    pub fn get_consumed_energy_wh(&mut self) -> u32 {
        let data = self.pzem.read_data().expect("Error reading pzem data!");
        data.energy_wh
    }
    pub fn get_current_power(&mut self) -> f32 {
        let data = self.pzem.read_data().expect("Error reading pzem data!");
        data.power
    }
    pub fn reset_consumed_energy(&mut self) {
        self.pzem
            .reset_consumed_energy()
            .expect("Error reseting pzem consumed energy!");
    }
    pub fn turn_off_and_reset_energy_counter(mut self) -> OffState {
        self.switch.turn_off();
        self.reset_consumed_energy();
        OffState {
            pzem: self.pzem,
            switch: self.switch,
        }
    }
}

#[derive(Debug)]
pub struct OnState {
    pzem: dringos::pzemv3::Pzem,
    switch: energy_switch::EnergySwitch,
}
