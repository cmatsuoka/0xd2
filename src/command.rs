use terminal;

pub enum Key {
    Pause,
    Exit,
    Forward,
    Backward,
    Next,
    Previous,
}

pub struct Command;

impl Command {
    pub fn new() -> Self {
         Command{
         }
    }

    pub fn process(&mut self, c: char) -> Option<Key> {
        match c {
            ' '    => return Some(Key::Pause),
            'q'    => return Some(Key::Exit),
            'f'    => return Some(Key::Forward),
            'b'    => return Some(Key::Backward),
            'n'    => return Some(Key::Next),
            'p'    => return Some(Key::Previous),

            #[cfg(unix)]
            '\x1b' => {
                match terminal::read_key() {
                    Some(c) => if c == '[' {
                        match terminal::read_key() {
                            Some(c) => match c {
                                'C' => return Some(Key::Forward),
                                'D' => return Some(Key::Backward),
                                'A' => return Some(Key::Next),
                                'B' => return Some(Key::Previous),
                                _   => (),
                            }
                            None    => (),
                        }
                    }
                    None    => return Some(Key::Exit),
                }
            },
            _      => (),
        }

        return None
    }
}
