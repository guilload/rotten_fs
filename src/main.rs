#![feature(libc)]
extern crate libc;

extern crate nix;

extern crate rotten_sh;

use std::io::prelude::*;
use std::io;

use nix::unistd::{getpid, setpgid, tcsetpgrp};

use rotten_sh::pipeline::Pipeline;
use rotten_sh::signal::Signal;


struct Shell {
    background_jobs: Vec<Pipeline>,
    suspended_jobs: Vec<Pipeline>,
}

impl Shell {

    fn new() -> Self {
        Signal::ignore();

        setpgid(0, 0).unwrap();
        tcsetpgrp(libc::STDIN_FILENO, getpid()).unwrap();

        Shell { background_jobs: vec![], suspended_jobs: vec![] }
    }

    fn run(&mut self) {
        println!("Rotten sh...");

        loop {
            print!("$ ");
            std::io::stdout().flush().unwrap();

            let mut buffer = String::new();
            io::stdin().read_line(&mut buffer).unwrap();

            if buffer.trim() == "fg" {

                if self.background_jobs.is_empty() {
                    println!("fg: no current job");
                    continue
                }

                self.background_jobs.pop().unwrap().fg().unwrap();
                continue
            }

            if buffer.trim() == "exit" {
                break;
            }

            if let Some(mut job) = Pipeline::parse(&buffer) {
                job.spawn().unwrap();

                if job.background {
                    job.bg();
                    self.background_jobs.push(job);
                }
                else {
                    job.fg().unwrap();
                    if job.is_suspended() {
                        self.suspended_jobs.push(job);
                    }
                }
            }
        }
    }

}


fn main() {
    Shell::new().run();
}
