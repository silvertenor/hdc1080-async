#![no_std]

use defmt::Format;
use embassy_time::Timer;
use embedded_hal_async::i2c::I2c;
pub const HDC1080I2C_ADDRESS: u8 = 0x40;
pub const HDC1080I2C_TMP_REG: u8 = 0x00;
pub const HDC1080I2C_HUM_REG: u8 = 0x01;
pub const HDC1080I2C_CONFIG_REG: u8 = 0x02;

#[derive(Format)]
pub struct Hdc1080<DRIVER> {
    driver: DRIVER,
    address: u8,
    config: Config,
}
pub enum Heat {
    Enabled,
    Disabled,
}
#[derive(Format)]
pub enum AcquisitionMode {
    Individual,
    Sequential,
}

#[derive(Format)]
pub enum TRes {
    High,
    Low,
}
#[derive(Format)]
pub enum HRes {
    High,
    Mid,
    Low,
}
struct ConfigRegisterFields;
impl ConfigRegisterFields {
    const RST: u16 = 1 << 15;
    const HEAT: u16 = 1 << 13;
    const MODE: u16 = 1 << 12;
    const TRES: u16 = 1 << 10;
    const HRES_9: u16 = 1 << 9;
    const HRES_8: u16 = 1 << 8;
}

#[derive(Format)]
pub struct Config {
    bits: u16,
    mode: AcquisitionMode,
    temp_res: TRes,
    hum_res: HRes,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bits: 0x0 | ConfigRegisterFields::MODE,
            mode: AcquisitionMode::Sequential,
            temp_res: TRes::High,
            hum_res: HRes::High,
        }
    }
}

impl Config {
    fn set_bits(&mut self, mask: u16) {
        self.bits |= mask
    }
    fn clear_bits(&mut self, mask: u16) {
        self.bits &= !mask
    }
    pub fn reset(&mut self) {
        self.set_bits(ConfigRegisterFields::RST);
    }
    pub fn set_heater(&mut self, heat: Heat) {
        match heat {
            Heat::Disabled => self.clear_bits(ConfigRegisterFields::HEAT),
            Heat::Enabled => self.set_bits(ConfigRegisterFields::HEAT),
        }
    }
    pub fn set_acqui_mode(&mut self, mode: AcquisitionMode) {
        match mode {
            AcquisitionMode::Individual => self.clear_bits(ConfigRegisterFields::MODE),
            AcquisitionMode::Sequential => self.set_bits(ConfigRegisterFields::MODE),
        }
        self.mode = mode;
    }
    pub fn set_temp_resolution(&mut self, res: TRes) {
        match res {
            TRes::High => self.clear_bits(ConfigRegisterFields::TRES),
            TRes::Low => self.set_bits(ConfigRegisterFields::TRES),
        }
        self.temp_res = res;
    }
    pub fn set_humidity_resolution(&mut self, res: HRes) {
        match res {
            HRes::High => {
                self.clear_bits(ConfigRegisterFields::HRES_9 | ConfigRegisterFields::HRES_8)
            }
            HRes::Mid => {
                self.clear_bits(ConfigRegisterFields::HRES_9);
                self.set_bits(ConfigRegisterFields::HRES_8);
            }
            HRes::Low => {
                self.set_bits(ConfigRegisterFields::HRES_9 | ConfigRegisterFields::HRES_8);
            }
        }
        self.hum_res = res;
    }
}

enum Acquisition {
    Temp,
    Humidity,
    Both,
}

#[derive(Format)]
pub struct Measurement {
    pub temp: Option<f32>,
    pub humidity: Option<f32>,
}
impl Default for Measurement {
    fn default() -> Self {
        Measurement {
            temp: None,
            humidity: None,
        }
    }
}
impl<DRIVER: I2c> Hdc1080<DRIVER> {
    pub fn new(driver: DRIVER) -> Hdc1080<DRIVER> {
        Hdc1080 {
            driver,
            address: HDC1080I2C_ADDRESS,
            config: Config::default(),
        }
    }
    pub async fn set_config(&mut self, config: Config) -> Result<(), DRIVER::Error> {
        self.config = config;
        let buf: [u8; 3] = [HDC1080I2C_CONFIG_REG, (self.config.bits >> 8) as u8, 0x0];
        self.driver.write(self.address, &buf).await?;
        Ok(())
    }
    pub async fn read_config(&mut self) -> Result<[u8; 2], DRIVER::Error> {
        let buffer: [u8; 1] = [HDC1080I2C_CONFIG_REG];
        self.driver.write(self.address, &buffer).await?;
        let mut buf: [u8; 2] = [0; 2];
        self.driver.read(self.address, &mut buf).await?;
        Ok(buf)
    }
    fn wait_us(&self, acqui: Acquisition) -> u64 {
        let mut delay: u64 = 0;
        let t_delay: u64 = match self.config.temp_res {
            TRes::High => 6350,
            TRes::Low => 3650,
        };
        let h_delay: u64 = match self.config.hum_res {
            HRes::High => 6500,
            HRes::Mid => 3850,
            HRes::Low => 2500,
        };
        delay += match acqui {
            Acquisition::Temp => t_delay,
            Acquisition::Humidity => h_delay,
            Acquisition::Both => t_delay + h_delay,
        };
        delay
    }
    pub async fn measure(&mut self) -> Result<Measurement, DRIVER::Error> {
        let mut measurement = Measurement::default();
        match self.config.mode {
            AcquisitionMode::Individual => {
                measurement.temp = Some(self.get_temp().await?);
                measurement.humidity = Some(self.get_humidity().await?);
                Ok(measurement)
            }
            AcquisitionMode::Sequential => self.get_both().await,
        }
    }
    async fn get_both(&mut self) -> Result<Measurement, DRIVER::Error> {
        let buffer: [u8; 1] = [HDC1080I2C_TMP_REG];
        self.driver.write(self.address, &buffer).await?;
        Timer::after_micros(self.wait_us(Acquisition::Both)).await;
        let mut read_buf: [u8; 4] = [0; 4];
        self.driver.read(self.address, &mut read_buf).await?;
        let t_buf: [u8; 2] = read_buf[0..2].try_into().unwrap();
        let h_buf: [u8; 2] = read_buf[2..].try_into().unwrap();
        let temp_c = self.convert(t_buf, Acquisition::Temp);
        // let temp_f = temp_c.unwrap() * 9.0 / 5.0 + 32.0;
        let rh = self.convert(h_buf, Acquisition::Humidity);
        Ok(Measurement {
            temp: temp_c,
            humidity: rh,
        })
    }

    fn convert(&self, bytes: [u8; 2], acqui: Acquisition) -> Option<f32> {
        match acqui {
            Acquisition::Temp => {
                let temp_c =
                    (((bytes[0] as u16) << 8 | bytes[1] as u16) as f32 / 65536.0) * 165.0 - 40.0;
                Some(temp_c)
            }
            Acquisition::Humidity => {
                Some((((bytes[0] as u16) << 8 | bytes[1] as u16) as f32 / 65536.0) * 100.0)
            }
            Acquisition::Both => None,
        }
    }
    pub async fn get_temp(&mut self) -> Result<f32, DRIVER::Error> {
        let buffer: [u8; 1] = [HDC1080I2C_TMP_REG];
        self.driver.write(self.address, &buffer).await?;
        Timer::after_micros(self.wait_us(Acquisition::Temp)).await;
        let mut read_buf: [u8; 2] = [0; 2];
        self.driver.read(self.address, &mut read_buf).await?;
        let temp_c = self.convert(read_buf, Acquisition::Temp);
        // let temp_f = temp_c.unwrap() * 9.0 / 5.0 + 32.0;
        Ok(temp_c.unwrap())
    }
    pub async fn get_humidity(&mut self) -> Result<f32, DRIVER::Error> {
        let write_buf: [u8; 1] = [HDC1080I2C_HUM_REG];
        self.driver.write(self.address, &write_buf).await?;
        Timer::after_micros(self.wait_us(Acquisition::Humidity)).await;
        let mut read_buf: [u8; 2] = [0; 2];
        self.driver.read(self.address, &mut read_buf).await?;
        defmt::info!("{:?}", read_buf);
        let rh = self.convert(read_buf, Acquisition::Humidity);
        Ok(rh.unwrap())
    }
}
