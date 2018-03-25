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
        oxdz::format::list().iter().enumerate().for_each(|(i,f)|
            println!("{}:{}", i+1, f.name())
        );
        return;
    }

    if matches.opt_present("P") {
        println!("ID      Player                                   Formats");
        println!("------- ---------------------------------------- -----------------");
        oxdz::player::list().iter().for_each(|p|
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


struct ModPlayer<'a> {
    oxdz: oxdz::Oxdz<'a>,
    fi: oxdz::FrameInfo,
    pause: bool,
    old_pause: bool,
    old_row: usize,
    rx: mpsc::Receiver<command::Key>,
    rate: u32,
    player_id: String,
    load_next: bool,
    name_list: &'a Vec<String>,
    index: usize,
}

impl<'a> ModPlayer<'a> {
    pub fn new(name_list: &'a Vec<String>, rate: u32, player_id: &str, rx: mpsc::Receiver<command::Key>, matches: &Matches) -> Result<Self, Box<Error>> {

        let mut oxdz = load_module(name_list, 0, rate, player_id)?;

        // Mute channels
        match matches.opt_str("M") {
            Some(val) => set_mute(&val, &mut oxdz, true)?,
            None      => {},
        }

        // Solo channels
        match matches.opt_str("S") {
            Some(val) => set_mute(&val, &mut oxdz, false)?,
            None      => {},
        }

        match matches.opt_str("i") {
            Some(val) => { oxdz.set_interpolator(&val)?; },
            None      => {},
        };

        Ok(ModPlayer{
            oxdz,
            fi: oxdz::FrameInfo::new(),
            pause: false,
            old_pause: false,
            old_row: 9999,
            rx,
            rate,
            player_id: player_id.to_owned(),
            load_next: false,
            name_list,
            index: 0,
        })
    }

    pub fn load(&mut self) -> Result<(), Box<Error>> {
        println!();
        self.index += 1;
        self.oxdz = load_module(self.name_list, self.index, self.rate, &self.player_id)?;
        self.load_next = false;
        Ok(())
    }

    pub fn fill_buffer(&mut self, mut buffer: &mut [i16]) {
        {
            self.oxdz.frame_info(&mut self.fi);

            if self.old_row != self.fi.row || self.pause != self.old_pause {
                show_info(&self.fi, self.fi.time / 1000.0, self.oxdz.module(), self.pause);
                self.old_row = self.fi.row;
                self.old_pause = self.pause;
            }

            while self.pause {
                match self.rx.recv() {
                    Ok(cmd) => match cmd {
                        command::Key::Pause => { self.pause = !self.pause },
                        command::Key::Exit  => { println!(); process::exit(0) },
                        _ => (),
                    },
                    Err(_)  => (),
                }
            }

            match self.rx.try_recv() {
                Ok(cmd) => match cmd {
                    command::Key::Pause    => { self.pause = !self.pause },
                    command::Key::Exit     => { println!(); process::exit(0) },
                    command::Key::Forward  => { self.oxdz.set_position(self.fi.pos + 1); },
                    command::Key::Backward => { self.oxdz.set_position(if self.fi.pos > 0 { self.fi.pos - 1 } else { 0 }); },
                },
                Err(_)  => (),
            }

            if self.fi.loop_count > 0 {
                self.load_next = true;
            }
        }

        self.oxdz.fill_buffer(&mut buffer, 0);
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

        thread::spawn(move || {
            let name_list = &matches.free;

            let mut mod_player = match ModPlayer::new(name_list, rate, &player_id, rx, &matches) {
                Ok(val) => val,
                Err(e)  => {
                    eprintln!("Error: {}", e);
                    process::exit(1);
                },
            };

            mod_player.oxdz.set_position(start);

            event_loop.run(move |_, data| {
                match data {
                    cpal::StreamData::Output{buffer: cpal::UnknownTypeOutputBuffer::I16(mut buffer)} => {
                        mod_player.fill_buffer(&mut buffer);
                        if mod_player.load_next {
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

fn load_module<'a>(name_list: &Vec<String>, index: usize, rate: u32, player_id: &str) -> Result<oxdz::Oxdz<'a>, Box<Error>> {
    if index >= name_list.len() {
        process::exit(0);  // no more modules to play
    }
    let name = &name_list[index];

    println!("Loading {}...", name);
    let file = File::open(name)?;
    let mmap = unsafe { Mmap::map(&file).expect("failed to map the file") };

    // Load the module and optionally set the player we want
    let mut oxdz = oxdz::Oxdz::new(&mmap[..], rate, &player_id)?;

    let mut mi = oxdz::ModuleInfo::new();
    oxdz.module_info(&mut mi);
    println!("Format  : {}", mi.description);
    println!("Creator : {}", mi.creator);
    println!("Channels: {}", mi.channels);
    println!("Title   : {}", mi.title);
    println!("Player  : {}", oxdz.player_info()?.name);

    println!("Duration: {}min{:02}s", (mi.total_time + 500) / 60000,
                                     ((mi.total_time + 500) / 1000) % 60);
    Ok(oxdz)
}

fn show_info(fi: &oxdz::FrameInfo, time: f32, module: &oxdz::Module, paused: bool) {
    let t = time as u32;
    print!("pos:{:02X}/{:02X} pat:{:02X}/{:02X} row:{:02X}/{:02X} speed:{:02X} tempo:{:02X}  {}:{:02}:{:02}  {} \r",
           fi.pos, module.len()-1, fi.pattern.unwrap_or(0), module.patterns()-1, fi.row, fi.num_rows, fi.speed,
           fi.tempo, t / (60 * 60), (t / 60) % 60, t % 60, if paused { "[PAUSE]" } else { "       " } );
    let _ = stdout().flush();
}

fn set_mute(list: &str, oxdz: &mut oxdz::Oxdz, val: bool) -> Result<(), Box<Error>> {
    oxdz.set_mute_all(!val);
    for range in list.split(",") {
        if range.contains("-") {
            let num = range.split("-").collect::<Vec<&str>>();
            if num.len() != 2 {
                //return Err(std::error::Error)
            }
            let start = num[0].parse::<usize>()?;
            let end   = num[1].parse::<usize>()?;
            for i in start..end+1 {
                oxdz.set_mute(i, val);
            }
        } else {
            let num = range.parse::<usize>()?;
            oxdz.set_mute(num, val);
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
