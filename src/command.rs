use std::process;
use std::thread;
use std::time;
use oxdz::Module;
use oxdz::FrameInfo;
use terminal;

pub struct Command {
     pause: bool,

}

impl Command {
    pub fn new() -> Self {
         Command{
             pause: false,
         }
    }

    pub fn process(&mut self, c: char, fi: &FrameInfo, module: &Module) {
        match c {
            ' '    => { self.pause = !self.pause; ::show_info(fi, module, self.pause) },
            'q'    => { println!(); process::exit(0) },
            '\x1b' => {
                match terminal::read_key() {
                    Some(_) => (), // handle arrows, etc
                    None    => { println!(); process::exit(0) },
                }
            },
            _      => (),
        }

        self.check_pause(fi, module);
    }

    pub fn check_pause(&mut self, fi: &FrameInfo, module: &Module) {
        if self.pause {
            while self.pause {
                thread::sleep(time::Duration::from_millis(100));
                match terminal::read_key() {
                    Some(c) => self.process(c, fi, module),
                    None    => (),
                }
            }
        }
    }
}
