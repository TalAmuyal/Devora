//! Thin layer over `brush-parser`: enumerates every simple command that would execute (including those nested in substitutions) and the byte span of every word, so that decisions and redaction share one view of a command line.

use std::ops::Range;

use brush_parser::ast::{
    self, Command as AstCommand, CommandPrefixOrSuffixItem as Item, CompoundCommand,
};
use brush_parser::word::{self, Parameter, ParameterExpr, WordPiece, WordPieceWithSource};
use brush_parser::{Parser, ParserOptions};

const MAX_DEPTH: usize = 16;

/// The parser recurses once per nesting level and a stack overflow aborts the process, so deeper input is not parsed.
const MAX_NESTING: usize = 1000;
const MAX_SOURCE_LEN: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Arg {
    Literal(String),
    /// Depends on expansion at run time (variable, substitution, arithmetic).
    Dynamic,
}

impl Arg {
    pub(crate) fn literal(&self) -> Option<&str> {
        match self {
            Arg::Literal(text) => Some(text),
            Arg::Dynamic => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum Part {
    Literal {
        span: Range<usize>,
        text: String,
    },
    Variable {
        span: Range<usize>,
        name: Option<String>,
    },
    /// Substitutions and arithmetic; the commands inside are reported as their own words.
    Nested,
}

#[derive(Debug)]
pub(crate) struct WordInfo {
    pub span: Range<usize>,
    pub parts: Vec<Part>,
}

impl WordInfo {
    pub(crate) fn arg(&self) -> Arg {
        let mut text = String::new();
        for part in &self.parts {
            match part {
                Part::Literal { text: piece, .. } => text.push_str(piece),
                Part::Variable { .. } | Part::Nested => return Arg::Dynamic,
            }
        }
        Arg::Literal(text)
    }
}

#[derive(Debug)]
pub(crate) struct Assignment {
    pub name: String,
    pub value: Arg,
}

#[derive(Debug, Default)]
pub(crate) struct Command {
    /// The leading `VAR=value` assignments.
    pub assigned: Vec<Assignment>,
    /// The command name followed by its arguments.
    pub words: Vec<WordInfo>,
}

impl Command {
    pub(crate) fn argv(&self) -> Vec<Arg> {
        self.words.iter().map(WordInfo::arg).collect()
    }
}

#[derive(Debug, Default)]
pub(crate) struct Parsed {
    pub commands: Vec<Command>,
    /// Words that are not part of a command line (assignment values, redirect targets, loop values, ...).
    pub other_words: Vec<WordInfo>,
    /// Constructs we deliberately do not analyze (function definitions, expansions hiding substitutions, ...).
    pub opaque: Vec<String>,
    /// Some word could not be mapped back onto the source, so span-based rewriting would be unsafe.
    pub unspanned: bool,
}

/// Whether brackets and block keywords nest deeper than the limits, wherever they appear (over-counting only costs a gap).
/// A closer pops only its own opener, so `case` patterns (`x)`) and stray closers do not hide depth.
fn nesting_exceeds_limit(src: &str) -> bool {
    let mut open: Vec<char> = Vec::new();
    let mut word = String::new();
    let close = |open: &mut Vec<char>, opener: char| {
        if open.last() == Some(&opener) {
            open.pop();
        }
    };
    for c in src.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_alphabetic() || c == '_' {
            word.push(c);
            continue;
        }
        match word.as_str() {
            "if" => open.push('i'),
            "case" => open.push('c'),
            "do" => open.push('d'),
            "coproc" => open.push('p'),
            "fi" => close(&mut open, 'i'),
            "esac" => close(&mut open, 'c'),
            "done" => close(&mut open, 'd'),
            _ => {}
        }
        word.clear();
        match c {
            '(' | '{' | '[' => open.push(c),
            ')' => close(&mut open, '('),
            '}' => close(&mut open, '{'),
            ']' => close(&mut open, '['),
            _ => {}
        }
        if open.len() > MAX_NESTING {
            return true;
        }
    }
    false
}

pub(crate) fn parse(src: &str) -> Result<Parsed, String> {
    if src.len() > MAX_SOURCE_LEN || nesting_exceeds_limit(src) {
        return Err("nesting or size beyond what is analyzed".into());
    }
    let mut parsed = Parsed::default();
    Walker {
        out: &mut parsed,
        source: String::new(),
    }
    .program(src, 0, 0)?;
    Ok(parsed)
}

struct Walker<'a> {
    out: &'a mut Parsed,
    /// The program being walked; spans are offsets into it (plus the `base` of nested programs).
    source: String,
}

impl Walker<'_> {
    fn program(&mut self, src: &str, base: usize, depth: usize) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Err("nesting too deep".into());
        }
        let options = ParserOptions::default();
        let program = Parser::new(std::io::Cursor::new(src), &options)
            .parse_program()
            .map_err(|error| error.to_string())?;
        let outer = std::mem::replace(&mut self.source, src.to_string());
        let walked = program
            .complete_commands
            .iter()
            .try_for_each(|list| self.list(list, base, depth));
        self.source = outer;
        walked
    }

    fn list(&mut self, list: &ast::CompoundList, base: usize, depth: usize) -> Result<(), String> {
        for ast::CompoundListItem(and_or, _) in &list.0 {
            let pipelines = std::iter::once(&and_or.first).chain(and_or.additional.iter().map(
                |next| match next {
                    ast::AndOr::And(pipeline) | ast::AndOr::Or(pipeline) => pipeline,
                },
            ));
            for pipeline in pipelines {
                for command in &pipeline.seq {
                    self.command(command, base, depth)?;
                }
            }
        }
        Ok(())
    }

    fn command(&mut self, command: &AstCommand, base: usize, depth: usize) -> Result<(), String> {
        match command {
            AstCommand::Simple(simple) => self.simple(simple, base, depth),
            AstCommand::Compound(compound, redirects) => {
                self.compound(compound, base, depth)?;
                self.redirects(redirects.as_ref(), base, depth)
            }
            AstCommand::Function(function) => {
                self.out.opaque.push("function definition".into());
                let ast::FunctionBody(body, redirects) = &function.body;
                self.compound(body, base, depth)?;
                self.redirects(redirects.as_ref(), base, depth)
            }
            AstCommand::ExtendedTest(test, redirects) => {
                self.extended_test(&test.expr, base, depth)?;
                self.redirects(redirects.as_ref(), base, depth)
            }
        }
    }

    fn compound(
        &mut self,
        compound: &CompoundCommand,
        base: usize,
        depth: usize,
    ) -> Result<(), String> {
        match compound {
            CompoundCommand::Arithmetic(arithmetic) => self.arithmetic(&arithmetic.expr, depth),
            CompoundCommand::ArithmeticForClause(clause) => {
                for expr in [&clause.initializer, &clause.condition, &clause.updater]
                    .into_iter()
                    .flatten()
                {
                    self.arithmetic(expr, depth)?;
                }
                self.list(&clause.body.list, base, depth)
            }
            CompoundCommand::BraceGroup(group) => self.list(&group.list, base, depth),
            CompoundCommand::Subshell(subshell) => self.list(&subshell.list, base, depth),
            CompoundCommand::ForClause(clause) => {
                for value in clause.values.iter().flatten() {
                    self.other_word(value, base, depth)?;
                }
                self.list(&clause.body.list, base, depth)
            }
            CompoundCommand::CaseClause(clause) => {
                self.other_word(&clause.value, base, depth)?;
                for item in &clause.cases {
                    for pattern in &item.patterns {
                        self.other_word(pattern, base, depth)?;
                    }
                    if let Some(body) = &item.cmd {
                        self.list(body, base, depth)?;
                    }
                }
                Ok(())
            }
            CompoundCommand::IfClause(clause) => {
                self.list(&clause.condition, base, depth)?;
                self.list(&clause.then, base, depth)?;
                for branch in clause.elses.iter().flatten() {
                    if let Some(condition) = &branch.condition {
                        self.list(condition, base, depth)?;
                    }
                    self.list(&branch.body, base, depth)?;
                }
                Ok(())
            }
            CompoundCommand::WhileClause(clause) | CompoundCommand::UntilClause(clause) => {
                self.list(&clause.0, base, depth)?;
                self.list(&clause.1.list, base, depth)
            }
            CompoundCommand::Coprocess(coprocess) => {
                self.out.opaque.push("coprocess".into());
                self.command(&coprocess.body, base, depth)
            }
        }
    }

    fn extended_test(
        &mut self,
        expr: &ast::ExtendedTestExpr,
        base: usize,
        depth: usize,
    ) -> Result<(), String> {
        use ast::ExtendedTestExpr as Expr;
        match expr {
            Expr::And(left, right) | Expr::Or(left, right) => {
                self.extended_test(left, base, depth)?;
                self.extended_test(right, base, depth)
            }
            Expr::Not(inner) | Expr::Parenthesized(inner) => self.extended_test(inner, base, depth),
            Expr::UnaryTest(_, operand) => self.other_word(operand, base, depth),
            Expr::BinaryTest(_, left, right) => {
                self.other_word(left, base, depth)?;
                self.other_word(right, base, depth)
            }
        }
    }

    /// A standalone `(( ))`; it has no span, so commands hiding in it cannot be redacted in place.
    fn arithmetic(
        &mut self,
        expr: &ast::UnexpandedArithmeticExpr,
        depth: usize,
    ) -> Result<(), String> {
        if hides_substitution(&expr.value) {
            self.out.unspanned = true;
            let pieces = word::parse(&expr.value, &ParserOptions::default())
                .map_err(|error| error.to_string())?;
            self.pieces(&expr.value, &pieces, 0, depth, &mut Vec::new())?;
        }
        Ok(())
    }

    fn simple(
        &mut self,
        simple: &ast::SimpleCommand,
        base: usize,
        depth: usize,
    ) -> Result<(), String> {
        let mut command = Command::default();
        for item in simple.prefix.iter().flat_map(|prefix| &prefix.0) {
            match item {
                Item::AssignmentWord(assignment, word) => {
                    let value = self.word(word, base, depth)?;
                    let name = assignment_name(assignment);
                    // The word is the whole `NAME=value`; anything but a plain `=` (`+=`) has no usable value.
                    let literal = value.arg();
                    command.assigned.push(Assignment {
                        value: match literal
                            .literal()
                            .and_then(|text| text.strip_prefix(&name)?.strip_prefix('='))
                        {
                            Some(text) => Arg::Literal(text.to_string()),
                            None => Arg::Dynamic,
                        },
                        name,
                    });
                    self.out.other_words.push(value);
                }
                other => self.item(other, &mut command, base, depth)?,
            }
        }
        if let Some(name) = &simple.word_or_name {
            command.words.push(self.word(name, base, depth)?);
        }
        for item in simple.suffix.iter().flat_map(|suffix| &suffix.0) {
            self.item(item, &mut command, base, depth)?;
        }
        self.out.commands.push(command);
        Ok(())
    }

    fn item(
        &mut self,
        item: &Item,
        command: &mut Command,
        base: usize,
        depth: usize,
    ) -> Result<(), String> {
        match item {
            Item::Word(word) => command.words.push(self.word(word, base, depth)?),
            Item::IoRedirect(redirect) => self.redirect(redirect, base, depth)?,
            Item::ProcessSubstitution(_, subshell) => self.list(&subshell.list, base, depth)?,
            // `export X=1` style arguments.
            Item::AssignmentWord(_, word) => command.words.push(self.word(word, base, depth)?),
        }
        Ok(())
    }

    fn redirects(
        &mut self,
        redirects: Option<&ast::RedirectList>,
        base: usize,
        depth: usize,
    ) -> Result<(), String> {
        for redirect in redirects.iter().flat_map(|list| &list.0) {
            self.redirect(redirect, base, depth)?;
        }
        Ok(())
    }

    fn redirect(
        &mut self,
        redirect: &ast::IoRedirect,
        base: usize,
        depth: usize,
    ) -> Result<(), String> {
        use ast::{IoFileRedirectTarget as Target, IoRedirect};
        match redirect {
            IoRedirect::File(_, _, Target::Filename(word) | Target::Duplicate(word)) => {
                self.other_word(word, base, depth)
            }
            IoRedirect::File(_, _, Target::ProcessSubstitution(_, subshell)) => {
                self.list(&subshell.list, base, depth)
            }
            IoRedirect::File(_, _, Target::Fd(_)) => Ok(()),
            IoRedirect::HereString(_, word) | IoRedirect::OutputAndError(word, _) => {
                self.other_word(word, base, depth)
            }
            IoRedirect::HereDocument(_, heredoc) => {
                self.heredoc(&heredoc.doc, heredoc.requires_expansion, base, depth)
            }
        }
    }

    fn heredoc(
        &mut self,
        doc: &ast::Word,
        expands: bool,
        base: usize,
        depth: usize,
    ) -> Result<(), String> {
        let Some(span) = self.span_of(doc, base) else {
            return Ok(());
        };
        let span = span.start..span.start + doc.value.len();
        let mut parts = Vec::new();
        if expands {
            let pieces = word::parse_heredoc(&doc.value, &ParserOptions::default())
                .map_err(|error| error.to_string())?;
            self.pieces(&doc.value, &pieces, span.start, depth, &mut parts)?;
        } else {
            parts.push(Part::Literal {
                span: span.clone(),
                text: doc.value.clone(),
            });
        }
        self.out.other_words.push(WordInfo { span, parts });
        Ok(())
    }

    /// The word's absolute span, if the parser's location really points at its text.
    fn span_of(&mut self, word: &ast::Word, base: usize) -> Option<Range<usize>> {
        let loc = word.loc.as_ref()?;
        let local = loc.start.index..loc.start.index + word.value.len();
        if self.source.get(local.clone()) == Some(word.value.as_str()) {
            Some(base + local.start..base + local.end)
        } else {
            self.out.unspanned = true;
            None
        }
    }

    fn other_word(&mut self, word: &ast::Word, base: usize, depth: usize) -> Result<(), String> {
        let info = self.word(word, base, depth)?;
        self.out.other_words.push(info);
        Ok(())
    }

    fn word(&mut self, word: &ast::Word, base: usize, depth: usize) -> Result<WordInfo, String> {
        let span = self.span_of(word, base).unwrap_or(0..0);
        let pieces = word::parse(&word.value, &ParserOptions::default())
            .map_err(|error| error.to_string())?;
        let mut parts = Vec::new();
        self.pieces(&word.value, &pieces, span.start, depth, &mut parts)?;
        Ok(WordInfo { span, parts })
    }

    /// `origin` is the absolute offset of `text` in the top-level source.
    fn pieces(
        &mut self,
        text: &str,
        pieces: &[WordPieceWithSource],
        origin: usize,
        depth: usize,
        parts: &mut Vec<Part>,
    ) -> Result<(), String> {
        for WordPieceWithSource {
            piece,
            start_index,
            end_index,
        } in pieces
        {
            let span = origin + start_index..origin + end_index;
            let raw = text.get(*start_index..*end_index).unwrap_or_default();
            match piece {
                WordPiece::Text(content) => parts.push(Part::Literal {
                    span,
                    text: content.clone(),
                }),
                WordPiece::SingleQuotedText(content) | WordPiece::AnsiCQuotedText(content) => {
                    parts.push(Part::Literal {
                        span,
                        text: content.clone(),
                    });
                }
                WordPiece::EscapeSequence(sequence) => {
                    // Kept out of redaction: the backslash belongs to the surrounding syntax.
                    let decoded = sequence.strip_prefix('\\').unwrap_or(sequence);
                    parts.push(Part::Literal {
                        span: origin + start_index..origin + start_index,
                        text: decoded.into(),
                    });
                }
                WordPiece::TildeExpansion(_) => parts.push(Part::Literal {
                    span: span.start..span.start,
                    text: raw.into(),
                }),
                WordPiece::DoubleQuotedSequence(inner)
                | WordPiece::GettextDoubleQuotedSequence(inner) => {
                    self.pieces(text, inner, origin, depth, parts)?;
                }
                WordPiece::ParameterExpansion(expr) => {
                    if hides_substitution(raw) {
                        self.out
                            .opaque
                            .push("substitution inside a parameter expansion".into());
                    }
                    parts.push(Part::Variable {
                        span,
                        name: simple_variable(expr),
                    });
                }
                WordPiece::CommandSubstitution(inner) => {
                    self.substitution(raw, inner, 2, span.start, depth)?;
                    parts.push(Part::Nested);
                }
                WordPiece::BackquotedCommandSubstitution(inner) => {
                    self.substitution(raw, inner, 1, span.start, depth)?;
                    parts.push(Part::Nested);
                }
                WordPiece::ArithmeticExpression(expr) => {
                    if hides_substitution(&expr.value) {
                        let verbatim =
                            raw.get(3..raw.len().saturating_sub(2)) == Some(expr.value.as_str());
                        let inner = word::parse(&expr.value, &ParserOptions::default())
                            .map_err(|error| error.to_string())?;
                        self.pieces(
                            &expr.value,
                            &inner,
                            if verbatim { span.start + 3 } else { 0 },
                            depth + 1,
                            &mut Vec::new(),
                        )?;
                        self.out.unspanned |= !verbatim;
                    }
                    parts.push(Part::Nested);
                }
            }
        }
        Ok(())
    }

    fn substitution(
        &mut self,
        raw: &str,
        inner: &str,
        open_len: usize,
        start: usize,
        depth: usize,
    ) -> Result<(), String> {
        // Backquotes unescape their content, which shifts offsets; then the inner commands cannot be redacted in place.
        let verbatim = raw.get(open_len..raw.len().saturating_sub(1)) == Some(inner);
        if !verbatim {
            self.out.unspanned = true;
        }
        self.program(
            inner,
            if verbatim { start + open_len } else { 0 },
            depth + 1,
        )
    }
}

fn assignment_name(assignment: &ast::Assignment) -> String {
    let (ast::AssignmentName::VariableName(name) | ast::AssignmentName::ArrayElementName(name, _)) =
        &assignment.name;
    name.clone()
}

fn simple_variable(expr: &ParameterExpr) -> Option<String> {
    match expr {
        ParameterExpr::Parameter {
            parameter: Parameter::Named(name),
            indirect: false,
        } => Some(name.clone()),
        _ => None,
    }
}

/// Parameter expansions can carry words (`${x:-$(cmd)}`) that the word parser leaves as raw text.
fn hides_substitution(raw: &str) -> bool {
    ["$(", "`", "<(", ">("]
        .iter()
        .any(|marker| raw.contains(marker))
}
