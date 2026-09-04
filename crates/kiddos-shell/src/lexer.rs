//! Line → tokens.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seg {
    pub text: String,
    /// Came from inside quotes: never globbed, never tilde-expanded.
    pub quoted: bool,
    /// `$NAME` / `${NAME}` / `$?` / `$1`: expand at run time.
    pub var: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    pub segs: Vec<Seg>,
}

impl Word {
    pub fn lit(s: &str) -> Word {
        Word {
            segs: vec![Seg {
                text: s.to_string(),
                quoted: false,
                var: false,
            }],
        }
    }
    /// The text as typed, without quotes (for tab completion and messages).
    pub fn raw(&self) -> String {
        self.segs
            .iter()
            .map(|s| if s.var { format!("${}", s.text) } else { s.text.clone() })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Word(Word),
    Pipe,
    And,
    Or,
    Semi,
    /// `>` (fd 1) or `2>`
    RedirOut {
        fd: u8,
        append: bool,
    },
    RedirIn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexError {
    UnterminatedQuote(char),
    TrailingBackslash,
}

pub fn lex(line: &str) -> Result<Vec<Token>, LexError> {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut tokens = Vec::new();
    let mut segs: Vec<Seg> = Vec::new();
    let mut cur = String::new();

    fn flush_seg(segs: &mut Vec<Seg>, cur: &mut String, quoted: bool) {
        if !cur.is_empty() {
            segs.push(Seg {
                text: std::mem::take(cur),
                quoted,
                var: false,
            });
        }
    }
    fn flush_word(tokens: &mut Vec<Token>, segs: &mut Vec<Seg>) {
        if !segs.is_empty() {
            tokens.push(Token::Word(Word {
                segs: std::mem::take(segs),
            }));
        }
    }
    fn read_var(chars: &[char], i: &mut usize) -> Option<String> {
        // called with chars[*i] == '$'
        let start = *i + 1;
        let mut j = start;
        if j < chars.len() && chars[j] == '{' {
            let mut k = j + 1;
            while k < chars.len() && chars[k] != '}' {
                k += 1;
            }
            if k < chars.len() {
                let name: String = chars[j + 1..k].iter().collect();
                *i = k + 1;
                return Some(name);
            }
            return None;
        }
        if j < chars.len() && (chars[j] == '?' || chars[j] == '#' || chars[j] == '@' || chars[j] == '$') {
            *i = j + 1;
            return Some(chars[j].to_string());
        }
        while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
            j += 1;
        }
        if j == start {
            return None;
        }
        let name: String = chars[start..j].iter().collect();
        *i = j;
        Some(name)
    }

    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' => {
                flush_seg(&mut segs, &mut cur, false);
                flush_word(&mut tokens, &mut segs);
                i += 1;
            }
            '#' if cur.is_empty() && segs.is_empty() => break,
            '\'' => {
                flush_seg(&mut segs, &mut cur, false);
                let mut j = i + 1;
                let mut s = String::new();
                while j < chars.len() && chars[j] != '\'' {
                    s.push(chars[j]);
                    j += 1;
                }
                if j >= chars.len() {
                    return Err(LexError::UnterminatedQuote('\''));
                }
                segs.push(Seg {
                    text: s,
                    quoted: true,
                    var: false,
                });
                i = j + 1;
            }
            '"' => {
                flush_seg(&mut segs, &mut cur, false);
                let mut j = i + 1;
                let mut s = String::new();
                let mut closed = false;
                while j < chars.len() {
                    match chars[j] {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\\' if j + 1 < chars.len() && matches!(chars[j + 1], '"' | '\\' | '$') => {
                            s.push(chars[j + 1]);
                            j += 2;
                        }
                        '$' => {
                            let mut k = j;
                            if let Some(name) = read_var(&chars, &mut k) {
                                if !s.is_empty() {
                                    segs.push(Seg {
                                        text: std::mem::take(&mut s),
                                        quoted: true,
                                        var: false,
                                    });
                                }
                                segs.push(Seg {
                                    text: name,
                                    quoted: true,
                                    var: true,
                                });
                                j = k;
                            } else {
                                s.push('$');
                                j += 1;
                            }
                        }
                        ch => {
                            s.push(ch);
                            j += 1;
                        }
                    }
                }
                if !closed {
                    return Err(LexError::UnterminatedQuote('"'));
                }
                // an empty "" still produces a (possibly empty) word
                segs.push(Seg {
                    text: s,
                    quoted: true,
                    var: false,
                });
                i = j + 1;
            }
            '\\' => {
                if i + 1 >= chars.len() {
                    return Err(LexError::TrailingBackslash);
                }
                flush_seg(&mut segs, &mut cur, false);
                segs.push(Seg {
                    text: chars[i + 1].to_string(),
                    quoted: true,
                    var: false,
                });
                i += 2;
            }
            '$' => {
                let mut k = i;
                if let Some(name) = read_var(&chars, &mut k) {
                    flush_seg(&mut segs, &mut cur, false);
                    segs.push(Seg {
                        text: name,
                        quoted: false,
                        var: true,
                    });
                    i = k;
                } else {
                    cur.push('$');
                    i += 1;
                }
            }
            '|' | '&' | ';' | '>' | '<' => {
                // `2>` : a bare "2" right before '>'
                let fd2 = c == '>' && cur == "2" && segs.is_empty();
                if fd2 {
                    cur.clear();
                }
                flush_seg(&mut segs, &mut cur, false);
                flush_word(&mut tokens, &mut segs);
                let next = chars.get(i + 1).copied();
                match (c, next) {
                    ('|', Some('|')) => {
                        tokens.push(Token::Or);
                        i += 2;
                    }
                    ('|', _) => {
                        tokens.push(Token::Pipe);
                        i += 1;
                    }
                    ('&', Some('&')) => {
                        tokens.push(Token::And);
                        i += 2;
                    }
                    ('&', _) => {
                        // background jobs are not a thing here; treat as ';'
                        tokens.push(Token::Semi);
                        i += 1;
                    }
                    (';', _) => {
                        tokens.push(Token::Semi);
                        i += 1;
                    }
                    ('>', Some('>')) => {
                        tokens.push(Token::RedirOut {
                            fd: if fd2 { 2 } else { 1 },
                            append: true,
                        });
                        i += 2;
                    }
                    ('>', _) => {
                        tokens.push(Token::RedirOut {
                            fd: if fd2 { 2 } else { 1 },
                            append: false,
                        });
                        i += 1;
                    }
                    ('<', _) => {
                        tokens.push(Token::RedirIn);
                        i += 1;
                    }
                    _ => unreachable!(),
                }
            }
            c => {
                cur.push(c);
                i += 1;
            }
        }
    }
    flush_seg(&mut segs, &mut cur, false);
    flush_word(&mut tokens, &mut segs);
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        lex(line)
            .unwrap()
            .into_iter()
            .map(|t| match t {
                Token::Word(w) => w.raw(),
                other => format!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn basic_words() {
        assert_eq!(words("ls -l  a b"), vec!["ls", "-l", "a", "b"]);
        assert_eq!(
            words("echo 'hi there' \"you $NAME\""),
            vec!["echo", "hi there", "you $NAME"]
        );
        assert_eq!(words("echo a # comment"), vec!["echo", "a"]);
        assert_eq!(words("echo a#b"), vec!["echo", "a#b"]);
    }

    #[test]
    fn operators() {
        let t = lex("ls | grep x > out.txt 2>err && echo ok || echo no; cat < in").unwrap();
        assert!(matches!(t[1], Token::Pipe));
        assert!(matches!(t[4], Token::RedirOut { fd: 1, append: false }));
        assert!(matches!(t[6], Token::RedirOut { fd: 2, append: false }));
        assert!(matches!(t[8], Token::And));
        assert!(matches!(t[11], Token::Or));
        assert!(matches!(t[14], Token::Semi));
        assert!(matches!(t[16], Token::RedirIn));
        let t = lex("echo hi >> log").unwrap();
        assert!(matches!(t[2], Token::RedirOut { fd: 1, append: true }));
    }

    #[test]
    fn vars_and_quotes() {
        let t = lex("echo $HOME/x ${A}b '$C' \"$D e\" $?").unwrap();
        let Token::Word(w) = &t[1] else { panic!() };
        assert_eq!(w.segs.len(), 2);
        assert!(w.segs[0].var && w.segs[0].text == "HOME");
        assert_eq!(w.segs[1].text, "/x");
        let Token::Word(w) = &t[2] else { panic!() };
        assert!(w.segs[0].var && w.segs[0].text == "A");
        let Token::Word(w) = &t[3] else { panic!() };
        assert!(!w.segs[0].var && w.segs[0].quoted);
        let Token::Word(w) = &t[4] else { panic!() };
        assert!(w.segs[0].var && w.segs[0].quoted);
        assert_eq!(w.segs[1].text, " e");
        let Token::Word(w) = &t[5] else { panic!() };
        assert!(w.segs[0].var && w.segs[0].text == "?");
    }

    #[test]
    fn errors() {
        assert_eq!(lex("echo 'oops"), Err(LexError::UnterminatedQuote('\'')));
        assert_eq!(lex("echo \"oops"), Err(LexError::UnterminatedQuote('"')));
        assert_eq!(lex("echo x\\"), Err(LexError::TrailingBackslash));
        assert_eq!(words("echo a\\ b"), vec!["echo", "a b"]);
    }
}
