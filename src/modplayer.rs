use std::error::Error;
use std::fs::File;
use std::io::{stdout, Write};
use std::process;
use std::sync::mpsc;
use memmap::Mmap;
use getopts;
use oxdz;
use command;

pub struct ModPlayer<'a> {
    oxdz: oxdz::Oxdz<'a>,
    fi: oxdz::FrameInfo,
    pause: bool,
    old_pause: bool,
    old_row: usize,
    rx: mpsc::Receiver<command::Key>,
    rate: u32,
    player_id: String,
    pub load_next: bool,
    name_list: &'a Vec<String>,
    index: usize,
}

impl<'a> ModPlayer<'a> {
    pub fn new(name_list: &'a Vec<String>, rate: u32, player_id: &str, rx: mpsc::Receiver<command::Key>, matches: &getopts::Matches) -> Result<Self, Box<Error>> {

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

    pub fn set_position(&mut self, pos: usize) {
        self.oxdz.set_position(pos);
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

