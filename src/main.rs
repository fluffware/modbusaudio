//! A demonstration of constructing and using a non-blocking stream.
//!
//! Audio from the default input device is passed directly to the default output device in a duplex
//! stream, so beware of feedback!

extern crate portaudio;
extern crate hound;

use portaudio as pa;
use std::sync::Arc;
use std::sync::Mutex;
use std::fs::File;
use std::path::Path;
use std::ffi::OsStr;
use std::env;
use std::io::Write;
use std::io::BufReader;
use std::io::BufRead;
use std::error::Error;

use std::collections::btree_map::BTreeMap;

const SAMPLE_RATE: f64 = 44_100.0;
const CHANNELS: i32 = 2;
const FRAMES_PER_BUFFER: u32 = 1024;

mod split_quoted;
use split_quoted::split_quoted;

mod modbus_server;

macro_rules! print_err {
    ($fmt:expr) => {writeln!(&mut std::io::stderr(),
                             $fmt).unwrap()};
    ($fmt:expr, $($arg:tt)*) => {writeln!(&mut std::io::stderr(),
                                          $fmt,
                                          $($arg)*).unwrap()}
}

struct ServerOps
{
}

impl ServerOps
{
    fn new() -> ServerOps
    {
        ServerOps{}
    }
}
         
impl modbus_server::Ops for ServerOps
{
    fn get_input(&self, addr: u16) -> Result<bool, u8> {
        Err(modbus_server::ILLEGAL_DATA_ADDRESS)
    }

    fn get_coil(&self, addr: u16) -> Result<bool, u8> {
        Err(modbus_server::ILLEGAL_DATA_ADDRESS)
    }
    
    fn set_coil(&mut self, addr: u16, v: bool) -> Result<bool, u8> {
        Ok(v)
    }
    
}

fn main() {
    let mut audio_clips = BTreeMap::<u16, Vec<i16>>::new();
    let args = env::args_os();
    let mut args = args.skip(1);
    let conf_path_str = 
        if let Some(path) = args.next() {
            path
        } else {
            OsStr::new("modbusaudio.conf").to_os_string()
        };
    let conf =
        match read_config(Path::new(&conf_path_str)) {
            Err(err) => {
                print_err!("Failed to read configuration file {:?}: {:?}", conf_path_str, err.description());
                return
            },
            Ok(c) => c
        };
    for line in conf {
        if line.cmd == "audio" {
            if line.args.len() < 2 {
                 print_err!("Too few arguments for audio clip");
                return
            }
            let slot = 
                match line.args[0].parse::<u16>() {
                    Ok(i) => i,
                    Err(err) => {
                        print_err!("Invalid audio slot: {}",
                                   err.description());
                        return
                    }
                };

            let path = Path::new(&line.args[1]);
            match hound::WavReader::open(path) {
                Ok(mut reader) => {
                    let sbuffer = reader.samples::<i16>()
                        .map(|r| {r.unwrap()}).collect::<Vec<i16>>();
                    audio_clips.insert(slot, sbuffer);
                    println!("WAV: {:?}", reader.spec());
                },
                Err(err) => {
                    print_err!("Failed to open audio file \"{}\": {}",
                               &line.args[1], err.description());
                    return
                }
            }
    
        }
        println!("Cmd: {}", line.cmd);
    }
    let audio_clips = Arc::new(audio_clips);
    
    let server = modbus_server::Server::new("0.0.0.0:5020", Arc::new(Mutex::new(ServerOps::new()))).unwrap();
    //run().unwrap()
    server.stop();
}

struct Config
{
    cmd: String,
    args: Vec<String>
}

fn read_config(path: &Path) -> std::io::Result<Vec<Config>>
{
    let mut conf = Vec::<Config>::new();
    let f = File::open(path)?;
    let reader = BufReader::new(f);
    for line_res in  reader.lines() {
        if let Ok(line) = line_res {
            let mut tokens = split_quoted(&line);
            if let Some(cmd) = tokens.next() {
                if cmd.starts_with("#") {
                    // Comment, ignore
                } else {
                    let conf_line = 
                        Config{cmd: cmd.to_string(), 
                               args: tokens.map(|arg| arg.to_string()).collect::<Vec<String>>()};
                    conf.push(conf_line);
                }
            }   
        }
    }
    Ok(conf)
}

fn run() -> Result<(), pa::Error> {

    let sbuffer = vec!(0);
    let pa = try!(pa::PortAudio::new());		
    
    println!("PortAudio:");
    println!("version: {}", pa.version());
    println!("version text: {:?}", pa.version_text());
    println!("host count: {}", try!(pa.host_api_count()));

    let default_host = try!(pa.default_host_api());
    println!("default host: {:#?}", pa.host_api_info(default_host));

    

    let def_output = try!(pa.default_output_device());
    let output_info = try!(pa.device_info(def_output));
    println!("Default output device info: {:#?}", &output_info);

    let mut settings: pa::OutputStreamSettings<i16> =
        try!(pa.default_output_stream_settings(CHANNELS, SAMPLE_RATE, 
                                               FRAMES_PER_BUFFER));
    // we won't output out of range samples so don't bother clipping them.
    settings.flags = pa::stream_flags::CLIP_OFF;

    let mut sbuffer_pos:usize = 0;

    // Callback that plays the audio in sbuffer
    let callback = move |pa::OutputStreamCallbackArgs { buffer, frames, .. }| {
    	let samples = 2 * frames;
    	let copy_len = if sbuffer_pos + samples > sbuffer.len() {
	    sbuffer.len()- sbuffer_pos
	} else {
	    samples
	};
	
    	buffer[0..copy_len].copy_from_slice(&sbuffer[sbuffer_pos..sbuffer_pos + copy_len]);
	sbuffer_pos += copy_len;
        pa::Continue
    };

    let mut stream = try!(pa.open_non_blocking_stream(settings, callback));

    try!(stream.start());

    
    pa.sleep(5 * 1_000);

    try!(stream.stop());
    try!(stream.close());

    println!("Test finished.");



    Ok(())
}

