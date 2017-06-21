#[macro_use]
extern crate nom;

use std::io::prelude::*;
use std::io;
use std::process;

use nom::*;


named!(parse_command<&str, Command>,
    do_parse!(
        program: ws!(alpha) >>
        args: many0!(ws!(is_not!(" "))) >>
        (Command { program: program.to_string(), args: args.iter().map(|a| a.to_string()).collect() })
    )
);


#[derive(Debug, PartialEq)]
struct Command {
    program: String,
    args: Vec<String>,
}


impl Command {

    fn new(command: &str) -> Option<Self> {
        match parse_command(command) {
            IResult::Done(_, cmd) => Some(cmd),
            _ => None
        }
    }

    fn run(&self) -> io::Result<()> {
        process::Command::new(&self.program)
            .args(&self.args)
            .spawn()?
            .wait()?;

        Ok(())
    }

}

fn main() {
    println!("Rotten sh...");

    loop {
        print!("$ ");
        std::io::stdout().flush();

        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer);

        if let Some(cmd) = Command::new(&buffer) {
            if let Err(e) = cmd.run() {
                println!("Command failed: {:?}", e);
            }
        }
    }
}


#[test]
fn test_command_new() {
    assert_eq!(
        Command::new("ls").unwrap(),
        Command { program: "ls".to_string(), args: vec![] }
    );

    assert_eq!(
        Command::new("ls -la").unwrap(),
        Command { program: "ls".to_string(), args: vec!["-la".to_string()] }
    );

    assert_eq!(
        Command::new("rm -rf dir").unwrap(),
        Command { program: "rm".to_string(), args: vec!["-rf".to_string(), "dir".to_string()] }
    );
}


