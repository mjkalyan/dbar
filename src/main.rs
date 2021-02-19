// Copyright © 2021 M. James Kalyan

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

extern crate sdl2;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::WindowCanvas;
use std::process::Command;
use std::time::Duration;
use structopt::StructOpt;

// Command line options
#[derive(StructOpt)]
#[structopt(about = "Just a slider bar. Left click to select a value in the given (inclusive) range from <start> to <end>. Continuously execute a command with --command. ESC to cancel.")]
struct Options {
    // TODO handle -negative input
    #[structopt(default_value = "0")]
    start: f32,

    #[structopt(default_value = "100")]
    end: f32,

    #[structopt(short, long, default_value = "", help = "A string representing a shell command that will be run when the dbar value changes. Occurrences of `%v` in <command> will be replaced with dbar's current value.")]
    command: String,

    #[structopt(short, long, help = "Do not round the result to the nearest integer")]
    floating: bool,

    // TODO maybe make default size based on screen dpi and make these flags percent of display rather than pixels
    #[structopt(short = "x", long, default_value = "600", help = "Width of the window")]
    width: u32,

    #[structopt(short = "y", long, default_value = "50", help = "Height of the window")]
    height: u32,

    #[structopt(long, default_value = "#222244", help = "The background colour in #rrggbb hex format")]
    bg_col: String,

    #[structopt(long, default_value = "#9c99c3", help = "The bar colour in #rrggbb hex format")]
    fg_col: String,

    #[structopt(long, help = "Do not capture/grab the mouse cursor")]
    no_mouse_capture: bool,

    #[structopt(short, long, default_value = "0.5", help = "The initial percentage of the bar filled as a float ∈ [0.0, 1.0]")]
    initial_percent: f32,

    #[structopt(short, long, default_value = "dbar", help = "The window title")]
    title: String,
}

pub fn main() -> Result<(), String> {
    let opt = Options::from_args(); // Parse command line options
    // TODO sanitize input:
    // - start must be smaller than end
    // - width and height must be greater than 0
    // - colours must be valid hex codes

    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;
    let mut events = sdl_context.event_pump()?;
    let window = video_subsystem
        .window(&opt.title, opt.width, opt.height)
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;

    let mut canvas : WindowCanvas = window.into_canvas()
        .present_vsync()
        .build().unwrap();

    // Conditionally grab window/capture mouse
    if !opt.no_mouse_capture {
        sdl_context.mouse().set_relative_mouse_mode(true);
    }

    let mut fill_pixels = 0;
    let mut first_draw = true;
    'running: loop {

        // Lazily evaluate the bar value for potential reuse
        let mut dbar_value = LazyResult::new(|floating: bool, width: u32, start: f32, end: f32, x: i32| {
            let range = (end - start).abs();
            let result = start + range * (x as f32 / (width-1) as f32); // width-1 to account for 0th pixel
            if floating { result }
            else { result.round() }
        });

        for event in events.poll_iter() {
            match event {
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                }
                | Event::Quit { .. } => break 'running,
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    ..
                } => { println!("{}", dbar_value.value(opt.floating, opt.width, opt.start, opt.end, fill_pixels)); break 'running },
                _ => {},
            }
        }

        // If the mouse moved or this is the first iteration
        let mouse_movement = events.relative_mouse_state().x();
        if (mouse_movement != 0) | first_draw {

            if first_draw { // On 1st iteration, compute bar fill from initial %
                first_draw = false;
                fill_pixels = (opt.initial_percent * opt.width as f32) as i32;
            } else if opt.no_mouse_capture {
                fill_pixels = events.mouse_state().x();
            } else {        // otherwise compute using mouse movement
                fill_pixels += mouse_movement;
                fill_pixels = if fill_pixels > opt.width as i32 { opt.width as i32 }
                              else if fill_pixels < 0 { 0 }
                              else { fill_pixels }
            }

            // Render the bar...
            canvas.set_draw_color(string_to_color(&opt.bg_col[..]).unwrap());
            canvas.clear();
            canvas.set_draw_color(string_to_color(&opt.fg_col[..]).unwrap());
            canvas.fill_rect(Rect::new(0, 0, (fill_pixels) as u32, opt.height)).expect("failed to draw rectangle");
            canvas.present();

            // ...and execute the user command if it was provided
            if !opt.command.is_empty() {
                let current_cmd =
                    &opt.command.replace("%v", &dbar_value.value(opt.floating, opt.width, opt.start, opt.end, fill_pixels).to_string());
                if cfg!(target_os = "windows") {
                    Command::new("cmd")
                            .args(&["/C", current_cmd])
                            .spawn()
                            .expect("failed to run user command");
                } else {
                    Command::new("sh")
                            .arg("-c")
                            .arg(current_cmd)
                            .spawn()
                            .expect("failed to run user command");
                }
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}

// Struct for memoizing the bar result
struct LazyResult<T>
where
    T: Fn(bool, u32, f32, f32, i32) -> f32
{
    calculation: T,
    value: Option<f32>,
}

impl<T> LazyResult<T>
where
    T: Fn(bool, u32, f32, f32, i32) -> f32
{
    fn new(calculation: T) -> LazyResult<T> {
        LazyResult {
            calculation,
            value: None,
        }
    }

    // Only run the calculation if we haven't set the value before
    fn value(&mut self, floating: bool, width: u32, start: f32, end: f32, x: i32) -> f32 {
        match self.value {
            Some(v) => v,
            None => {
                let v = (self.calculation)(floating, width, start, end, x);
                self.value = Some(v);
                v
            }
        }
    }
}

// Parses a color hex code of the form '#rRgGbB..' into sdl2::pixels::Color
fn string_to_color(hex_code: &str) -> Result<Color, std::num::ParseIntError> {
    let r: u8 = u8::from_str_radix(&hex_code[1..3], 16)?;
    let g: u8 = u8::from_str_radix(&hex_code[3..5], 16)?;
    let b: u8 = u8::from_str_radix(&hex_code[5..7], 16)?;

    Ok(Color::RGB(r, g, b))
}
