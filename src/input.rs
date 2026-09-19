//! Shared terminal editing and history for local and attached REPLs.

use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
};

use rustyline::{DefaultEditor, error::ReadlineError};

pub(crate) trait Input {
    fn line(&mut self, prompt: &str, output: &mut impl Write) -> io::Result<Option<String>>;

    fn remember(&mut self, _source: &str) {}
}

impl<R: BufRead> Input for R {
    fn line(&mut self, prompt: &str, output: &mut impl Write) -> io::Result<Option<String>> {
        write!(output, "{prompt}")?;
        output.flush()?;
        self.lines().next().transpose()
    }
}

pub(crate) struct Terminal {
    editor: DefaultEditor,
    history_path: Option<PathBuf>,
}

impl Terminal {
    pub(crate) fn new() -> io::Result<Self> {
        Self::with_history(std::env::home_dir().map(|home| home.join(".pliq_history")))
    }

    pub(crate) fn with_history(history_path: Option<PathBuf>) -> io::Result<Self> {
        let mut editor = DefaultEditor::new().map_err(io::Error::other)?;
        if let Some(path) = &history_path
            && let Err(error) = editor.load_history(path)
            && !matches!(&error, ReadlineError::Io(e) if e.kind() == io::ErrorKind::NotFound)
        {
            eprintln!("warning: cannot load REPL history: {error}");
        }
        Ok(Self {
            editor,
            history_path,
        })
    }
}

impl Input for Terminal {
    fn line(&mut self, prompt: &str, output: &mut impl Write) -> io::Result<Option<String>> {
        output.flush()?;
        match self.editor.readline(prompt) {
            Ok(line) => Ok(Some(line)),
            Err(ReadlineError::Eof) => Ok(None),
            Err(ReadlineError::Interrupted) => Ok(Some("\\c".into())),
            Err(error) => Err(io::Error::other(error)),
        }
    }

    fn remember(&mut self, source: &str) {
        let source = source.trim_end_matches('\n');
        if source.trim().is_empty() {
            return;
        }
        let result = self.editor.add_history_entry(source).and_then(|added| {
            if added && let Some(path) = &self.history_path {
                self.editor.append_history(path)?;
            }
            Ok(())
        });
        if let Err(error) = result {
            eprintln!("warning: cannot save REPL history: {error}");
        }
    }
}
