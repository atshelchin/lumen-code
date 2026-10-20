// Minimal JSON reader for the conformance tests (keeps the crate dependency-free).
#![allow(dead_code)]
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(BTreeMap<String, J>),
}

impl J {
    pub fn get(&self, k: &str) -> &J {
        match self {
            J::Obj(m) => m.get(k).unwrap_or_else(|| panic!("missing key {k}")),
            _ => panic!("not an object"),
        }
    }
    pub fn opt(&self, k: &str) -> Option<&J> {
        match self {
            J::Obj(m) => m.get(k),
            _ => None,
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, J::Null)
    }
    pub fn b(&self) -> bool {
        match self {
            J::Bool(v) => *v,
            _ => panic!("not a bool: {self:?}"),
        }
    }
    pub fn at(&self, i: usize) -> &J {
        match self {
            J::Arr(a) => &a[i],
            _ => panic!("not an array"),
        }
    }
    pub fn arr(&self) -> &Vec<J> {
        match self {
            J::Arr(a) => a,
            _ => panic!("not an array"),
        }
    }
    pub fn num(&self) -> f64 {
        match self {
            J::Num(n) => *n,
            _ => panic!("not a number: {self:?}"),
        }
    }
    pub fn u(&self) -> u64 {
        self.num() as u64
    }
    pub fn s(&self) -> &str {
        match self {
            J::Str(s) => s,
            _ => panic!("not a string"),
        }
    }
    pub fn hex(&self) -> Vec<u8> {
        let s = self.s();
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }
    pub fn us(&self) -> Vec<u64> {
        self.arr().iter().map(|v| v.u()).collect()
    }
}

pub fn parse(s: &str) -> J {
    let b = s.as_bytes();
    let mut i = 0;
    val(b, &mut i)
}

fn ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && (b[*i] as char).is_ascii_whitespace() {
        *i += 1;
    }
}

fn val(b: &[u8], i: &mut usize) -> J {
    ws(b, i);
    match b[*i] {
        b'{' => {
            *i += 1;
            let mut m = BTreeMap::new();
            loop {
                ws(b, i);
                if b[*i] == b'}' {
                    *i += 1;
                    break;
                }
                let k = match val(b, i) {
                    J::Str(s) => s,
                    _ => panic!(),
                };
                ws(b, i);
                *i += 1; // :
                let v = val(b, i);
                m.insert(k, v);
                ws(b, i);
                if b[*i] == b',' {
                    *i += 1;
                }
            }
            J::Obj(m)
        }
        b'[' => {
            *i += 1;
            let mut a = Vec::new();
            loop {
                ws(b, i);
                if b[*i] == b']' {
                    *i += 1;
                    break;
                }
                a.push(val(b, i));
                ws(b, i);
                if b[*i] == b',' {
                    *i += 1;
                }
            }
            J::Arr(a)
        }
        b'"' => {
            *i += 1;
            let mut out = Vec::new();
            while b[*i] != b'"' {
                if b[*i] == b'\\' {
                    *i += 1;
                    match b[*i] {
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let cp = u32::from_str_radix(std::str::from_utf8(&b[*i + 1..*i + 5]).unwrap(), 16).unwrap();
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(char::from_u32(cp).unwrap_or('?').encode_utf8(&mut buf).as_bytes());
                            *i += 4;
                        }
                        c => out.push(c),
                    }
                } else {
                    out.push(b[*i]);
                }
                *i += 1;
            }
            *i += 1;
            J::Str(String::from_utf8(out).unwrap())
        }
        b't' => {
            *i += 4;
            J::Bool(true)
        }
        b'f' => {
            *i += 5;
            J::Bool(false)
        }
        b'n' => {
            *i += 4;
            J::Null
        }
        _ => {
            let st = *i;
            while *i < b.len() && matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                *i += 1;
            }
            J::Num(std::str::from_utf8(&b[st..*i]).unwrap().parse().unwrap())
        }
    }
}
