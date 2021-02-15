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
#[structopt(about = "Use left click to select a value in the given (inclusive) range [<start>, <end>]. ESC to cancel.")]
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

    #[structopt(long, default_value = "#aaaaff", help = "The foreground (bar) colour in #rrggbb hex format")]
    fg_col: String,

    #[structopt(long, help = "Do not capture/grab the mouse cursor")]
    no_mouse_capture: bool,
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
        .window("dbar", opt.width, opt.height)
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;

    let mut canvas : WindowCanvas = window.into_canvas()
        .present_vsync()
        .build().unwrap();

    // Conditionally grab window/capture mouse
    if ! opt.no_mouse_capture {
        sdl_context.mouse().set_relative_mouse_mode(true);
    }

    let mut first_draw = true;
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
                    x,
                    ..
                } => { println!("{}", calc_result(opt.floating, opt.width, opt.start, opt.end, x)); break 'running },
                _ => {},
            }
        }


        // If the mouse moved or this is the first iteration
        if (events.relative_mouse_state().x() != 0) | first_draw {
            first_draw = false;

            // Render the bar...
            canvas.set_draw_color(string_to_color(&opt.bg_col[..]).unwrap());
            canvas.clear();
            canvas.set_draw_color(string_to_color(&opt.fg_col[..]).unwrap());
            canvas.fill_rect(Rect::new(0, 0, (events.mouse_state().x() + 1) as u32, opt.height)).expect("failed to draw rectangle");
            canvas.present();

            // ...and execute the user command if it was provided
            if opt.command.len() > 0 {
                let current_cmd =
                    &opt.command.replace("%v", &calc_result(opt.floating, opt.width, opt.start, opt.end, events.mouse_state().x()).to_string());
                Command::new("sh")
                    .arg("-c")
                    .arg(current_cmd)
                    .spawn()
                    .expect("failed to run user command");
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}

fn calc_result(floating: bool, width: u32, start: f32, end: f32, x: i32) -> f32 {
    let range = (end - start).abs();
    let result = start + range * (x as f32 / (width-1) as f32); // need width-1 because we count the 0th pixel
    if floating { result }
    else { result.round() }
}

// Parses a color hex code of the form '#rRgGbB..' into sdl2::pixels::Color
fn string_to_color(hex_code: &str) -> Result<Color, std::num::ParseIntError> {
    let r: u8 = u8::from_str_radix(&hex_code[1..3], 16)?;
    let g: u8 = u8::from_str_radix(&hex_code[3..5], 16)?;
    let b: u8 = u8::from_str_radix(&hex_code[5..7], 16)?;

    Ok(Color::RGB(r, g, b))
}
