//! Tokens → a list of pipelines joined by `;`, `&&`, `||`.

use crate::lexer::{lex, LexError, Token, Word};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Redirect {
    Out { fd: u8, target: Word, append: bool },
    In { source: Word },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SimpleCmd {
    pub words: Vec<Word>,
    pub redirects: Vec<Redirect>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pipeline {
    pub cmds: Vec<SimpleCmd>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connector {
    /// run unconditionally (`;` or end)
    Always,
    /// run if the previous succeeded (`&&`)
    IfOk,
    /// run if the previous failed (`||`)
    IfFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct List {
    /// Each pipeline with the connector that decides whether the *next* one runs.
    pub items: Vec<(Pipeline, Connector)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Lex(LexError),
    /// An operator with nothing before/after it, e.g. `| grep`.
    EmptyCommand(String),
    /// A redirect without a file name.
    RedirectNeedsFile,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Lex(LexError::UnterminatedQuote(c)) => {
                write!(f, "I see a quote mark {c} that never closes.")
            }
            ParseError::Lex(LexError::TrailingBackslash) => {
                write!(
                    f,
                    "The line ends with a backslash. Add the character it should protect."
                )
            }
            ParseError::EmptyCommand(op) => write!(f, "There is nothing for {op} to connect to."),
            ParseError::RedirectNeedsFile => write!(f, "> needs a file name after it, like: echo hi > note.txt"),
        }
    }
}

pub fn parse(line: &str) -> Result<List, ParseError> {
    let tokens = lex(line).map_err(ParseError::Lex)?;
    let mut list = List::default();
    let mut pipeline = Pipeline::default();
    let mut cmd = SimpleCmd::default();
    let mut i = 0;

    fn end_cmd(pipeline: &mut Pipeline, cmd: &mut SimpleCmd, op: &str) -> Result<(), ParseError> {
        if cmd.words.is_empty() && cmd.redirects.is_empty() {
            return Err(ParseError::EmptyCommand(op.to_string()));
        }
        pipeline.cmds.push(std::mem::take(cmd));
        Ok(())
    }

    while i < tokens.len() {
        match &tokens[i] {
            Token::Word(w) => cmd.words.push(w.clone()),
            Token::RedirOut { fd, append } => {
                i += 1;
                match tokens.get(i) {
                    Some(Token::Word(w)) => cmd.redirects.push(Redirect::Out {
                        fd: *fd,
                        target: w.clone(),
                        append: *append,
                    }),
                    _ => return Err(ParseError::RedirectNeedsFile),
                }
            }
            Token::RedirIn => {
                i += 1;
                match tokens.get(i) {
                    Some(Token::Word(w)) => cmd.redirects.push(Redirect::In { source: w.clone() }),
                    _ => return Err(ParseError::RedirectNeedsFile),
                }
            }
            Token::Pipe => end_cmd(&mut pipeline, &mut cmd, "|")?,
            Token::And | Token::Or | Token::Semi => {
                let (op, conn) = match &tokens[i] {
                    Token::And => ("&&", Connector::IfOk),
                    Token::Or => ("||", Connector::IfFailed),
                    _ => (";", Connector::Always),
                };
                if cmd.words.is_empty() && cmd.redirects.is_empty() && pipeline.cmds.is_empty() {
                    if conn == Connector::Always {
                        i += 1;
                        continue; // stray ';' is harmless
                    }
                    return Err(ParseError::EmptyCommand(op.to_string()));
                }
                end_cmd(&mut pipeline, &mut cmd, op)?;
                list.items.push((std::mem::take(&mut pipeline), conn));
            }
        }
        i += 1;
    }
    if !cmd.words.is_empty() || !cmd.redirects.is_empty() {
        pipeline.cmds.push(cmd);
    } else if !pipeline.cmds.is_empty() {
        return Err(ParseError::EmptyCommand("|".into()));
    }
    if !pipeline.cmds.is_empty() {
        list.items.push((pipeline, Connector::Always));
    } else if let Some((_, conn)) = list.items.last() {
        if *conn != Connector::Always {
            let op = if *conn == Connector::IfOk { "&&" } else { "||" };
            return Err(ParseError::EmptyCommand(op.into()));
        }
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pipeline_and_list() {
        let l = parse("ls -l | grep x > out ; echo done && echo ok").unwrap();
        assert_eq!(l.items.len(), 3);
        assert_eq!(l.items[0].0.cmds.len(), 2);
        assert_eq!(l.items[0].0.cmds[1].words[0].raw(), "grep");
        assert_eq!(l.items[0].0.cmds[1].redirects.len(), 1);
        assert_eq!(l.items[0].1, Connector::Always);
        assert_eq!(l.items[1].1, Connector::IfOk);
    }

    #[test]
    fn errors() {
        assert_eq!(parse("| grep"), Err(ParseError::EmptyCommand("|".into())));
        assert_eq!(parse("ls |"), Err(ParseError::EmptyCommand("|".into())));
        assert_eq!(parse("ls &&"), Err(ParseError::EmptyCommand("&&".into())));
        assert_eq!(parse("ls >"), Err(ParseError::RedirectNeedsFile));
        assert!(parse("").unwrap().items.is_empty());
        assert!(parse("   # just a comment").unwrap().items.is_empty());
        assert_eq!(parse("ls;").unwrap().items.len(), 1);
    }
}
