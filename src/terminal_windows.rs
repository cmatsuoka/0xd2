use std::error::Error;
use winapi;
use kernel32;
use libc;

extern {
    pub fn getchar() -> libc::c_int;
}


pub struct Terminal {
    handle: winapi::HANDLE,
    mode: winapi::DWORD,
}

impl Terminal {
    pub fn new() -> Result<Self, Box<Error>> {
        let handle = unsafe { kernel32::GetStdHandle(winapi::winbase::STD_INPUT_HANDLE) };
        let mut mode: winapi::DWORD = 0;
        unsafe {
            kernel32::GetConsoleMode(handle, &mut mode);
        }
        Ok(Terminal { handle, mode })
    }

    pub fn set(&self) {
        let mode = self.mode & !(winapi::wincon::ENABLE_ECHO_INPUT | winapi::wincon::ENABLE_LINE_INPUT);
        unsafe {
            kernel32::SetConsoleMode(self.handle, mode);
        }
    }

    pub fn reset(&self) {
        unsafe {
            kernel32::SetConsoleMode(self.handle, self.mode);
        }
    }
}

pub fn read_key() -> Option<char> {
    let ch = unsafe{ getchar() };
    Some(ch as u8 as char)
}
