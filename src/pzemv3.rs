use crc::Algorithm;
use log;
use serialport::ClearBuffer;
use std::fmt::Formatter;
use std::io::{Error, ErrorKind, Read, Write};
pub struct Pzem {
    uart: Box<dyn serialport::SerialPort>,
}

impl std::fmt::Debug for Pzem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pzem")
            .field("uart", &"Box<dyn serialport::SerialPort>")
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct Data {
    pub voltage: f32,
    pub current: f32,
    pub power: f32,
    pub energy_wh: u32,
    pub frequency: f32,
    pub power_factor: f32,
    pub alarm: f32,
}

impl Pzem {
    pub fn new<T: Into<Box<dyn serialport::SerialPort>>>(serialport: T) -> Self {
        Self {
            uart: serialport.into(),
        }
    }

    pub fn reset_consumed_energy(&mut self) -> Result<(), Error> {
        let mut request = [0x01, 0x42, 0x0, 0x0];
        Self::crc_write(request.as_mut_slice());
        self.uart.write_all(&request)?;
        self.uart.flush()?;
        let mut response = [0; 4];
        self.uart.read_exact(&mut response)?;
        if response[1] != 0x42 {
            self.uart.clear(ClearBuffer::Input)?;
            return Err(std::io::Error::new(
                ErrorKind::Other,
                format!("Internal Pzem Error Code: {}", response[2]),
            ));
        }
        if !Self::crc_is_valid(&response) {
            return Err(std::io::Error::new(
                ErrorKind::Other,
                format!("Pzem returned an invalid CRC value"),
            ));
        }
        if self.read_data()?.energy_wh != 0 {
            return Err(std::io::Error::new(
                ErrorKind::Other,
                format!("Pzem returned a non zero energy value after reset"),
            ));
        }
        Ok(())
    }
    pub fn read_data(&mut self) -> Result<Data, Error> {
        // [Slave Address (0x01 ~ 0xF7), 0x04, Register Address High Byte, Register Address Low Byte, Number of Registers High Byte, Number of Registers Low Byte, CRC Check High Byte, CRC Check Low Byte]
        // Read Registers 0 to 11 from slave 1
        let mut request = [0x01, 0x04, 0x00, 0x00, 0x00, 0x0A, 0x0, 0x0];
        Self::crc_write(request.as_mut_slice());
        self.uart.write_all(&request)?;
        self.uart.flush()?;
        let mut response = [0; 25];
        self.uart.read_exact(&mut response[0..3])?;

        // if the second byte isn't 0x04 it means it we got an error and the error
        // itself is specified by the third byte or corrupt msg
        if response[1] != 0x04 {
            self.uart.clear(ClearBuffer::Input)?;
            return Err(std::io::Error::new(
                ErrorKind::Other,
                format!("Internal Pzem Error Code: {}", response[2]),
            ));
        }
        // Since we requested
        if response[2] != 20 {
            return Err(std::io::Error::new(
                ErrorKind::Other,
                format!("Pzem returned a different amount of data than requested. Should have been 20 bytes, was: {}", response[2]),
            ));
        }
        // 2 last bytes are CRC
        self.uart.read_exact(&mut response[3..])?;
        if !Self::crc_is_valid(&response) {
            return Err(std::io::Error::new(
                ErrorKind::Other,
                format!("Pzem returned an invalid CRC value"),
            ));
        }
        if self.uart.bytes_to_read()? != 0 {
            log::warn!("Had bytes leftover in usb line!");
            self.uart.clear(ClearBuffer::All)?;
        }
        let response = &response[3..];
        let data = Data {
            voltage: u16::from_be_bytes([response[0], response[1]]) as f32 / 10.,
            current: u32::from_be_bytes([response[4], response[5], response[2], response[3]])
                as f32
                / 1000.,
            power: u32::from_be_bytes([response[8], response[9], response[6], response[7]]) as f32
                / 10.,
            energy_wh: u32::from_be_bytes([response[12], response[13], response[10], response[11]]),
            frequency: u16::from_be_bytes([response[14], response[15]]) as f32 / 10.,
            power_factor: u16::from_be_bytes([response[16], response[17]]) as f32 / 100.,
            alarm: u16::from_be_bytes([response[18], response[19]]) as f32,
        };
        Ok(data)
    }

    // 16-bit cyclic redundancy check (CRC).
    fn crc_write(buf: &mut [u8]) {
        let n = buf.len();
        assert!(n > 3, "Need at least 3 bytes to calculate the CRC check");
        // this results in the bytes possibly "inverted", we later make it big endian
        let crc = crc::Crc::<u16>::new(&crc::CRC_16_MODBUS);
        let mut digest = crc.digest();
        digest.update(&buf[0..n - 2]);
        let final_res = digest.finalize().to_be().to_be_bytes();
        buf[n - 2] = final_res[0];
        buf[n - 1] = final_res[1];
    }

    // Returns true if the CRC is valid
    fn crc_is_valid(buf: &[u8]) -> bool {
        let n = buf.len();
        assert!(n > 3, "Need at least 3 bytes to calculate the CRC check");
        let crc = crc::Crc::<u16>::new(&crc::CRC_16_MODBUS);
        let mut digest = crc.digest();
        digest.update(&buf[0..n - 2]);
        let final_res = digest.finalize().to_be().to_be_bytes();
        final_res == &buf[n - 2..]
    }
}

#[test]
fn crc_work() {
    let correct_message_with_crc = [
        0x01, 0x4, 0x14, 0x4, 0xEB, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 2, 0x57, 0, 0, 0, 0, 0x18,
        0xB8,
    ];
    assert!(Pzem::crc_is_valid(&correct_message_with_crc));
    let mut msg = [
        0x01, 0x4, 0x14, 0x4, 0xEB, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 2, 0x57, 0, 0, 0, 0, 0, 0,
    ];
    Pzem::crc_write(&mut msg);
    let len = msg.len();
    assert_eq!(&correct_message_with_crc[len - 2..], &msg[len - 2..]);
}
