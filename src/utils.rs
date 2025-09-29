use std::{
    io,
    ops::Range,
    process::{Command, Stdio},
};

pub struct Ranges<I: Iterator<Item = usize>> {
    v: I,
    current: Option<(usize, usize)>,
}

impl<I: Iterator<Item = usize>> Iterator for Ranges<I> {
    type Item = Range<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match (self.v.next(), self.current) {
                (None, None) => break None,
                (None, Some((start, end))) => {
                    self.current = None;
                    break Some(start..end);
                }
                (Some(i), None) => self.current = Some((i, i + 1)),
                (Some(i), Some((start, end))) => {
                    if end == i {
                        self.current = Some((start, i + 1));
                    } else {
                        self.current = Some((i, i + 1));
                        break Some(start..end);
                    }
                }
            }
        }
    }
}

pub trait IteratorExt: Iterator<Item = usize> {
    fn ranges(self) -> Ranges<Self>
    where
        Self: Sized,
    {
        Ranges {
            v: self,
            current: None,
        }
    }
}

impl<I: Iterator<Item = usize>> IteratorExt for I {}

pub trait CommandExt {
    fn spawn_detached(&mut self) -> io::Result<()>;
}

impl CommandExt for Command {
    fn spawn_detached(&mut self) -> io::Result<()> {
        // This is pretty much lifted from the implementation in Alacritty:
        // https://github.com/alacritty/alacritty/blob/b9c886872d1202fc9302f68a0bedbb17daa35335/alacritty/src/daemon.rs

        self.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(unix)]
        unsafe {
            use std::os::unix::process::CommandExt as _;

            self.pre_exec(move || {
                match libc::fork() {
                    -1 => return Err(io::Error::last_os_error()),
                    0 => (),
                    _ => libc::_exit(0),
                }

                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }

                Ok(())
            });
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            self.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
        }

        self.spawn().map(|_| ())
    }
}
