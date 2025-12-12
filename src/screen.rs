use anyhow::Result;

pub trait Screen {
    fn clear(&mut self) -> Result<()>;
    fn refresh(&mut self) -> Result<()>;
    fn draw_text(&mut self, text: &str, x: i32, y: i32);
    fn draw_text_at(&mut self, text: &str, row: i32, col: i32);
    fn sleep(&mut self) -> Result<()>;
}

pub struct NoScreen {}
impl Screen for NoScreen {
    fn clear(&mut self) -> Result<()> {
        Ok(())
    }

    fn refresh(&mut self) -> Result<()> {
        Ok(())
    }

    fn draw_text(&mut self, _text: &str, _x: i32, _y: i32) {}

    fn draw_text_at(&mut self, _text: &str, _row: i32, _col: i32) {}

    fn sleep(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
pub mod epd {
    use super::*;
    use embedded_graphics::{
        mono_font::MonoTextStyleBuilder,
        prelude::*,
        text::{Baseline, Text, TextStyleBuilder},
    };
    use epd_waveshare::{
        color::*,
        epd2in13_v2::{Display2in13, Epd2in13},
        prelude::*,
    };

    use linux_embedded_hal::{
        Delay, SpidevDevice, SysfsPin,
        spidev::{self, SpidevOptions},
        sysfs_gpio::Direction,
    };

    // Check ls /sys/class/gpio -> export gpiochip512 unexport ?
    const GPIO_BASE: u64 = 512;

    pub struct EpdScreen {
        spi: SpidevDevice,
        epd: Epd2in13<SpidevDevice, SysfsPin, SysfsPin, SysfsPin, Delay>,
        display: Display2in13,
    }

    impl EpdScreen {
        pub fn new() -> Result<Self> {
            // Configure SPI
            let mut spi = SpidevDevice::open("/dev/spidev0.0")?;
            let options = SpidevOptions::new()
                .bits_per_word(8)
                .max_speed_hz(4_000_000)
                .mode(spidev::SpiModeFlags::SPI_MODE_0)
                .build();
            spi.configure(&options)?;

            // Configure Digital I/O Pin to be used as Chip Select for SPI
            let cs = SysfsPin::new(GPIO_BASE + 26); //BCM7 CE0
            cs.export()?;
            while !cs.is_exported() {}
            cs.set_direction(Direction::Out)?;
            cs.set_value(1)?;

            let busy = SysfsPin::new(GPIO_BASE + 24); // GPIO 24, board J-18
            busy.export()?;
            while !busy.is_exported() {}
            busy.set_direction(Direction::In)?;

            let dc = SysfsPin::new(GPIO_BASE + 25); // GPIO 25, board J-22
            dc.export().expect("dc export");
            while !dc.is_exported() {}
            dc.set_direction(Direction::Out)?;
            dc.set_value(1)?;

            let rst = SysfsPin::new(GPIO_BASE + 17); // GPIO 17, board J-11
            rst.export()?;
            while !rst.is_exported() {}
            rst.set_direction(Direction::Out)?;
            rst.set_value(1)?;

            let mut delay = Delay {};
            let mut epd = Epd2in13::new(&mut spi, busy, dc, rst, &mut delay, None)?;
            let mut display = Display2in13::default();
            display.set_rotation(DisplayRotation::Rotate90);
            epd.set_refresh(&mut spi, &mut delay, RefreshLut::Quick)
                .unwrap();
            epd.clear_frame(&mut spi, &mut delay).unwrap();

            let _ = display.clear(Color::White);
            epd.update_and_display_frame(&mut spi, display.buffer(), &mut delay)?;

            Ok(Self { spi, epd, display })
        }
    }

    impl Screen for EpdScreen {
        fn clear(&mut self) -> Result<()> {
            let mut delay = Delay {};
            let _ = self.display.clear(Color::White);
            self.epd
                .update_and_display_frame(&mut self.spi, self.display.buffer(), &mut delay)?;

            Ok(())
        }
        fn refresh(&mut self) -> Result<()> {
            let mut delay = Delay {};
            self.epd
                .update_and_display_frame(&mut self.spi, self.display.buffer(), &mut delay)?;

            Ok(())
        }
        fn draw_text(&mut self, text: &str, x: i32, y: i32) {
            let style = MonoTextStyleBuilder::new()
                .font(&embedded_graphics::mono_font::ascii::FONT_6X10)
                .text_color(Color::Black)
                .background_color(Color::White)
                .build();

            let text_style = TextStyleBuilder::new().baseline(Baseline::Top).build();

            let _ = Text::with_text_style(text, Point::new(x, y), style, text_style)
                .draw(&mut self.display);
        }
        fn draw_text_at(&mut self, text: &str, row: i32, col: i32) {
            let padded = format!("{:<42}", text);
            self.draw_text(&padded, col * 6, row * 10);
        }
        fn sleep(&mut self) -> Result<()> {
            let mut delay = Delay {};
            let _ = self.epd.sleep(&mut self.spi, &mut delay);
            Ok(())
        }
    }
}
