use std::process::Command;

fn main() {
    let output = Command::new("tar")
        .args(&["-cJf", "lang.tar.xz", "./lang"])
        .output()
        .expect("Zhopa");

    if !(output.status.code() == Some(0)) {
        eprintln!("Can't create langs.tar");
        std::process::exit(-1);
    }
}
