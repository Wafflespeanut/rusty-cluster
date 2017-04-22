extern crate bincode;
#[macro_use] extern crate lazy_static;
extern crate serde;
extern crate serde_json;
#[macro_use] extern crate serde_derive;

pub mod master;
#[macro_use] mod utils;
pub mod slave;

use bincode::Infinite;
use utils::DjB2;

use std::hash::Hasher;
use std::io::{BufReader, BufWriter, Read, Write};
use std::mem;

const BUFFER_SIZE: usize = 8192;

lazy_static! {
    static ref HASHED: Vec<u64> = {
        (0..ProcessType::Shutdown as usize).map(|i| {   // Minor hack: Assume that Shutdown is always the last variant
            let mut h = DjB2::new();
            h.write_u64(i as u64);
            h.finish()
        }).collect()
    };
}

#[repr(u64)]
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum ProcessType {
    Ping,
    Execute,
    Shutdown,           // NOTE: This should always be the last variant.
}

impl ProcessType {
    fn hash(&self) -> [u8; 8] {
        let mut h = DjB2::new();
        h.write_u64(*self as u64);
        unsafe { mem::transmute(h.finish()) }
    }

    fn into_stream<W: Write>(&self, stream: &mut BufWriter<W>) -> Result<(), String> {
        let bytes = self.hash();
        let _ = stream.write(&bytes)
                      .map_err(|e| format!("Cannot write ProcessType into stream ({})", e))?;
        Ok(())
    }

    #[inline]
    fn from_stream<R: Read>(stream: &mut BufReader<R>) -> Result<ProcessType, String> {
        let mut bytes = [0; 8];
        let _ = stream.read_exact(&mut bytes)
                      .map_err(|e| format!("Cannot read ProcessType from stream ({})", e))?;
        let hash: u64 = unsafe { mem::transmute(bytes) };
        HASHED.iter().enumerate().find(|&(_, v)| v == &hash)
                     .map(|(i, _)| unsafe { mem::transmute(i) })
                     .ok_or(String::from("Invalid process type in stream"))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Data(Vec<u8>);

impl Data {
    pub fn serialize_into<W: Write>(&self, stream: &mut BufWriter<W>) -> Result<(), String> {
        let mut writer = BufWriter::with_capacity(BUFFER_SIZE, stream);
        bincode::serialize_into(&mut writer, &self.0, Infinite)
                .map_err(|e| format!("Cannot serialize data into stream ({})", e))
    }

    pub fn deserialize_from<R: Read>(stream: &mut BufReader<R>) -> Result<Data, String> {
        let mut reader = BufReader::with_capacity(BUFFER_SIZE, stream);
        bincode::deserialize_from(&mut reader, Infinite)
                .map_err(|e| format!("Cannot deserialize data from stream ({})", e))
    }
}