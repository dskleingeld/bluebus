use bluebus::BleBuilder;
use nix::poll::{poll, PollFd, PollFlags};
use std::fs::File;
use std::io::prelude::*;
use std::os::unix::io::FromRawFd;
use std::time::Instant;

const DEVICE_ADDRESS: &'static str = "0A:0A:0A:0A:0A:0A";

fn main() {
    let mut ble = BleBuilder::default().build().unwrap();
    ble.connect(DEVICE_ADDRESS).unwrap();
    dbg!(ble.is_connected(DEVICE_ADDRESS).unwrap());

    let mut fd = ble
        .notify(DEVICE_ADDRESS, "93700001-1bb7-1599-985b-f5e7dc991483")
        .unwrap();

    let mut counter = 0u32;
    let mut start = Instant::now();

    let mut buffer = [0u8; 4];
    let mut expected = None;
    let pollfd = PollFd::new(fd, PollFlags::POLLIN);
    let mut file = unsafe { File::from_raw_fd(fd) };
    loop {
        if let Err(_) = poll(&mut [pollfd], -1) {
            continue;
        }
        let nread = file.read(&mut buffer).unwrap();
        //if file.read(&mut buffer).is_err(){
        //    continue;
        //}
        if nread != 4 {
            println!("nread: {}", nread);
        }

        let new = u32::from_le_bytes(buffer);
        if let Some(expected) = expected {
            if new != expected {
                println!("error: new != prev+1, {} != {}", new, expected);
            }
        }
        expected = Some(new + 1);
        //expected = expected.map_or(Some(new+1), |e| Some(e+1));

        counter += 1;
        if counter == 10_000 {
            let freq = (counter as f32) / start.elapsed().as_secs_f32();
            println!("recieved {} numbers at {} hz", counter, freq);
            counter = 0;
            start = Instant::now();
        }
    }

    //ble.disconnect("C6:46:56:AC:2C:4C").unwrap();
    //dbg!(ble.is_connected("C6:46:56:AC:2C:4C").unwrap());
}
