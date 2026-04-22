# Rust ESP32S3 Lvgl Clickme

The purpose of this demo is to get lv-binding-rust (Lvgl) running on the ESP32S3 development board and to use the touchscreen.

## Display
4D Systems [4DLCD-70800480](https://resources.4dsystems.com.au/datasheets/4dlcd/4DLCD-70800480/) — 7.0" 800×480 24-bit RGB TFT LCD with capacitive touch (GT911 controller).

Connect the display to an ESP32S3 board.  The GPIO assignments in `main.rs` reflect one example wiring; adjust them to match your PCB layout:

| Signal | GPIO |
|--------|------|
| HSYNC  | 39   |
| VSYNC  | 40   |
| DE     | 41   |
| PCLK   | 42   |
| Backlight (PWM) | 2 |
| Touch SDA | 19 |
| Touch SCL | 20 |
| Touch RST | 38 |
| B3–B7, G2–G7, R3–R7 | 15, 7, 6, 5, 4, 9, 46, 3, 8, 16, 1, 14, 21, 47, 48, 45 |

## Overview
This application shows how to use lv-binding-rust crate on a ESP32S3 device along with the touchscreen.  The program will display a large button that shows "Click me!".  When the user clicks the button the button will now show "Clicked!"


## partition-table folder
The partition-table folder contains a file called partitons.csv.  This file increases the default factory/app partiton from the default of 1M to 3M. This allows us more space for our program and since the flash size is 16M this should not be a problem.  This file will be called when we flash the device.

## custom-fonts folder
I left the custom-fonts folder in the project but currenly I am not using a custom but but instead using the LV_FONT_MONTSERRAT_28 enabled in the lv_conf.h file.
I also made the LV_FONT_MONTSERRAT_28 as the LV_FONT_DEFAULT.
The custom-fonts folder contains our custom fonts.  The customs fonts are converted from TTF fonts using lvgl online font converter at https://lvgl.io/tools/fontconverter.  I used https://ttfonts.net to find a font I liked and then downloaded the font.  In the lvgl-online-font-converter I used the font name plus the font size for the name of the font.  I chose Bpp of 2 bit-per-pixel and set the range of 0x30-0x3A since I only need numbers and the ":" character.  After clicking on "Convert" the file will be downloaded. I placed this downloaded file (*.c) into the custom-fonts folder.  Then I created a header file which has an extern to my *.c file, along with changing the ifndef and define names.
To use this custom font, I added ```LVGL_FONTS_DIR = {relative = true, value = "custom-fonts"}``` to my config.toml under [env].  This allows our font to be compiled when lvgl is compiled.

## lvgl-configs folder
The lvgl-configs folder holds the lv_config.h and lv_drv_conf.h files which are required by lvgl to compile.  Everything in lv_drv_conf.h file is set to 0 as I am not using the lvgl drivers. To show memory usage and cpu utilization on display I set #define LV_USE_PERF_MONITOR 1 (line 246) and #define LV_USE_MEM_MONITOR 1 (line 253).  I set #define LV_DISP_DEF_REFR_PERIOD 10  (line 81)


## lcd_panel.rs file
The LCD RGB panel driver.

## gt911.rs file
The GT911 touchscreen controller driver.

## sdkconfig.defaults file
The following needs to be added for using PSRAM.
```
CONFIG_FREERTOS_HZ=1000

CONFIG_SPIRAM=y
CONFIG_SPIRAM_MODE_OCT=y
CONFIG_SPIRAM_SPEED_80M=y

# Enabling the following configurations can help increase the PCLK frequency in the case when
# the Frame Buffer is allocated from the PSRAM and fetched by EDMA
CONFIG_SPIRAM_FETCH_INSTRUCTIONS=y
CONFIG_SPIRAM_RODATA=y

CONFIG_ESP_DEFAULT_CPU_FREQ_MHZ_240=y
```

## Cargo.toml project file
I have the following to the "dependencies" section.
```
[dependencies]
# Logging
log = "0.4"

# ESP specifics
esp-idf-svc = "0.51"

# LVGL
lvgl = { version = "0.6.2", default-features = false, features = [
    "embedded_graphics",
    "unsafe_no_autoinit",
    #"lvgl_alloc",
    #"alloc"
] }

lvgl-sys = { version = "0.6.2" }

# Hardware IO Abstraction Layer
embedded-hal = {version = "1.0.0"}
embedded-graphics-core = "0.4.0"

# Error
anyhow = "1.0"

# C String
cstr_core = "0.2.1"

```

I also included patch.crates-io section to patch lvgl and lvgl-sys, esp-idf-svc, esp-idf-sys, esp-idf-hal
```
[patch.crates-io]
lvgl = { git = "https://github.com/enelson1001/lv_binding_rust"}
lvgl-sys = { git = "https://github.com/enelson1001/lv_binding_rust"}

# Need to use Master branch if using esp-idf greater than version 5.2 (ie v5.4.2, v5.5.1)otherwise esp_lcd_panel_rgb.h is not included
esp-idf-sys = { git =  "https://github.com/esp-rs/esp-idf-sys.git"}

# Need to use Master branch if using esp-idf v5.5.1 or else you will get the following 2 errors
# error[E0422]: cannot find struct, variant or union type `twai_timing_config_t__bindgen_ty_1` in this scope
# error[E0560]: struct `esp_idf_sys::twai_timing_config_t` has no field named `__bindgen_anon_1`
esp-idf-hal = { git =  "https://github.com/esp-rs/esp-idf-hal.git"}
esp-idf-svc = { git =  "https://github.com/esp-rs/esp-idf-svc.git"}
```

## config.toml
To get lv-bindings-rust to comple and build I made the following changes to the config.toml file.
```
[build]
target = "xtensa-esp32s3-espidf"

[target.xtensa-esp32s3-espidf]
linker = "ldproxy"
runner = "espflash flash --monitor"
rustflags = [ "--cfg",  "espidf_time64",]

[unstable]
build-std = ["std", "panic_abort"]

[env]
MCU="esp32s3"

# Note: this variable is not used by the pio builder (`cargo build --features pio`)
ESP_IDF_VERSION = "v5.5.1"

# The directory that has the lvgl config files - lv_conf.h, lv_drv_conf.h
DEP_LV_CONFIG_PATH = { relative = true, value = "lvgl-configs" }

# Required to make lvgl build correctly otherwise get wrong file type (ie compiled for a big endian system and target is little endian)
CROSS_COMPILE = "xtensa-esp32s3-elf"

# Required for lvgl otherwise the build would fail with the error -> dangerous relocation: call8: call target out of range
# for some lvgl functions
CFLAGS_xtensa_esp32s3_espidf="-mlongcalls"

# Directory for custom fonts (written in C) that Lvgl can use
LVGL_FONTS_DIR = {relative = true, value = "custom-fonts"}

# Required for lvgl to build otherwise you will get string.h not found.
# Verfiy path and toolchain version being used on your PC (esp-14.2.0_20240906)
TARGET_C_INCLUDE_PATH = "/home/ed/.rustup/toolchains/esp/xtensa-esp-elf/esp-15.2.0_20250920/xtensa-esp-elf/xtensa-esp-elf/include"
```

## lv-binding-rust fork
I updated my fork of lv-binding-rust to include PR153 ie the changes recommended by madwizard-thomas and merged with Master commit d83b374

## Flashing the ESP32S3 device
Source the ESP environment first, then flash the ESP32S3 device.
```
$ . ~/export-esp.sh
$ cargo espflash flash --partition-table=partition-table/partitions.csv --monitor
```

## Observations
Setting  ```#define LV_DISP_DEF_REFR_PERIOD 10  (line 81) in lv_conf.h ``` increased FPS from 66 to 100.

Setting ```lcd_panel_config = RgbPanelConfigBuilder::new().clk_src_ppl240m(true)``` and ```CONFIG_ESP_DEFAULT_CPU_FREQ_MHZ_240=y``` reduce CPU utilization.

Setting ```lcd_panel_config = RgbPanelConfigBuilder::new().bounce_buffer_size_px(3200)``` to a larger value did not seem to make any difference.  But if you see the display shift you may want to adjust this value.  

Also if display shifts then setting ```CONFIG_SPIRAM_XIP_FROM_PSRAM=y CONFIG_LCD_RGB_RESTART_IN_VSYNC=y``` in sdconfig.defaults may help.

Code size was ```App/part. size:    931,184/3,145,728 bytes, 29.60%``` in debug.

Lvgl memory monitor displayed 9.1kB used (20%), 2% frag.

Lvgl cpu utilization displayed 100 FPS, 0% CPU



## Picture of the demo running
The click me
![esp32s3-clickme](photos/clickme.jpg)

The clicked
![esp32s3-clicked](photos/clicked.jpg)



# Versions
### v1.0 :
- initial release

## Change History
Apr 22, 2026
- Adapt LCD timing for 4D Systems 4DLCD-70800480 (pclk 33.3 MHz, typical H/V sync timings, rising-edge PCLK)
- Update README to reference 4D Systems display and document GPIO wiring

Oct 13, 2025 
- Update lcd_panel.rs to use builder for easier user implementation for different LCD displays
- Tested with the following versions
    - Rust: rustc 1.90.0 (1159e78c4 2025-09-14)
    - espup: espup 0.16.0
    - esp-idf: v5.42 or v5.5.1

Feb 04, 2025 
- Use latest lv_binding_rust commit d83b374, use esp-idf-svc v0.51
