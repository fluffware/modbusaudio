
use std::net::TcpListener;
use std::net::ToSocketAddrs;
use std::net::TcpStream;
use std;
use std::thread;
use std::thread::JoinHandle;
use std::error::Error;
use std::io::Read;
use std::io::Write;
use std::io::ErrorKind;
use std::time::Duration;

macro_rules! print_err {
    ($fmt:expr) => {writeln!(&mut std::io::stderr(),
                             $fmt).unwrap()};
    ($fmt:expr, $($arg:tt)*) => {writeln!(&mut std::io::stderr(),
                                          $fmt,
                                          $($arg)*).unwrap()}
}
pub struct Server
{
    t: JoinHandle<u32>
}


fn be16_to_u16(bytes: &[u8]) -> u16
{
    return (bytes[0] as u16) << 8 | bytes[1] as u16;
}

fn u16_to_be16(v: u16, bytes: & mut [u8])
{
    bytes[0] = (v >> 8) as u8;
    bytes[1] = v as u8;
}

enum MBFunc {
    
}

const WRITE_SINGLE_COIL:u8 = 5;

const ILLEGAL_FUNCTION: u8 = 1;

fn handle_request(req: & [u8]) -> Vec<u8>
{
    println!("Req: {:?}", req);
    match req[0] {
        WRITE_SINGLE_COIL => vec![WRITE_SINGLE_COIL, req[3], req[4]],
        c => vec![c | 0x80, 0x01]
    }
    
}

fn handle_connection(mut stream: TcpStream)
{
    thread::spawn(move || {
        stream.set_read_timeout(Some(Duration::new(1,0)));
        let mut read_buffer = [0u8;16];
        let mut buf = Vec::<u8>::new();
        println!("new client!");
        loop {
            match stream.read(&mut read_buffer) {
                Ok(r) => {
                    if r == 0 {break;};
                    buf.extend(&read_buffer[0..r]);
                    println!("Read {}",buf.len());
                    if buf.len() >= 8 {
                        let proto = be16_to_u16(&buf[2..4]);
                        if proto != 0 {
                            buf.truncate(0);
                            continue;
                        }
                        let msg_len = be16_to_u16(&buf[4..6]);
                        if buf.len() >= (msg_len + 6) as usize {
                            let mut msg = buf.drain(0..(msg_len+6) as usize).collect::<Vec<u8>>();
                            let reply = handle_request(&msg[7..]);

                            msg.truncate(7);
                            msg.extend(&reply);
                            u16_to_be16((reply.len() + 1) as u16, &mut msg[4..6]);
                            stream.write_all(&msg);
                        }

                    }
                },
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut {
                        buf.truncate(0);
                    } else {
                            print_err!("Read error: {}", e.description());
                            break;
                    }
                }
            }
            
        }
    });
}
impl Server {
    pub fn new<A: ToSocketAddrs>(addr: A) -> std::io::Result<Server> {
        let listener = TcpListener::bind(addr)?;
        let t = thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        handle_connection(stream);
                    }
                    Err(e) => { println!("Failed to accept: {}", e.description());}
                }
            }
            6
        });
        Ok(Server {t: t})
    }

    pub fn stop(mut self) {
        self.t.join();
    }
}
