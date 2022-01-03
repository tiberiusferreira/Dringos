use log;
use serialport::ClearBuffer;
use std::io::{Read, Write};

pub struct Pzem {
    uart: Box<dyn serialport::SerialPort>,
}

impl Pzem {
    pub fn new<T: Into<Box<dyn serialport::SerialPort>>>(serialport: T) -> Self {
        Self {
            uart: serialport.into(),
        }
    }

    pub fn send_request_get_response(&mut self, request: [u8; 7]) -> std::io::Result<[u8; 7]> {
        self.uart.write_all(&request)?;
        self.uart.flush()?;
        let mut response = [0; 7];
        self.uart.read_exact(&mut response)?;
        if self.uart.bytes_to_read()? != 0 {
            log::warn!("Found extra bytes in read buffer, clearing them!");
            self.uart.clear(ClearBuffer::Input)?;
        }
        let checksum_byte = response[6];
        let sum = response[0..(response.len() - 2)]
            .iter()
            .fold(0 as u8, |acc, curr| acc.wrapping_add(*curr));
        if checksum_byte != sum {
            log::error!("Checksum failed");
            std::io::Error::new(std::io::ErrorKind::Other, "Pzem packet checksum failed");
        }
        Ok(response)
    }

    pub fn read_voltage(&mut self) -> std::io::Result<f32> {
        let read_voltage_packet = [0xB0, 0xC0, 0xA8, 0x01, 0x01, 0x00, 0x1A];
        let response = self.send_request_get_response(read_voltage_packet)?;
        let integer_part = response[2];
        let decimal = response[3];
        let voltage = f32::from(integer_part) + f32::from(decimal) / 10.;
        Ok(voltage)
    }

    pub fn read_power(&mut self) -> std::io::Result<u32> {
        let read_power_packet = [0xB2, 0xC0, 0xA8, 0x01, 0x01, 0x00, 0x1C];
        let response = self.send_request_get_response(read_power_packet)?;
        let power = u32::from_le_bytes([response[2], response[1], 0, 0]);
        Ok(power)
    }
    pub fn read_current(&mut self) -> std::io::Result<f32> {
        let read_power_packet = [0xB1, 0xC0, 0xA8, 0x01, 0x01, 0x00, 0x1B];
        let response = self.send_request_get_response(read_power_packet)?;
        let integer = response[2];
        let decimal = response[3];
        let current = f32::from(integer) + f32::from(decimal) / 10.;
        Ok(current)
    }
}
