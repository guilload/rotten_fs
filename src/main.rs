use std::io::prelude::*;
use std::io;
use std::process;


#[derive(Debug, PartialEq)]
struct Command {
    program: String,
    args: Vec<String>,
}


impl Command {

    fn new(command: &str) -> Option<Self> {
        let mut tokens = command.trim().split_whitespace().map(|t| t.to_string());
        tokens.next().map(|program| Command { program: program, args: tokens.collect() })
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
        Command::new("ls -la").unwrap(),
        Command { program: "ls".to_string(), args: vec!["-la".to_string()] }
    );
}


