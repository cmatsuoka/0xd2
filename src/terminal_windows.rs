use std::error::Error;
use winapi;
use kernel32;

pub struct Terminal {
    handle: winapi::HANDLE,
    mode: winapi::DWORD,
}

impl Terminal {
    pub fn new() -> Result<Self, Box<Error>> {
        let handle = unsafe { kernel32::GetStdHandle(winapi::winbase::STD_INPUT_HANDLE) };
        let mut mode: winapi::DWORD = 0;
        unsafe{ kernel32::GetConsoleMode(handle, &mut mode); }
        Ok(Terminal{
            handle,
            mode,
        })
    }

    pub fn set(&self) {
        let mode = self.mode & !(winapi::wincon::ENABLE_ECHO_INPUT | winapi::wincon::ENABLE_LINE_INPUT);
        unsafe { kernel32::SetConsoleMode(self.handle, mode); }
    }
    
    pub fn reset(&self) {
        unsafe { kernel32::SetConsoleMode(self.handle, self.mode); }
    }

    pub fn read_key(&self) -> Option<char> {
        let mut num: winapi::DWORD = 0;
        let input: winapi::PINPUT_RECORD = 0 as winapi::PINPUT_RECORD;
        unsafe {
            kernel32::ReadConsoleInputA(self.handle, input, 1, &mut num);
	    if (*input).EventType & winapi::KEY_EVENT != 0 {
	    	let key = (*input).KeyEvent();
		if key.bKeyDown != 0 {
	    	    println!("key={}", *key.AsciiChar() as u8 as char);
	    	    return Some(*key.AsciiChar() as u8 as char);
		}
	    }
        }
	None
    }
}
