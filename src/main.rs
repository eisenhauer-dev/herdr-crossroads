use std::io;

fn main() -> io::Result<()> {
    println!("Herdr Crossroads");
    println!("Press Enter to close.");

    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(())
}
