
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
use std::sync::Arc;
use std::sync::Mutex;
use std::iter::Iterator;

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

#[allow(dead_code)]
pub const READ_DISCRETE_INPUTS:u8 = 2;

#[allow(dead_code)]
pub const READ_COILS:u8 = 1;

#[allow(dead_code)]
pub const WRITE_SINGLE_COIL:u8 = 5;
#[allow(dead_code)]
pub const WRITE_MULTIPLE_COILS:u8 = 15;

#[allow(dead_code)]
pub const ILLEGAL_FUNCTION: u8 = 1;
#[allow(dead_code)]
pub const ILLEGAL_DATA_ADDRESS: u8 = 2;
#[allow(dead_code)]
pub const ILLEGAL_DATA_VALUE: u8 = 3;
#[allow(dead_code)]
pub const SERVER_DEVICE_FAILURE: u8 = 4;
#[allow(dead_code)]
pub const ACKNOWLEDGE: u8 = 5;
#[allow(dead_code)]
pub const SERVER_DEVICE_BUSY: u8 = 6;
#[allow(dead_code)]
pub const MEMORY_PARITY_ERROR: u8 = 8;
#[allow(dead_code)]
pub const GATEWAY_PATH_UNAVAILABLE: u8 = 0xa;
#[allow(dead_code)]
pub const GATEWAY_TARGET_DEVICE_FAILED_TO_RESPOND: u8 = 0xb;

fn handle_request(req: & [u8], ops:Arc<Mutex<Ops>>) -> Vec<u8>
{
    //println!("Req: {:?}", req);
    let mut ops = ops.lock().unwrap();
    match req[0] {
        WRITE_SINGLE_COIL => {
            if req.len() != 5 {
                return vec![WRITE_SINGLE_COIL | 0x80, ILLEGAL_DATA_VALUE];
            }
            let addr = be16_to_u16(&req[1..3]);
            match (*ops).set_coil(addr,req[3] != 0x00) {
                Ok(v) =>            vec![WRITE_SINGLE_COIL, 
                                         if v {0xff} else {0x00}, 0x00],
                Err(code) => vec![WRITE_SINGLE_COIL | 0x80, code]
            }
        },
        WRITE_MULTIPLE_COILS => {
            if req.len() < 6 {
                return vec![WRITE_MULTIPLE_COILS | 0x80, ILLEGAL_DATA_VALUE];
            }
            let addr = be16_to_u16(&req[1..3]);
            let len = be16_to_u16(&req[3..5]);
            let n_bytes = req[5];
            if (n_bytes + 6) as usize != req.len() {
                return vec![WRITE_MULTIPLE_COILS | 0x80, ILLEGAL_DATA_VALUE];
            }
            let mut bits = Vec::<bool>::new();
            for byte in &req[6..] {
                let mut byte = *byte;
                for _ in 0..7 {
                    bits.push((byte & 1) != 0);
                    byte >>= 1;
                }
            }
            bits.truncate(len as usize);
            match (*ops).set_coils(addr,&bits) {
                Ok(_v) => req[0..5].to_vec(), 
                Err(code) => vec![WRITE_SINGLE_COIL | 0x80, code]
            }
        }
        c => vec![c | 0x80, ILLEGAL_FUNCTION]
    }
    
}

fn handle_connection(mut stream: TcpStream, ops: Arc<Mutex<Ops>>)
{
    thread::spawn(move || {
        stream.set_read_timeout(Some(Duration::new(1,0))).unwrap();
        let mut read_buffer = [0u8;16];
        let mut buf = Vec::<u8>::new();
        loop {
            match stream.read(&mut read_buffer) {
                Ok(r) => {
                    if r == 0 {break;};
                    buf.extend(&read_buffer[0..r]);
                    //println!("Read {}",buf.len());
                    if buf.len() >= 8 {
                        let proto = be16_to_u16(&buf[2..4]);
                        if proto != 0 {
                            buf.truncate(0);
                            continue;
                        }
                        let msg_len = be16_to_u16(&buf[4..6]);
                        if buf.len() >= (msg_len + 6) as usize {
                            let mut msg = buf.drain(0..(msg_len+6) as usize).collect::<Vec<u8>>();
                            let reply = handle_request(&msg[7..],ops.clone());

                            msg.truncate(7);
                            msg.extend(&reply);
                            u16_to_be16((reply.len() + 1) as u16, &mut msg[4..6]);
                            if let Err(err) = stream.write_all(&msg) {
                                print_err!("Failed to write Modbus reply: {}",
                                           err.description());
                            }
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
    pub fn new<A: ToSocketAddrs>(addr: A, ops: Arc<Mutex<Ops>>) -> std::io::Result<Server> {
        let listener = TcpListener::bind(addr)?;
        let t = thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        handle_connection(stream, ops.clone());
                    }
                    Err(e) => { println!("Failed to accept: {}", e.description());}
                }
            }
            6
        });
        Ok(Server {t: t})
    }

    pub fn run(self) {
        self.t.join().unwrap();
    }
}

pub trait Ops: Send
{
    // Discrete inputs
    fn get_input(&self, addr: u16) -> Result<bool, u8>;
    fn get_inputs(&self, addr: u16, len: u16) -> Result<(), u8> {
        for a in addr..(addr+len) {
            if let Err(err) = self.get_input(a) {
                return Err(err);
            }
        }
        Ok(())
    }

    // Coils
    fn get_coil(&self, addr: u16) -> Result<bool, u8>;
    fn get_coils(&self, addr: u16, v: &[bool]) -> Result<(), u8> {
        for a in addr..(addr+v.len() as u16) {
            if let Err(err) = self.get_coil(a) {
                return Err(err);
            }
        }
        Ok(())
    }
    fn set_coil(&mut self, addr: u16, v: bool) -> Result<bool, u8>;
    fn set_coils(&mut self, addr: u16, v: &[bool]) -> Result<(), u8> {
        for (i,v) in v.iter().enumerate() {
            if let Err(err) = self.set_coil(i as u16+addr,*v) {
                return Err(err);
            }
        }
        Ok(())
    }

    
}
