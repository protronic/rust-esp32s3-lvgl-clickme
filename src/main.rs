// Place pub before mod otherwise youu will get warnings about multiple methods not used in lcd_panel
pub mod gt911;
pub mod lcd_panel;

use log::*;

use cstr_core::CString;
use std::time::Instant;
use std::{cell::RefCell, thread};

use esp_idf_svc::hal::{
    delay::{Ets, FreeRtos},
    gpio::{self, PinDriver},
    i2c::{I2cConfig, I2cDriver},
    ledc::{
        config::TimerConfig,
        {LedcDriver, LedcTimerDriver},
    },
    peripherals::Peripherals,
    units::FromValueType,
};

use lvgl::style::Style;
use lvgl::widgets::{Btn, Label};
use lvgl::{Align, Color, Display, DrawBuffer, Part, Widget};

use embedded_graphics_core::prelude::Point;
use lvgl::input_device::{
    pointer::{Pointer, PointerInputData},
    InputDriver,
};

use crate::gt911::{TouchState, GT911};
use crate::lcd_panel::{EspLcdRgbPanel, RgbPanelConfigBuilder};

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("=================== Starting APP! =========================");

    let peripherals = Peripherals::take()?;

    #[allow(unused)]
    let pins = peripherals.pins;

    //============================================================================================================
    //               Create the I2C to communicate with the touchscreen controller
    //============================================================================================================
    info!("Create I2C");
    let i2c = peripherals.i2c0;
    let sda = pins.gpio19;
    let scl = pins.gpio20;
    let config = I2cConfig::new().baudrate(100.kHz().into());
    let i2c = I2cDriver::new(i2c, sda, scl, &config)?;

    //============================================================================================================
    //               Create the LedcDriver to drive the backlight on the Lcd Panel
    //============================================================================================================

    info!("Create LedcDriver");
    let mut channel = LedcDriver::new(
        peripherals.ledc.channel0,
        LedcTimerDriver::new(
            peripherals.ledc.timer0,
            &TimerConfig::new().frequency(25.kHz().into()),
        )
        .unwrap(),
        pins.gpio2,
    )?;
    //channel.set_duty(channel.get_max_duty() / 2)?;
    channel.set_duty(channel.get_max_duty())?;
    info!("Backlight turned on");

    //============================================================================================================
    //               Create thread for Lvgl and User Interface
    //============================================================================================================
    // Stack size value - 50,000 for 10 lines, 60,000 for 12 lines
    let _lvgl_thread = thread::Builder::new().stack_size(24000).spawn(move || {
        // LCD timing parameters for the 4D Systems 4DLCD-70800480 (7" 800x480 RGB panel)
        // Datasheet: https://resources.4dsystems.com.au/datasheets/4dlcd/4DLCD-70800480/
        //
        // Timing characteristics (typical values from datasheet):
        //   Pixel clock  : 33.3 MHz
        //   HSYNC (HPW)  : 20 PCLK   back porch (HBP): 182 PCLK   front porch (HFP): 210 PCLK
        //   VSYNC (VPW)  : 14 lines  back porch (VBP): 23 lines   front porch (VFP): 22 lines
        //   HSYNC/VSYNC  : active-low (idle high)
        //   DE           : active-high
        //   PCLK         : data sampled on rising edge (pclk_active_neg = false)
        //
        // GPIO assignments depend on your PCB / adapter board wiring:
        //   HSYNC: GPIO39  VSYNC: GPIO40  DE: GPIO41  PCLK: GPIO42
        //   RGB565 data lines [B3-B7, G2-G7, R3-R7]:
        //   [15,  7,  6,  5,  4,    9, 46,  3,  8, 16,  1,   14, 21, 47, 48, 45]
        info!("Create LCD panel");
        let lcd_panel_config = RgbPanelConfigBuilder::new()
            .h_res(800)
            .v_res(480)
            .pclk_hz(33_300_000)       // 33.3 MHz typical pixel clock for 4DLCD-70800480
            .hsync_pulse_width(20)     // HPW typ 20 PCLK
            .hsync_back_porch(182)     // HBP typ 182 PCLK
            .hsync_front_porch(210)    // HFP typ 210 PCLK
            .vsync_pulse_width(14)     // VPW typ 14 lines
            .vsync_back_porch(23)      // VBP typ 23 lines
            .vsync_front_porch(22)     // VFP typ 22 lines
            .hsync_idle_low(false)     // HSYNC active-low (idle high)
            .vsync_idle_low(false)     // VSYNC active-low (idle high)
            .de_idle_high(false)       // DE active-high
            .pclk_active_neg(false)    // data sampled on rising edge of PCLK
            .pclk_idle_high(false)
            .clk_src_ppl240m(true)     // CONFIG_ESP_DEFAULT_CPU_FREQ_MHZ_240=y must be set in sdkconfig.defaults
            .data_width(16)
            .bits_per_pixel(0)
            .num_fbs(1)
            .bounce_buffer_size_px(3200)
            .sram_trans_align(8)
            .dma_burst_size(64)
            .hsync_gpio_num(39)
            .vsync_gpio_num(40)
            .de_gpio_num(41)
            .pclk_gpio_num(42)
            .disp_gpio_num(-1)
            // GPIO assignments for RGB565 data lines - adjust to match your PCB wiring
            // [B3, B4, B5, B6, B7,   G2, G3, G4, G5, G6, G7    R3, R4, R5, R6, R7]
            //  [15,  7,  6,  5,  4,    9, 46,  3,  8, 16,  1,   14, 21, 47, 48, 45]
            .data_gpio_nums(&[15, 7, 6, 5, 4, 9, 46, 3, 8, 16, 1, 14, 21, 47, 48, 45])
            .disp_active_low(false)
            .refresh_on_demand(false)
            .fb_in_psram(true)
            .double_fb(false)
            .no_fb(false)
            .bb_invalidate_cache(false)
            .build();

        let mut lcd_panel = EspLcdRgbPanel::new(lcd_panel_config).unwrap();

        // Initialize lvgl
        lvgl::init();

        info!("Registering Display");
        const HOR_RES: u32 = 800;
        const VER_RES: u32 = 480;
        const LINES: u32 = 2; // The number of lines (rows) that will be refreshed  was 12
        let draw_buffer = DrawBuffer::<{ (HOR_RES * LINES) as usize }>::default();
        let display = Display::register(draw_buffer, HOR_RES, VER_RES, |refresh| {
            lcd_panel
                .set_pixels_lvgl_color(
                    refresh.area.x1.into(),
                    refresh.area.y1.into(),
                    (refresh.area.x2 + 1i16).into(),
                    (refresh.area.y2 + 1i16).into(),
                    refresh.colors,
                )
                .unwrap();
        })
        .unwrap();

        //======================================================================================================
        //                          Create the driver for the Touchscreen
        //======================================================================================================
        info!("Creating Touchscreen");
        //let gt911_touchscreen = RefCell::new(GT911::new(i2c, rst, Ets));
        //gt911_touchscreen.borrow_mut().reset().unwrap();
        let touchscreen = RefCell::new(GT911::new(i2c));
        reset_gt911(pins.gpio38.into());

        // The read_touchscreen_cb is used by Lvgl to detect touchscreen presses and releases
        let read_touchscreen_cb = || {
            // Need to use RefCell here, if we just used gt911_touchscreen.read_touch().unwrap() we will get a
            // compile error -> cannot borrow `read_touchscreen` as mutable, as it is a captured variable in a `Fn` closure
            //
            // From searching the web https://users.rust-lang.org/t/cannot-borrow-write-as-mutable-as-it-is-a-captured-variable-in-a-fn-closure/78506
            // Closures capture their environment - the Fn trait expects its arguments by reference, NOT BY MUTABLE reference.
            // I was using a mutable reference (read_touch) within the closure, that was defined as a mutable reference outside.
            // I orginally had outside the closure this statement -> let mut gt911_touchscreen = GT911::new(i2c, rst, Ets);
            // The solution was to use interior mutability to solve this problem. This means wrapping your mutable reference
            // within a special type (RefCell), that can be shared via an immutable reference, but still allows mutability of its inner value.

            let touch = touchscreen.borrow_mut().read_touch().unwrap();

            match touch {
                TouchState::PRESSED(tp) => {
                    //info!("Pressed");
                    PointerInputData::Touch(Point {
                        x: tp.x as i32,
                        y: tp.y as i32,
                    })
                    .pressed()
                    .once()
                }

                TouchState::RELEASED(tp) => {
                    //info!("Released");
                    PointerInputData::Touch(Point {
                        x: tp.x as i32,
                        y: tp.y as i32,
                    })
                    .released()
                    .once()
                }
            }
        };

        // Register a new input device that's capable of reading the current state of the input
        let _touch_screen = Pointer::register(read_touchscreen_cb, &display).unwrap();

        //=======================================================================================================
        //                               Create the User Interface
        //=======================================================================================================
        info!("Creating UI");
        // Create screen and widgets
        let mut screen = display.get_scr_act().unwrap();
        let mut screen_style = Style::default();
        screen_style.set_bg_color(Color::from_rgb((0, 0, 139)));
        screen_style.set_radius(0);
        screen.add_style(Part::Main, &mut screen_style);

        // Create the button
        let mut button = Btn::create(&mut screen).unwrap();
        button.set_align(Align::LeftMid, 30, 0);
        button.set_size(180, 80);

        // Create button label, align in center of button
        let mut btn_lbl = Label::create(&mut button).unwrap();
        btn_lbl.set_align(Align::Center, 0, 0);
        btn_lbl.set_text(CString::new("Click me!").unwrap().as_c_str());

        let mut btn_state = false;
        button
            .on_event(|_btn, event| {
                if let lvgl::Event::Clicked = event {
                    println!("Clicked Event");
                    if btn_state {
                        let nt = CString::new("Click me!").unwrap();
                        btn_lbl.set_text(nt.as_c_str());
                    } else {
                        let nt = CString::new("Clicked!").unwrap();
                        btn_lbl.set_text(nt.as_c_str());
                    }
                    btn_state = !btn_state;
                }
            })
            .unwrap();

        loop {
            let start = Instant::now();

            lvgl::task_handler();

            // Keep the loop delay short so Lvgl can respond quickly to touchscreen presses and releases
            FreeRtos::delay_ms(5);

            lvgl::tick_inc(Instant::now().duration_since(start));
        }
    })?;

    loop {
        // Don't exit application
        FreeRtos::delay_ms(10000);
    }
}

// Reset the GT911 chip
fn reset_gt911(rst_pin: gpio::AnyOutputPin) {
    let mut rst = PinDriver::output(rst_pin).unwrap();
    rst.set_low().unwrap();
    Ets::delay_us(200);
    rst.set_high().unwrap();
    Ets::delay_ms(5);
}
