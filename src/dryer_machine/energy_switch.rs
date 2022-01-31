use gpio_cdev::{LineHandle, LineRequestFlags};

#[derive(Debug)]
pub struct EnergySwitch {
    switch_gpio: LineHandle,
}

impl EnergySwitch {
    pub fn new() -> Self {
        let mut chip =
            gpio_cdev::Chip::new("/dev/gpiochip0").expect("Error initializing GPIO Chip!");
        let switch_gpio = chip
            .get_line(26)
            .expect("Error initializing GPIO switch line!")
            .request(LineRequestFlags::OUTPUT, 0, "dryer-switch")
            .expect("Error initializing GPIO switch line as output!");
        switch_gpio
            .set_value(0)
            .expect("Error setting dryer switch off");

        Self { switch_gpio }
    }
    pub fn turn_on(&mut self) {
        self.switch_gpio
            .set_value(1)
            .expect("Error setting dryer on!")
    }
    pub fn turn_off(&mut self) {
        self.switch_gpio
            .set_value(0)
            .expect("Error setting dryer off!")
    }
}
