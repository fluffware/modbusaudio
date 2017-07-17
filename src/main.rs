//! A demonstration of constructing and using a non-blocking stream.
//!
//! Audio from the default input device is passed directly to the default output device in a duplex
//! stream, so beware of feedback!

extern crate portaudio;
extern crate hound;

use portaudio as pa;
use std::sync::Arc;
use std::fs::File;
use std::path::Path;
use std::ffi::OsStr;
use std::env;
use std::io::Write;
use std::io::BufReader;
use std::io::BufRead;
use std::error::Error;

const SAMPLE_RATE: f64 = 44_100.0;
const CHANNELS: i32 = 2;
const FRAMES_PER_BUFFER: u32 = 1024;

struct SplitQuoted<'a>
{
    s: &'a str,
    pos: usize
}

impl<'a> Iterator for SplitQuoted<'a>
{
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        Some("g")
    }
}

fn split_quoted<'a>(s: &'a str) -> SplitQuoted {
    for c in s.chars() {
        
    }
    SplitQuoted {s: s, pos: 0}
}

fn main() {
    let  args = env::args_os();
    let mut args = args.skip(1);
    let conf_path_str = 
        if let Some(path) = args.next() {
            path
        } else {
            OsStr::new("modbusaudio.conf").to_os_string()
        };
    if let Err(err) = read_config(Path::new(&conf_path_str)) {
        writeln!(&mut std::io::stderr(), "Failed to read configuration file {:?}: {:?}", conf_path_str, err.description()).unwrap();
    }
    run().unwrap()
}

fn read_config(path: &Path) -> std::io::Result<()>
{
    let f = File::open(path)?;
    let reader = BufReader::new(f);
    for line_res in  reader.lines() {
        if let Ok(line) = line_res {
            let trimmed = line.trim();
            if trimmed.starts_with("audio") {
                
            } else {
                println!("Line: {}", line);
            }
        }
    }
    Ok(())
}

fn run() -> Result<(), pa::Error> {

    let mut reader = hound::WavReader::open("D:\\Ljud\\Alarm.wav").unwrap();
    let sbuffer = Arc::new(reader.samples::<i16>()
                           .map(|r| {r.unwrap()}).collect::<Vec<i16>>());
    println!("WAV: {:?}", reader.spec());
    
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
