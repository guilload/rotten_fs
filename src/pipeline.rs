extern crate libc;

extern crate nix;
extern crate nom;

use std::io;
use std::os::unix::io::RawFd;

use self::nix::sys::wait::{waitpid, WaitStatus, WUNTRACED};
use self::nix::unistd::{close, getpid, pipe, tcsetpgrp};
use self::nom::*;

use command::{Command, Status};
use command::parse_command;
use stdx::StdX;


fn close_fd_opt(opt: Option<RawFd>) -> Result<(), nix::Error> {
    if let Some(fd) = opt {
        close(fd)?
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
pub struct Pipeline {
    commands: Vec<Command>,
    pub background: bool,
    jobid: u32,
    pgid: libc::pid_t,
}

impl Pipeline {

    pub fn pgid(&mut self, p: libc::pid_t) -> &mut Self {
        self.pgid = p;
        self
    }

    pub fn parse(pipeline: &str) -> Option<Self> {
        match parse_pipeline(pipeline.trim()) {
            IResult::Done(_, ppln) => Some(ppln),
            _ => None
        }
    }

    pub fn bg(&self) {
        println!("[{}]  + {} suspended  {}", "jobid", "pid", "command");
    }

    pub fn fg(&mut self) -> Result<(), nix::Error> {
        tcsetpgrp(libc::STDIN_FILENO, self.pgid)?;
        self.wait()?;
        tcsetpgrp(libc::STDIN_FILENO, getpid())?;
        Ok(())
    }

    pub fn is_completed(&self) -> bool {
        self.commands.iter().all(|c| c.is_completed())
    }

    pub fn wait(&mut self) -> Result<(), nix::Error> {
        while !self.is_completed() { // FIXME
            let status = waitpid(-1 * self.pgid, Some(WUNTRACED))?;

            match status {
                WaitStatus::Exited(pid, _) => self.commands.iter_mut().find(|c| c.pid == pid).unwrap().status(Status::Completed),  // FIXME
                WaitStatus::Stopped(pid, _) => self.commands.iter_mut().find(|c| c.pid == pid).unwrap().status(Status::Suspended),
                _ => panic!("unimplemented!"),
            };
        }

        Ok(())
    }

    pub fn spawn(&mut self) -> io::Result<Vec<libc::pid_t>> {
        let mut pgid: Option<libc::pid_t> = None;
        let mut pids = vec![];

        if self.commands.len() == 1 {
            let first = self.commands.first_mut().unwrap();

            let pid = first.spawn(pgid.unwrap_or(0))?;
            first.pid(pid);
            pids.push(pid);

            pgid = pgid.or(Some(pid));
        }

        else {
            let mut pastfdin: Option<RawFd> = None;

            for i in 0..self.commands.len() - 1 {
                let (fdin, fdout) = pipe()?;

                self.commands[i].stdout(StdX::Pipe(fdout));
                self.commands[i + 1].stdin(StdX::Pipe(fdin));

                let pid = self.commands[i].spawn(pgid.unwrap_or(0))?;
                self.commands[i].pid(pid);
                pids.push(pid);

                pgid = pgid.or(Some(pid));

                close(fdout)?;
                close_fd_opt(pastfdin)?;

                pastfdin = Some(fdin);
            }

            let last = self.commands.last_mut().unwrap();

            let pid = last.spawn(pgid.unwrap_or(0))?;
            last.pid(pid);
            pids.push(pid);

            close_fd_opt(pastfdin)?;
        }

        self.pgid(pgid.unwrap());
        Ok(pids)
    }

}


// nom parser
named!(parse_pipeline<&str, Pipeline>,
    do_parse!(
        init: parse_command >>
        commands:
            fold_many0!(
                do_parse!(
                    tag!("|") >>
                    command: ws!(parse_command) >>
                    (command)
                ),
                vec![init],
                |mut acc: Vec<Command>, cmd| { acc.push(cmd); acc }
            ) >>
        background: opt!(complete!(tag!("&"))) >>

        (Pipeline { commands: commands, background: background.is_some(), jobid: 0, pgid: 0, } )
    )
);

#[test]
fn test_pipeline_new() {
    assert_eq!(
        Pipeline::parse("ls &"),
        Some(
            Pipeline {
                commands: vec![Command::new("ls")],
                background: true,
                jobid: 0,
                pgid: 0,
            }
        )
    );

    assert_eq!(
        Pipeline::parse("ls | wc &"),
        Some(
            Pipeline {
                commands: vec![Command::new("ls"), Command::new("wc")],
                background: true,
                jobid: 0,
                pgid: 0,
            }
        )
    );
}
