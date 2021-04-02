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

use regex::Regex;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::WindowCanvas;
use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;
use structopt::clap::AppSettings;
use structopt::StructOpt;

// Command line options
#[derive(StructOpt)]
#[structopt(
    about = "A simple slider bar. Left click to select a value in the (inclusive) range from <start> to <end>. Continuously execute a command with --command. Return to print the current value and exit regardless of options. ESC to cancel & return nothing.",
    setting = AppSettings::AllowNegativeNumbers)]
struct Options {
    #[structopt(default_value = "0")]
    start: f32,

    #[structopt(default_value = "100")]
    end: f32,

    #[structopt(short, long, default_value = "", help = "A string representing a shell command that will be run when the dbar value changes. Occurrences of `%v` in <command> will be replaced with dbar's current value")]
    command: String,

    #[structopt(short = "C", long, default_value = "", help = "Like --command but only execute <command> when a click occurs & do not exit the bar until ESC is hit")]
    command_on_click: String,

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

    #[structopt(short = "v", long, help = "Display the current bar value in the window title")]
    show_value: bool,

    #[structopt(short = "r", long, default_value = "15", help = "Milliseconds in between bar redraws - lower is smoother but more compute intensive")]
    refresh_rate: u64,
}

pub fn main() -> Result<(), String> {
    let opt = Options::from_args(); // Parse command line options
    // Sanitize inputs
    assert!(opt.start < opt.end,
            "<start> = {} must be smaller than <end> = {}", opt.start, opt.end);
    assert!(opt.width > 1 || opt.height > 1,
            "<width> = {} and <height> = {} must be greater than 0", opt.width, opt.height);
    let bg_col = string_to_color(&opt.bg_col[..]);
    let fg_col = string_to_color(&opt.fg_col[..]);

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

    // Lazily evaluate the bar value for potential reuse
    let mut dbar_value = LazyResult::new(|x: i32| {
        let range = (opt.end - opt.start).abs();
        let result = opt.start + range * (x as f32 / opt.width as f32);
        if opt.floating { result }
        else { result.round() }
    });


    let have_command = !opt.command.is_empty();
    let on_windows = cfg!(target_os = "windows");
    let mut last_val: Option<f32> = None;
    let mut fill_pixels = 0;
    let mut first_draw = true;

    // Main execution loop
    'running: loop {
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
                } => {
                    if !opt.command_on_click.is_empty() {
                        run_command(&opt.command_on_click, dbar_value.value(fill_pixels), on_windows);
                    } else {
                        println!("{}", dbar_value.value(fill_pixels));
                        break 'running
                    }
                },
                Event::KeyDown {
                    keycode: Some(Keycode::Return),
                    ..
                } => {
                    println!("{}", dbar_value.value(fill_pixels));
                    break 'running
                }
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

            // Render the bar
            canvas.set_draw_color(bg_col);
            canvas.clear();
            canvas.set_draw_color(fg_col);
            canvas.fill_rect(Rect::new(0, 0, (fill_pixels) as u32, opt.height))
                  .expect("failed to draw rectangle");
            canvas.present();

            // Only compute last value in the cases it's used (-c || -v).
            let value_changed = if have_command || opt.show_value {
                // Check if the current value is different from last value
                let changed = if let Some(v) = last_val {
                    v != dbar_value.value(fill_pixels)
                } else { true };

                // Update last value for next time
                if changed { last_val = Some(dbar_value.value(fill_pixels)); }
                changed

            } else { false }; // value_changed is unused ∴ return arbitrary bool

            // Write value to window title if requested & the value has changed
            if opt.show_value && value_changed {
                let title_update = opt.title.clone() + " - "
                    + &dbar_value.value(fill_pixels).to_string();
                canvas.window_mut().set_title(&title_update).unwrap();
            }

            // Execute user command if provided & the value has changed
            if have_command && value_changed {
                run_command(&opt.command, dbar_value.value(fill_pixels), on_windows);
            }
        }

        std::thread::sleep(Duration::from_millis(opt.refresh_rate));
    }

    Ok(())
}

// Struct for memoizing the bar result
struct LazyResult<T>
where
    T: Fn(i32) -> f32
{
    calculation: T,
    value: HashMap<i32, f32>,
}

impl<T> LazyResult<T>
where
    T: Fn(i32) -> f32
{
    fn new(calculation: T) -> LazyResult<T> {
        LazyResult {
            calculation,
            value: HashMap::new(),
        }
    }

    // Only run the calculation if we haven't set the value before
    fn value(&mut self, x: i32) -> f32 {
        match self.value.get(&x) {
            None => {
                let novel_value = (self.calculation)(x);
                self.value.insert(x, novel_value);
                novel_value
            }
            Some(v) => *v
        }
    }
}

fn string_to_color(hex_code: &str) -> Color {
    // Check whether the string is a valid hex colour code
    let re = Regex::new(r"^#[a-f,A-F,0-9]{6}").unwrap();
    if ! re.is_match(hex_code) {
        panic!("Invalid hex colour code: {}", hex_code);
    }

    let r: u8 = u8::from_str_radix(&hex_code[1..3], 16).unwrap();
    let g: u8 = u8::from_str_radix(&hex_code[3..5], 16).unwrap();
    let b: u8 = u8::from_str_radix(&hex_code[5..7], 16).unwrap();

    Color::RGB(r, g, b)
}

fn run_command(command: &String, value: f32, on_windows: bool) {
    let current_cmd =
        &command.replace("%v", &value.to_string());
    if on_windows {
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
