extern crate memmap;
extern crate oxdz;
extern crate cpal;
extern crate getopts;
extern crate termios;
extern crate rand;
extern crate libc;

use std::env;
use std::error::Error;
use std::process;
use std::thread;
use std::sync::mpsc;

mod terminal;
mod command;
mod modplayer;

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


fn main() {

    let args: Vec<String> = env::args().collect();
    let mut opts = getopts::Options::new();

    opts.optflag("h", "help", "Display a summary of the command line options");
    opts.optopt("i", "interpolator", "Select interpolation type", "{nearest|linear|spline}");
    opts.optflag("L", "list-formats", "List supported module formats");
    opts.optopt("M", "mute", "Mute channels or ranges of channels", "list");
    opts.optflag("P", "list-players", "List available players");
    opts.optopt("p", "player", "Use this player", "id");
    opts.optflag("R", "random", "Randomize list of files before playing");
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
        oxdz::format_list().iter().enumerate().for_each(|(i,f)|
            println!("{}:{}", i+1, f.name())
        );
        return;
    }

    if matches.opt_present("P") {
        println!("ID      Player                                   Formats");
        println!("------- ---------------------------------------- -----------------");
        oxdz::player_list().iter().for_each(|p|
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

fn run(matches: &getopts::Matches) -> Result<(), Box<Error>> {

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

        #[cfg(debug_assertions)]
        let stack_size = 8_000_000;

        #[cfg(not(debug_assertions))]
        let stack_size = 4_000_000;

        thread::Builder::new().stack_size(stack_size).spawn(move || {
            let name_list = &mut matches.free.to_vec()[..];

            let mut mod_player = match modplayer::ModPlayer::new(name_list, rate, &player_id, rx, &matches) {
                Ok(val) => val,
                Err(e)  => {
                    eprintln!("Error: {}", e);
                    process::exit(1);
                },
            };

            mod_player.set_position(start);

            event_loop.run(move |_, data| {
                match data {
                    cpal::StreamData::Output{buffer: cpal::UnknownTypeOutputBuffer::I16(mut buffer)} => {
                        mod_player.fill_buffer(&mut buffer);
                        while mod_player.load_next {
                            println!();
                            match mod_player.load() {
                                Ok(_)  => {},
                                Err(e) => print!("error: {}", e),
                            }
                        }
                    }

                    _ => { }
                }
            });
        }).unwrap();
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

fn parse_num(s: &str) -> Result<usize, std::num::ParseIntError> {
    if s.starts_with("0x") {
        usize::from_str_radix(&s[2..], 16)
    } else {
        s.parse()
    }
}
