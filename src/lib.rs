#![no_std]

use embassy_time::Timer;
use embedded_hal_async::i2c::I2c;
pub const HDC1080I2C_ADDRESS: u8 = 0x40;
pub const HDC1080I2C_TMP_REG: u8 = 0x00;
pub const HDC1080I2C_HUM_REG: u8 = 0x01;
pub const HDC1080I2C_CONFIG_REG: u8 = 0x02;

pub struct Hdc1080<DRIVER> {
    driver: DRIVER,
    address: u8,
}

impl<DRIVER: I2c> Hdc1080<DRIVER> {
    pub fn new(driver: DRIVER) -> Hdc1080<DRIVER> {
        Hdc1080 {
            driver,
            address: HDC1080I2C_ADDRESS,
        }
    }

    pub async fn begin(&mut self) -> Result<(), DRIVER::Error> {
        Timer::after_millis(15).await;
        let buffer: [u8; 3] = [HDC1080I2C_CONFIG_REG, 0x0, 0x0];
        self.driver.write(self.address, &buffer).await?;
        Ok(())
    }

    pub async fn get_temp(&mut self) -> Result<f32, DRIVER::Error> {
        let buffer: [u8; 1] = [HDC1080I2C_TMP_REG];
        self.driver.write(self.address, &buffer).await?;
        Timer::after_millis(8).await;
        let mut read_buf: [u8; 2] = [0; 2];
        self.driver.read(self.address, &mut read_buf).await?;
        let temp_c =
            (((read_buf[0] as u16) << 8 | read_buf[1] as u16) as f32 / 65536.0) * 165.0 - 40.0;
        let temp_f = temp_c * 9.0 / 5.0 + 32.0;
        Ok(temp_f)
    }

    pub async fn get_humidity(&mut self) -> Result<f32, DRIVER::Error> {
        let write_buf: [u8; 1] = [HDC1080I2C_HUM_REG];
        self.driver.write(self.address, &write_buf).await?;
        Timer::after_millis(8).await;
        let mut read_buf: [u8; 2] = [0; 2];
        self.driver.read(self.address, &mut read_buf).await?;
        let rh = (((read_buf[0] as u16) << 8 | read_buf[1] as u16) as f32 / 65536.0) * 100.0;
        Ok(rh)
    }

    pub async fn get_humidity_new(&mut self) -> Result<f32, DRIVER::Error> {
        // let write_buf: [u8; 1] = [HDC1080I2C_HUM_REG];
        // self.driver.write(self.address, &write_buf).await?;
        Timer::after_millis(8).await;
        let mut read_buf: [u8; 2] = [0; 2];
        self.driver.read(self.address, &mut read_buf).await?;
        let rh = (((read_buf[0] as u16) << 8 | read_buf[1] as u16) as f32 / 65536.0) * 100.0;
        Ok(rh)
    }
}
