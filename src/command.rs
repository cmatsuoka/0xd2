use std::process;
use std::thread;
use std::time;
use oxdz::Module;
use oxdz::FrameInfo;
use terminal;


pub enum Key {
    Forward,
}

pub struct Command {
    pause: bool,

}

impl Command {
    pub fn new() -> Self {
         Command{
             pause: false,
         }
    }

    pub fn process(&mut self, c: char, fi: &FrameInfo, time: f32, module: &Module) -> Option<Key> {
        match c {
            ' '    => { self.pause = !self.pause; ::show_info(fi, time, module, self.pause) },
            'q'    => { println!(); process::exit(0) },
            '\x1b' => {
                match terminal::read_key() {
                    Some(c) => if c == '[' {
                        match terminal::read_key() {
                            Some(c) => match c {
                                'C' => return Some(Key::Forward),
                                _   => (),
                            }
                            None    => (),
                        }
                    }
                    None    => { println!(); process::exit(0) },
                }
            },
            _      => (),
        }

        self.check_pause(fi, time, module);

        return None
    }

    pub fn check_pause(&mut self, fi: &FrameInfo, time: f32, module: &Module) {
        if self.pause {
            while self.pause {
                thread::sleep(time::Duration::from_millis(100));
                match terminal::read_key() {
                    Some(c) => self.process(c, fi, time, module),
                    None    => None,
                };
            }
        }
    }
}
