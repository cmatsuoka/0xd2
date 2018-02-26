use std::error::Error;
use termios::*;
use libc;

pub struct Terminal {
    term: Termios,
}

impl Terminal {
    pub fn new() -> Result<Self, Box<Error>> {
        Ok(Terminal{
            term: Termios::from_fd(0)?,
        })
    }

    pub fn set(&self) {
        #[cfg(unix)] {
            if !isatty(0) {
                return
            }
    
            let mut t = self.term.clone();
            t.c_lflag &= !(ECHO | ICANON | TOSTOP);
            t.c_cc[VMIN] = 0;
            t.c_cc[VTIME] = 0;
            match tcsetattr(0, TCSAFLUSH, &t) {
                Ok(_)  => (),
                Err(e) => eprintln!("can't set terminal: {}", e),
            }
        }
    }
    
    pub fn reset(&self) {
        #[cfg(unix)] {
            if !isatty(0) {
                return
            }

            match tcsetattr(0, TCSAFLUSH, &self.term) {
                Ok(_)  => (),
                Err(e) => eprintln!("can't reset terminal: {}", e),
            }
        }
    }
}

#[cfg(unix)]
pub fn isatty(fd: libc::c_int) -> bool {
    unsafe { return libc::isatty(fd) != 0 }
}
