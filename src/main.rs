extern crate memmap;
extern crate oxdz;
extern crate cpal;
extern crate getopts;
extern crate termios;
extern crate libc;

use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{stdout, Write};
use std::process;
use std::thread;
use std::sync::mpsc;
use getopts::{Options, Matches};
use memmap::Mmap;
use oxdz::{Oxdz, Module, FrameInfo, format, player};

mod terminal;
mod command;

const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");


// https://stackoverflow.com/questions/29963449/golang-like-defer-in-rust
struct ScopeCall<F: FnOnce()> {
    c: Option<F>
}
impl<F: FnOnce()> Drop for ScopeCall<F> {
    fn drop(&mut self) {
        self.c.take().unwrap()()
    }
}

macro_rules! expr { ($e: expr) => { $e } } // tt hack
macro_rules! defer {
    ($($data: tt)*) => (
        let _scope_call = ScopeCall {
            c: Some(|| -> () { expr!({ $($data)* }) })
        };
    )
}

#[macro_export]
macro_rules! try_ {
    ( $a: expr ) => {
        match $a {
            Ok(val) => val,
            Err(e)  => {
                eprintln!("Error: {}", e);
                return false;
            }
        }
    }
}


fn main() {

    let args: Vec<String> = env::args().collect();
    let mut opts = Options::new();

    opts.optflag("h", "help", "Display a summary of the command line options");
    opts.optopt("i", "interpolator", "Select interpolation type", "{nearest|linear|spline}");
    opts.optflag("L", "list-formats", "List supported module formats");
    opts.optopt("M", "mute", "Mute channels or ranges of channels", "list");
    opts.optflag("P", "list-players", "List available players");
    opts.optopt("p", "player", "Use this player", "id");
    opts.optopt("r", "rate", "Set the sampling rate in hertz", "freq");
    opts.optopt("S", "solo", "Solo channels or ranges of channels", "list");
    opts.optopt("s", "start", "Start from the specified order", "num");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    if matches.opt_present("L") {
        format::list().iter().enumerate().for_each(|(i,f)|
            println!("{}:{}", i+1, f.name())
        );
        return;
    }

    if matches.opt_present("P") {
        println!("ID      Player                                   Formats");
        println!("------- ---------------------------------------- -----------------");
        player::list().iter().for_each(|p|
            println!("{:7} {:40} {}", p.id, p.name, p.accepts.join(", "))
        );
        return;
    }

    if matches.opt_present("h") ||  matches.free.len() < 1 {
        let brief = format!("Usage: {} [options] filename", args[0]);
        print!("{}", opts.usage(&brief));
        return;
    }

    match run(&matches) {
        Ok(_)  => {},
        Err(e) => println!("Error: {}", e),
    }
}

fn run(matches: &Matches) -> Result<(), Box<Error>> {

    println!(r#"  ___          _ ____  "#); 
    println!(r#" / _ \__  ____| |___ \ "#);
    println!(r#"| | | \ \/ / _` | __) |"#);
    println!(r#"| |_| |>  < (_| |/ __/ "#);
    println!(r#" \___//_/\_\__,_|_____|  {}"#, VERSION.unwrap_or(""));


    // Set up our audio output
    let device = cpal::default_output_device().expect("Failed to get default output device");

    // Choose sampling rate from parameter, or get system default
    let rate = match matches.opt_str("r") {
        Some(val) => val.parse()?,
        None      => {
            if let Ok(fmt) = device.default_output_format() {
                let cpal::SampleRate(sample_rate) = fmt.sample_rate;
                sample_rate
            } else {
                44100
            }
        }
    };

    println!("Sampling rate : {}Hz", rate);

    // Handle option to set start order
    let start = match matches.opt_str("s") {
        Some(val) => parse_num(&val)?,
        None      => 0,
    };

    // Handle option to specify player
    let player_id = match matches.opt_str("p") {
        Some(val) => val,
        None      => "".to_owned(),
    };

    // Create event loop
    let format = cpal::Format{
        channels   : 2,
        sample_rate: cpal::SampleRate(rate),
        data_type  : cpal::SampleFormat::I16,
    };
    let event_loop = cpal::EventLoop::new();
    let stream_id = event_loop.build_output_stream(&device, &format)?;
    event_loop.play_stream(stream_id);

    let (tx, rx) = mpsc::channel();

    {
        let matches = matches.clone();
        let mut old_row = 9999;

        thread::spawn(move || {
            let name = &matches.free[0];
            let file = try_!(File::open(name));
            let mmap = unsafe { Mmap::map(&file).expect("failed to map the file") };

            // Load the module and optionally set the player we want
            let oxdz = try_!(Oxdz::new(&mmap[..], rate, &player_id));
        
            println!("Format  : {}", oxdz.module.description);
            println!("Creator : {}", oxdz.module.creator);
            println!("Channels: {}", oxdz.module.channels);
            println!("Title   : {}", oxdz.module.title());
            println!("Player  : {}", try_!(oxdz.player_info()).name);
        
            let mut player = try_!(oxdz.player());
            player.data.pos = start;
        
            // Must be after module scan (called in oxdz::player())
            println!("Duration: {}min{:02}s", (player.total_time + 500) / 60000,
                                             ((player.total_time + 500) / 1000) % 60);
        
            // Mute channels
            match matches.opt_str("M") {
                Some(val) => try_!(set_mute(&val, &mut player, true)),
                None      => {},
            }
        
            // Solo channels
            match matches.opt_str("S") {
                Some(val) => try_!(set_mute(&val, &mut player, false)),
                None      => {},
            }
        
            player.start();
        
            // Select interpolator (must be after player start)
            match matches.opt_str("i") {
                Some(val) => try_!(player.set_interpolator(&val)),
                None      => {},
            }
        
            let mut pause = false;
            let mut old_pause = false;
            let mut fi = FrameInfo::new();

            event_loop.run(move |_, data| {
                match data {
                    cpal::StreamData::Output{buffer: cpal::UnknownTypeOutputBuffer::I16(mut buffer)} => {
                        {
                            player.info(&mut fi);

                            if old_row != fi.row || pause != old_pause {
                                show_info(&fi, fi.time / 1000.0, player.module(), pause);
                                old_row = fi.row;
                                old_pause = pause;
                            }

                            while pause {
                                match rx.recv() {
                                    Ok(cmd) => match cmd {
                                        command::Key::Pause => { pause = !pause },
                                        command::Key::Exit  => { println!(); process::exit(0) },
                                        _ => (),
                                    },
                                    Err(_)  => (),
                                }
                            }

                            match rx.try_recv() {
                                Ok(cmd) => match cmd {
                                    command::Key::Pause    => { pause = !pause },
                                    command::Key::Exit     => { println!(); process::exit(0) },
                                    command::Key::Forward  => { player.set_position(fi.pos + 1); },
                                    command::Key::Backward => { player.set_position(if fi.pos > 0 { fi.pos - 1 } else { 0 }); },
                                },
                                Err(_)  => (),
                            }

                            if fi.loop_count > 0 {
                                println!();
                                process::exit(0);
                            }
                        }

                        player.fill_buffer(&mut buffer, 0);
                    }
        
                    _ => { }
                }
            });
        });
    }

    let tty = terminal::Terminal::new()?;
    tty.set();
    defer!{ tty.reset() }

    let mut cmd = command::Command::new();

    loop {
        {
            let cmd = match terminal::read_key() {
                Some(c) => cmd.process(c),
                None    => None,
            };

            match cmd {
                Some(c) => tx.send(c).unwrap(),
                None    => (),
            }

        };
    }

    //Ok(())
}

pub fn show_info(fi: &FrameInfo, time: f32, module: &Module, paused: bool) {
    let t = time as u32;
    print!("pos:{:02X}/{:02X} pat:{:02X}/{:02X} row:{:02X}/{:02X} speed:{:02X} tempo:{:02X}  {}:{:02}:{:02}  {} \r",
           fi.pos, module.len()-1, fi.pattern.unwrap_or(0), module.patterns()-1, fi.row, fi.num_rows, fi.speed,
           fi.tempo, t / (60 * 60), (t / 60) % 60, t % 60, if paused { "[PAUSE]" } else { "       " } );
    let _ = stdout().flush();
}

fn set_mute(list: &str, player: &mut player::Player, val: bool) -> Result<(), Box<Error>> {
    player.set_mute_all(!val);
    for range in list.split(",") {
        if range.contains("-") {
            let num = range.split("-").collect::<Vec<&str>>();
            if num.len() != 2 {
                //return Err(std::error::Error)
            }
            let start = num[0].parse::<usize>()?;
            let end   = num[1].parse::<usize>()?;
            for i in start..end+1 {
                player.set_mute(i, val);
            }
        } else {
            let num = range.parse::<usize>()?;
            player.set_mute(num, val);
        }
    }
    Ok(())
}

fn parse_num(s: &str) -> Result<usize, std::num::ParseIntError> {
    if s.starts_with("0x") {
        usize::from_str_radix(&s[2..], 16)
    } else {
        s.parse()
    }
}
