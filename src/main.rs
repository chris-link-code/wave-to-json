use std::io::BufReader;
use std::time::Instant;
use std::{collections::HashMap, fs::File, io::Read, io::Write};

const LEN_CHUNK_DESCRIPTOR: usize = 4;
const LEN_WAVE_FLAG: usize = 4;
const LEN_FMT_SUB_CHUNK: usize = 4;
const LEN_DATA_SUB_CHUNK: usize = 4;

const RIFF: &str = "RIFF";
const WAVE: &str = "WAVE";
const FMT_: &str = "fmt ";
const DATA: &str = "data";

const WIDTH: u32 = 2000;

//https://www.bilibili.com/video/BV1wG4y1Y7RQ/
// 用到RMS算法
fn main() {
    let now = Instant::now();
    // let path = "http://192.168.1.2:8099/2.wav";
    let path = "../123.wav";
    let mut wave_to_json = WaveToJson::new(path);
    let result_data = wave_to_json.decode();
    println!("解码时间: {}", now.elapsed().as_millis());
    let mut json_file = File::create("data.json").expect("create failed");
    json_file.write(b"[").unwrap();
    for v in result_data.iter() {
        json_file.write(v.to_string().as_bytes()).unwrap();
        json_file.write(b",").unwrap();
    }
    json_file.write(b"]").unwrap();
}

struct WaveToJson {
    chunk_descriptor: String,
    chunk_size: u64,
    wave_flag: String,
    fmt_sub_chunk: String,
    sub_chunk1_size: u64,
    audio_format: i32,
    num_channels: u32,
    sample_rate: u64,
    byte_rate: u64,
    block_align: i32,
    bits_per_sample: u32,
    sub_chunk2_size: u32,
    data: String,
    length: u32,
    reader: BufReader<File>,
    sample_data: HashMap<u32, Vec<f64>>,
}

impl WaveToJson {
    fn new(path: &str) -> Self {
        // let prefix = path.strip_prefix("http");
        // match prefix {
        //     Some(_) => {
        //         let resp = reqwest::blocking::get(path).unwrap();
        //         let reader = BufReader::new(resp);
        //     }
        //     None => {
        //         let file = File::open(path).unwrap();
        //         let reader = BufReader::new(file);
        //     }
        // }
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);
        Self {
            reader,
            chunk_descriptor: Default::default(),
            chunk_size: Default::default(),
            wave_flag: Default::default(),
            fmt_sub_chunk: Default::default(),
            sub_chunk1_size: Default::default(),
            audio_format: Default::default(),
            num_channels: Default::default(),
            sample_rate: Default::default(),
            byte_rate: Default::default(),
            block_align: Default::default(),
            bits_per_sample: Default::default(),
            sub_chunk2_size: Default::default(),
            data: Default::default(),
            length: Default::default(),
            sample_data: Default::default(),
        }
    }

    fn decode(&mut self) -> Vec<f64> {

        // TODO 非标准格式解码时，仅提取data后的数据
        let mut buf = vec![0u8; 2];
        self.chunk_descriptor = self.read_string(LEN_CHUNK_DESCRIPTOR);
        if !self.chunk_descriptor.eq(RIFF) {
            println!("非标准格式")
        }
        self.chunk_size = self.read_long();
        self.wave_flag = self.read_string(LEN_WAVE_FLAG);
        if !self.wave_flag.eq(WAVE) {
            println!("非标准格式")
        }
        self.fmt_sub_chunk = self.read_string(LEN_FMT_SUB_CHUNK);
        if !self.fmt_sub_chunk.eq(FMT_) {
            println!("非标准格式")
        }
        self.sub_chunk1_size = self.read_long();
        self.audio_format = self.read_int(&mut buf);
        self.num_channels = self.read_int(&mut buf) as u32;
        self.sample_rate = self.read_long();
        self.byte_rate = self.read_long();
        self.block_align = self.read_int(&mut buf);
        self.bits_per_sample = self.read_int(&mut buf) as u32;

        self.data = self.read_string(LEN_DATA_SUB_CHUNK);
        if !self.data.eq(DATA) {
            println!("非标准格式")
        }
        self.sub_chunk2_size = self.read_long() as u32;
        self.length =
            (self.sub_chunk2_size / (self.bits_per_sample / 8) / self.num_channels) as u32;

        self.read_data(self.length);

        let mut result_data: Vec<f64> = Vec::new();

        let len = self.sample_data.len() as u32;
        match len {
            1 => {
                result_data = self.sample_data.get(&0).unwrap().to_vec();
            }
            _ => {
                let mut max4 = 0;
                for i in 0..len {
                    if i <= len - 2 {
                        max4 = self
                            .sample_data
                            .get(&i)
                            .unwrap()
                            .len()
                            .max(self.sample_data.get(&(i + 1)).unwrap().len())
                    }
                }
                for i in 0..max4 {
                    for j in 0..len {
                        if self.sample_data.get(&j).unwrap().len() >= i {
                            let val = self.sample_data.get(&j).unwrap().get(i).unwrap();
                            result_data.push((*val * 100.0).round() / 100.0);
                        }
                    }
                }
            }
        }

        result_data
    }

    fn read_data(&mut self, length: u32) {
        let size;
        if length <= WIDTH {
            size = 1;
        } else {
            size = length / WIDTH;
        }

        for i in 0..self.num_channels {
            self.sample_data.insert(i, Vec::new());
        }

        let buf: &mut Vec<u8> = &mut vec![0u8; 2];

        let mut sample_sum: i64 = 0;
        let mut i = 0;
        while i < length {
            let mut n = 0;
            while n < self.num_channels {
                match self.bits_per_sample {
                    8 => {
                        if i == length - 1 {
                            self.handle8bit(n, sample_sum, size);
                        } else {
                            let mut buf = vec![0u8; 1];
                            self.reader.read(&mut buf).unwrap();
                            if i != 0 && (i % size) == 0 {
                                self.handle8bit(n, sample_sum, size);
                                sample_sum = 0;
                            } else {
                                sample_sum += (buf[0] as i64).pow(2);
                            }
                        }
                    }
                    16 => {
                        if i == length - 1 {
                            self.handle16bit(n, sample_sum, size);
                        } else {
                            let val = self.read_int(buf) as i64;
                            if i != 0 && (i % size) == 0 {
                                self.handle16bit(n, sample_sum, size);
                                sample_sum = 0;
                            } else {
                                sample_sum += val.pow(2);
                            }
                        }
                    }
                    _ => {}
                }

                n += 1;
            }
            i += 1;
        }
    }

    fn handle8bit(&mut self, key: u32, sample_sum: i64, size: u32) {
        self.handle_bit(key, sample_sum, size, 128f32);
    }

    fn handle16bit(&mut self, key: u32, sample_sum: i64, size: u32) {
        self.handle_bit(key, sample_sum, size, 32768f32);
    }

    fn handle24bit(&mut self, key: u32, sample_sum: i64, size: u32) {
        self.handle_bit(key, sample_sum, size, 8388608f32);
    }

    fn handle32bit(&mut self, key: u32, sample_sum: i64, size: u32) {
        self.handle_bit(key, sample_sum, size, 2147483648f32);
    }

    fn handle_bit(&mut self, key: u32, sample_sum: i64, size: u32, scope: f32) {
        let sample_arr = self.sample_data.get_mut(&key).unwrap();
        let data = cal_rms(sample_sum as f32 / scope, size);
        sample_arr.push(data);
    }

    fn read_string(&mut self, len: usize) -> String {
        let mut buf = vec![0u8; len];
        self.reader.read(&mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    fn read_long(&mut self) -> u64 {
        let mut buffer = [0u64; 4];
        let mut i = 0;
        while i < 4 {
            let mut buf = vec![0u8; 1];
            self.reader.read(&mut buf).unwrap();
            buffer[i] = buf[0] as u64;
            i += 1;
        }
        buffer[0] | (buffer[1] << 8) | (buffer[2] << 16) | (buffer[3] << 24)
    }

    fn read_int(&mut self, buf: &mut Vec<u8>) -> i32 {
        self.reader.read(buf).unwrap();
        buf[0] as i32 | ((buf[1] as i8) as i32) << 8
    }
}

fn cal_rms(sample_sum: f32, size: u32) -> f64 {
    (sample_sum / size as f32).sqrt() as f64
}
