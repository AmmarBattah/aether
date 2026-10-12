#[derive(Debug, Default)]
pub(super) struct Word {
    pub(super) text: String,
    pub(super) started: bool,
    pub(super) dynamic: bool,
    pub(super) quoted: bool,
}

impl Word {
    pub(super) fn as_str(&self) -> &str {
        &self.text
    }
}

#[derive(Debug)]
pub(super) enum Segment {
    Command(Vec<Word>),
    Boundary(Boundary),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Boundary {
    And,
    Ambiguous,
    Substitution,
    Pipe,
    Background,
    Open,
    Close,
    Sequence,
}

#[derive(Default)]
struct Lexer {
    segments: Vec<Segment>,
    words: Vec<Word>,
    word: Word,
    quote: Option<char>,
    redirect_target: bool,
    substitutions: Vec<String>,
}

/// Tokenize just enough shell syntax to keep redirects/comments out of argv.
pub(super) fn segments(command: &str) -> Vec<Segment> {
    let mut pending = vec![command.to_owned()];
    let mut result = Vec::new();
    while let Some(command) = pending.pop() {
        let lexer = tokenize(&command);
        // Substitutions execute in a subshell; their outer cwd may be dynamic.
        if !result.is_empty() {
            result.push(Segment::Boundary(Boundary::Substitution));
        }
        result.extend(lexer.segments);
        pending.extend(lexer.substitutions);
    }
    result
}

fn tokenize(command: &str) -> Lexer {
    let mut lexer = Lexer::default();
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'(') && lexer.quote != Some('\'') {
            lexer.substitution(&mut chars);
        } else if c == '\\' && lexer.quote != Some('\'') {
            lexer.escape(&mut chars);
        } else if matches!(c, '\'' | '"') && lexer.quote.is_none_or(|q| q == c) {
            lexer.word.started = true;
            lexer.word.quoted = true;
            lexer.quote = if lexer.quote.is_some() { None } else { Some(c) };
        } else if lexer.quote.is_some() {
            lexer.push(c);
        } else if c == '#' && !lexer.word.started {
            for c in chars.by_ref() {
                if c == '\n' {
                    break;
                }
            }
            lexer.end_segment(Boundary::Sequence);
        } else if matches!(c, '<' | '>') || (c == '&' && chars.peek() == Some(&'>')) {
            lexer.redirect(c, &mut chars);
        } else if matches!(c, '&' | '|') && chars.peek() == Some(&c) {
            chars.next();
            lexer.end_segment(if c == '&' {
                Boundary::And
            } else {
                Boundary::Ambiguous
            });
        } else if matches!(c, '&' | '|' | ';' | '\n' | '(' | ')') {
            lexer.end_segment(match c {
                '(' => Boundary::Open,
                ')' => Boundary::Close,
                '&' => Boundary::Background,
                '|' => Boundary::Pipe,
                _ => Boundary::Sequence,
            });
        } else if c.is_whitespace() {
            lexer.end_word();
        } else {
            lexer.push(c);
        }
    }
    lexer.end_segment(Boundary::Sequence);
    lexer
}

impl Lexer {
    fn substitution(&mut self, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
        chars.next(); // The opening parenthesis after `$`.
        let mut depth = 1;
        let mut quote = None;
        let mut body = String::new();
        while let Some(c) = chars.next() {
            if c == '\\' && quote != Some('\'') {
                body.push(c);
                if let Some(escaped) = chars.next() {
                    body.push(escaped);
                }
                continue;
            }
            match c {
                '\'' | '"' if quote == Some(c) => quote = None,
                '\'' | '"' if quote.is_none() => quote = Some(c),
                '(' if quote.is_none() => depth += 1,
                ')' if quote.is_none() => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                break;
            }
            body.push(c);
        }
        self.word.started = true;
        self.word.dynamic = true;
        self.word.text.push_str(&format!("$({body})"));
        self.substitutions.push(body);
    }

    fn push(&mut self, c: char) {
        self.word.started = true;
        self.word.dynamic |= self.quote != Some('\'') && matches!(c, '$' | '`');
        self.word.dynamic |= self.quote.is_none() && matches!(c, '~' | '*' | '?' | '[');
        self.word.text.push(c);
    }

    fn escape(&mut self, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
        if let Some(c) = chars.next()
            && c != '\n'
        {
            self.word.started = true;
            self.word.quoted = true;
            if self.quote == Some('"') && !matches!(c, '$' | '`' | '"' | '\\') {
                self.word.text.push('\\');
            }
            self.word.text.push(c);
        }
    }

    fn end_word(&mut self) {
        if self.word.started {
            let word = std::mem::take(&mut self.word);
            if !self.redirect_target {
                self.words.push(word);
            }
            self.redirect_target = false;
        }
    }

    fn end_segment(&mut self, separator: Boundary) {
        self.end_word();
        if !self.words.is_empty() {
            self.segments
                .push(Segment::Command(std::mem::take(&mut self.words)));
        }
        self.segments.push(Segment::Boundary(separator));
    }

    fn redirect(&mut self, c: char, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
        // An adjacent numeric word is the descriptor, not an argument.
        if !self.word.quoted && self.word.text.chars().all(|c| c.is_ascii_digit()) {
            self.word = Word::default();
        } else {
            self.end_word();
        }
        if c == '&' || chars.peek() == Some(&c) {
            chars.next();
        }
        if matches!(chars.peek(), Some('&' | '|')) {
            chars.next();
        }
        self.redirect_target = true;
    }
}
