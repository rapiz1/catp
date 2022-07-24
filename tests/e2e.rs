use catp::{catp, CatpArgs};
use std::{io::Read, process, thread};

#[test]
fn hello() {
    let mut p = process::Command::new("tests/scripts/hello")
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .spawn()
        .unwrap();

    let args = CatpArgs {
        fd: 1,
        pid: p.id(),
        verbose: false,
    };

    let t = thread::spawn(move || {
        let mut actual: Vec<u8> = vec![];
        catp(args, &mut actual).unwrap();
        actual
    });

    // Start hello
    drop(p.stdin.take());

    // We only need to wait for catp to avoid race because catp waits for hello
    let actual = t.join().unwrap();

    let mut expected = vec![];
    p.stdout.take().unwrap().read_to_end(&mut expected).unwrap();

    assert_eq!(actual, *expected);
}
