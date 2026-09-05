//! The file `as` writes and the kernel runs: `\0arm`, a header, the
//! instructions, the data, and enough to debug it - which line each
//! instruction came from, every label, and the source itself.

pub const MAGIC: &[u8; 4] = b"\0arm";
const VERSION: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Image {
    pub text: Vec<u8>,
    pub data: Vec<u8>,
    pub bss: u32,
    pub entry: u64,
    /// `(address, 1-based source line)` for every instruction, in order.
    pub lines: Vec<(u32, u32)>,
    /// Every label, with its absolute address.
    pub symbols: Vec<(String, u64)>,
    pub source: String,
}

impl Image {
    pub fn is_image(bytes: &[u8]) -> bool {
        bytes.starts_with(MAGIC)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + self.text.len() + self.data.len() + self.source.len());
        out.extend_from_slice(MAGIC);
        for v in [
            VERSION,
            self.entry as u32,
            self.text.len() as u32,
            self.data.len() as u32,
            self.bss,
            self.lines.len() as u32,
            self.symbols.len() as u32,
            self.source.len() as u32,
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&self.text);
        out.extend_from_slice(&self.data);
        for (a, l) in &self.lines {
            out.extend_from_slice(&a.to_le_bytes());
            out.extend_from_slice(&l.to_le_bytes());
        }
        for (name, addr) in &self.symbols {
            out.extend_from_slice(&(*addr as u32).to_le_bytes());
            let name = name.as_bytes();
            out.extend_from_slice(&(name.len().min(65535) as u16).to_le_bytes());
            out.extend_from_slice(&name[..name.len().min(65535)]);
        }
        out.extend_from_slice(self.source.as_bytes());
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Image, String> {
        if !Self::is_image(bytes) {
            return Err("this is not an assembled program (no \\0arm header)".into());
        }
        let mut pos = 4;
        let u32_at = |pos: &mut usize| -> Result<u32, String> {
            let b = bytes.get(*pos..*pos + 4).ok_or("the file ends too early")?;
            *pos += 4;
            Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let version = u32_at(&mut pos)?;
        if version != VERSION {
            return Err(format!(
                "this program is version {version}; I understand version {VERSION}"
            ));
        }
        let entry = u32_at(&mut pos)? as u64;
        let text_len = u32_at(&mut pos)? as usize;
        let data_len = u32_at(&mut pos)? as usize;
        let bss = u32_at(&mut pos)?;
        let nlines = u32_at(&mut pos)? as usize;
        let nsyms = u32_at(&mut pos)? as usize;
        let src_len = u32_at(&mut pos)? as usize;
        let take = |pos: &mut usize, n: usize| -> Result<&[u8], String> {
            let b = bytes.get(*pos..*pos + n).ok_or("the file ends too early")?;
            *pos += n;
            Ok(b)
        };
        let text = take(&mut pos, text_len)?.to_vec();
        let data = take(&mut pos, data_len)?.to_vec();
        let mut lines = Vec::with_capacity(nlines);
        for _ in 0..nlines {
            let b = take(&mut pos, 8)?;
            lines.push((
                u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
            ));
        }
        let mut symbols = Vec::with_capacity(nsyms);
        for _ in 0..nsyms {
            let b = take(&mut pos, 6)?;
            let addr = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as u64;
            let len = u16::from_le_bytes([b[4], b[5]]) as usize;
            let name = String::from_utf8_lossy(take(&mut pos, len)?).into_owned();
            symbols.push((name, addr));
        }
        let source = String::from_utf8_lossy(take(&mut pos, src_len)?).into_owned();
        Ok(Image {
            text,
            data,
            bss,
            entry,
            lines,
            symbols,
            source,
        })
    }

    /// The source line (1-based) an address came from.
    pub fn line_of(&self, addr: u64) -> Option<u32> {
        self.lines.iter().find(|(a, _)| *a as u64 == addr).map(|(_, l)| *l)
    }

    /// The first address assembled from a source line.
    pub fn addr_of_line(&self, line: u32) -> Option<u64> {
        self.lines.iter().find(|(_, l)| *l == line).map(|(a, _)| *a as u64)
    }

    /// `addr` as a kid reads it: `0x10040` or `msg+2 (0x10042)`.
    pub fn name_of(&self, addr: u64) -> String {
        let mut best: Option<(&str, u64)> = None;
        for (name, a) in &self.symbols {
            if *a <= addr && addr - a < 4096 && best.is_none_or(|(_, ba)| *a > ba) {
                best = Some((name, *a));
            }
        }
        match best {
            Some((name, a)) if a == addr => format!("{name} (0x{addr:x})"),
            Some((name, a)) => format!("{name}+{} (0x{addr:x})", addr - a),
            None => format!("0x{addr:x}"),
        }
    }

    pub fn symbol(&self, name: &str) -> Option<u64> {
        self.symbols.iter().find(|(n, _)| n == name).map(|(_, a)| *a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let img = Image {
            text: vec![1, 2, 3, 4],
            data: b"hi".to_vec(),
            bss: 16,
            entry: 0x10000,
            lines: vec![(0x10000, 3)],
            symbols: vec![("_start".into(), 0x10000), ("msg".into(), 0x10010)],
            source: "// hello\n".into(),
        };
        let back = Image::from_bytes(&img.to_bytes()).unwrap();
        assert_eq!(back, img);
        assert_eq!(back.name_of(0x10012), "msg+2 (0x10012)");
        assert_eq!(back.name_of(0x30000), "0x30000");
    }
}
