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
            t.c_cc[VMIN] = 1;  // blocking read
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

pub fn read_key() -> Option<char> {
    if !cfg!(unix) {
        return None
    }

    let mut b: [char; 1] = ['\0'];
    let ret = unsafe { libc::read(0, b.as_mut_ptr() as *mut libc::c_void, 1) };
    if ret == 1 {
        Some(b[0])
    } else {
        None
    }
}
