extern crate memmap;
extern crate oxdz;
extern crate cpal;
extern crate getopts;

use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{stdout, Write};
use getopts::{Options, Matches};
use memmap::Mmap;
use oxdz::{Oxdz, FrameInfo, format, player};


const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");


fn main() {

    let args: Vec<String> = env::args().collect();
    let mut opts = Options::new();

    opts.optflag("h", "help", "Display a summary of the command line options");
    opts.optflag("L", "list-formats", "List the supported module formats");
    opts.optflag("P", "list-players", "List the available players");
    opts.optopt("p", "player", "Use this player", "id");
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
    let name = &matches.free[0];
    let file = try!(File::open(name));
    let mmap = unsafe { Mmap::map(&file).expect("failed to map the file") };

    println!("■ ■   ■   {}", VERSION.unwrap_or(""));
    println!("    ■  ");

    // Handle option to set start order
    let start = match matches.opt_str("s") {
        Some(val) => val.parse()?,
        None      => 0,
    };

    // Handle option to specify player
    let player_id = match matches.opt_str("p") {
        Some(val) => val,
        None      => "".to_owned(),
    };

    // Load the module and optionally set the player we want
    let mut oxdz = Oxdz::new(&mmap[..], &player_id)?;

    println!("Format : {}", oxdz.module.description);
    println!("Creator: {}", oxdz.module.creator);
    println!("Title  : {}", oxdz.module.title());
    println!("Player : {}", oxdz.player_info()?.name);

    let mut player = oxdz.player()?;
    player.data.pos = start;

    player.start();

    // Set up our audio output
    let endpoint = cpal::default_endpoint().expect("Failed to get default endpoint");
    let format = cpal::Format{
        channels    : vec![cpal::ChannelPosition::FrontLeft, cpal::ChannelPosition::FrontRight],
        samples_rate: cpal::SamplesRate(44100),
        data_type   : cpal::SampleFormat::I16,
    };

    let event_loop = cpal::EventLoop::new();
    let voice_id = event_loop.build_voice(&endpoint, &format).unwrap();
    event_loop.play(voice_id);

    let mut fi = FrameInfo::new();

    event_loop.run(move |_, buffer| {
        match buffer {
            cpal::UnknownTypeBuffer::I16(mut buffer) => {
                player.info(&mut fi).fill_buffer(&mut buffer, 0);
                print!("info pos:{:02X} row:{:02X} speed:{:02X} tempo:{:02X}    \r", fi.pos, fi.row, fi.speed, fi.tempo);
                let _ = stdout().flush();
            }

            _ => { }
        }
    });

    //println!();

    //Ok(())
}
