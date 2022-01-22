#[derive(Debug)]
pub struct EnergySwitch {
    gpio: (),
}

impl EnergySwitch {
    pub fn new() -> Self {
        Self { gpio: () }
    }
    pub fn turn_on(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
    pub fn turn_off(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}
