#![feature(libc)]
extern crate libc;

extern crate nix;

extern crate rotten_sh;

use std::io::prelude::*;
use std::io;

use nix::unistd::{getpid, getpgid, tcgetpgrp, tcsetpgrp, setpgid};

use rotten_sh::pipeline::Pipeline;
use rotten_sh::signal::Signal;


struct Shell {}

impl Shell {

    fn init() {
        Signal::ignore();
        setpgid(0, 0);
        tcsetpgrp(libc::STDIN_FILENO, getpid()); // FIXME: does not work when launched via cargo run
        println!("My PID is {:?}", getpid());
        println!("My PGID is {:?}", getpgid(None));
        println!("Foreground process is {:?}", tcgetpgrp(libc::STDIN_FILENO));
    }

    fn run() {
        println!("Rotten sh...");

        loop {
            print!("$ ");
            std::io::stdout().flush();

            let mut buffer = String::new();
            io::stdin().read_line(&mut buffer);

            if buffer.trim() == "exit" {
                break;
            }

            if let Some(mut pipeline) = Pipeline::parse(&buffer) { // FIXME: Pipeline.run()?
                pipeline.spawn();
                if pipeline.background {
                    pipeline.bg();
                }
                else {
                    pipeline.fg();
                }
            }
        }
    }

}


fn main() {
    Shell::init();
    Shell::run();
}
