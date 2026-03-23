use crate::shell::{
    agent_apply, agent_preview, copy_workspace_path, dependencies_report, doctor_report,
    documentation_lines, find_workspace_paths, install_package, list_directory, list_modules,
    list_packages, manifest_snapshot, outline_workspace_file, parse_shell_command,
    permissions_report, read_workspace_file, remove_package, render_tree, scaffold_module,
    scaffold_snippet, search_workspace_text, set_permission, shell_help_lines, shell_tool_lines,
    snippet_catalog, terminal_status_report, validate_host_shell_command, ShellCommand,
};
use crate::{format_diagnostics, security_policy_for_entry};
use anyhow::{bail, Context, Result};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::Print,
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use snsx_compiler::security::SecurityPolicy;
use snsx_compiler::{
    audit_source, compile_source_with_policy, compile_vm_with_policy, write_artifact, Artifact,
    CompileOptions, Target,
};
use snsx_vm::{InputRequest, VirtualMachine, VmOptions, VmTrap};
use std::env;
use std::fs;
use std::io::{stdout, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Tree,
    Editor,
    Input,
    Output,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputTab {
    Run,
    Build,
    Security,
    Terminal,
}

impl OutputTab {
    fn title(self) -> &'static str {
        match self {
            OutputTab::Run => "Run",
            OutputTab::Build => "Build",
            OutputTab::Security => "Security",
            OutputTab::Terminal => "Terminal",
        }
    }

    fn next(self) -> Self {
        match self {
            OutputTab::Run => OutputTab::Build,
            OutputTab::Build => OutputTab::Security,
            OutputTab::Security => OutputTab::Terminal,
            OutputTab::Terminal => OutputTab::Run,
        }
    }

    fn previous(self) -> Self {
        match self {
            OutputTab::Run => OutputTab::Terminal,
            OutputTab::Build => OutputTab::Run,
            OutputTab::Security => OutputTab::Build,
            OutputTab::Terminal => OutputTab::Security,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptKind {
    NewFile,
    NewFolder,
    Rename,
    Delete,
}

#[derive(Debug, Clone)]
struct PromptState {
    kind: PromptKind,
    input: String,
    target: Option<PathBuf>,
    label: String,
}

#[derive(Debug, Clone, Copy)]
struct InteractiveRunSession {
    line_index: usize,
}

#[derive(Debug, Clone)]
struct ScreenFrame {
    lines: Vec<String>,
    cursor: Option<(u16, u16)>,
}

const TEXT_BUFFER_GUTTER_WIDTH: usize = 5;
#[derive(Debug, Clone)]
struct TreeEntry {
    path: PathBuf,
    depth: usize,
    is_dir: bool,
}

#[derive(Debug, Clone)]
struct TextBuffer {
    lines: Vec<String>,
    cursor_x: usize,
    cursor_y: usize,
    scroll: usize,
    hscroll: usize,
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_x: 0,
            cursor_y: 0,
            scroll: 0,
            hscroll: 0,
        }
    }
}

impl TextBuffer {
    fn from_text(text: &str) -> Self {
        let mut lines = if text.is_empty() {
            vec![String::new()]
        } else {
            text.lines().map(ToString::to_string).collect::<Vec<_>>()
        };
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self {
            lines,
            cursor_x: 0,
            cursor_y: 0,
            scroll: 0,
            hscroll: 0,
        }
    }

    fn to_text(&self) -> String {
        self.lines.join("\n")
    }

    fn set_text(&mut self, text: &str) {
        *self = Self::from_text(text);
    }

    fn ensure_nonempty(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
    }

    fn clamp_cursor(&mut self) {
        self.ensure_nonempty();
        if self.cursor_y >= self.lines.len() {
            self.cursor_y = self.lines.len().saturating_sub(1);
        }
        let line_len = text_char_len(&self.lines[self.cursor_y]);
        if self.cursor_x > line_len {
            self.cursor_x = line_len;
        }
    }

    fn ensure_visible(&mut self, visible_height: usize, visible_width: usize) {
        let visible_height = visible_height.max(1);
        let visible_width = visible_width.max(TEXT_BUFFER_GUTTER_WIDTH + 1);
        let content_width = visible_width
            .saturating_sub(TEXT_BUFFER_GUTTER_WIDTH)
            .max(1);
        self.clamp_cursor();
        if self.cursor_y < self.scroll {
            self.scroll = self.cursor_y;
        }
        if self.cursor_y >= self.scroll + visible_height {
            self.scroll = self
                .cursor_y
                .saturating_sub(visible_height.saturating_sub(1));
        }
        if self.cursor_x < self.hscroll {
            self.hscroll = self.cursor_x;
        }
        if self.cursor_x >= self.hscroll + content_width {
            self.hscroll = self
                .cursor_x
                .saturating_sub(content_width.saturating_sub(1));
        }
    }

    fn move_up(&mut self) {
        self.cursor_y = self.cursor_y.saturating_sub(1);
        self.clamp_cursor();
    }

    fn move_down(&mut self) {
        if self.cursor_y + 1 < self.lines.len() {
            self.cursor_y += 1;
        }
        self.clamp_cursor();
    }

    fn move_left(&mut self) {
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.cursor_x = text_char_len(&self.lines[self.cursor_y]);
        }
    }

    fn move_right(&mut self) {
        let line_len = self
            .lines
            .get(self.cursor_y)
            .map(|line| text_char_len(line))
            .unwrap_or(0);
        if self.cursor_x < line_len {
            self.cursor_x += 1;
        } else if self.cursor_y + 1 < self.lines.len() {
            self.cursor_y += 1;
            self.cursor_x = 0;
        }
    }

    fn move_home(&mut self) {
        self.cursor_x = 0;
    }

    fn move_end(&mut self) {
        self.cursor_x = self
            .lines
            .get(self.cursor_y)
            .map(|line| text_char_len(line))
            .unwrap_or(0);
    }

    fn insert_char(&mut self, ch: char) {
        self.ensure_nonempty();
        if let Some(line) = self.lines.get_mut(self.cursor_y) {
            insert_char_at(line, self.cursor_x, ch);
            self.cursor_x += 1;
        }
    }

    fn insert_str(&mut self, text: &str) {
        for ch in text.chars() {
            self.insert_char(ch);
        }
    }

    fn insert_newline(&mut self) {
        self.ensure_nonempty();
        let split_at = byte_index_for_char(&self.lines[self.cursor_y], self.cursor_x);
        let tail = self.lines[self.cursor_y].split_off(split_at);
        self.cursor_y += 1;
        self.cursor_x = 0;
        self.hscroll = 0;
        self.lines.insert(self.cursor_y, tail);
    }

    fn backspace(&mut self) -> bool {
        self.ensure_nonempty();
        if self.cursor_x > 0 {
            remove_char_at(&mut self.lines[self.cursor_y], self.cursor_x - 1);
            self.cursor_x -= 1;
            return true;
        }
        if self.cursor_y > 0 {
            let removed = self.lines.remove(self.cursor_y);
            self.cursor_y -= 1;
            self.cursor_x = text_char_len(&self.lines[self.cursor_y]);
            self.lines[self.cursor_y].push_str(&removed);
            return true;
        }
        false
    }

    fn delete_forward(&mut self) -> bool {
        self.ensure_nonempty();
        if self.cursor_x < text_char_len(&self.lines[self.cursor_y]) {
            remove_char_at(&mut self.lines[self.cursor_y], self.cursor_x);
            return true;
        }
        if self.cursor_y + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor_y + 1);
            self.lines[self.cursor_y].push_str(&next);
            return true;
        }
        false
    }
}

#[derive(Debug, Clone)]
struct LogBuffer {
    lines: Vec<String>,
    scroll: usize,
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            scroll: 0,
        }
    }
}

impl LogBuffer {
    fn with_lines(lines: Vec<String>) -> Self {
        let mut buffer = Self::default();
        buffer.set_lines(lines);
        buffer
    }

    fn set_lines(&mut self, lines: Vec<String>) {
        self.lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        self.scroll = 0;
    }

    fn append_lines<I>(&mut self, lines: I)
    where
        I: IntoIterator<Item = String>,
    {
        if self.lines.len() == 1 && self.lines[0].is_empty() {
            self.lines.clear();
        }
        self.lines.extend(lines);
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.scroll = self.lines.len().saturating_sub(1);
    }

    fn append_text(&mut self, text: &str) {
        let lines = if text.trim().is_empty() {
            vec![String::new()]
        } else {
            text.lines().map(ToString::to_string).collect::<Vec<_>>()
        };
        self.append_lines(lines);
    }

    fn clear(&mut self, message: &str) {
        self.lines = vec![message.to_string()];
        self.scroll = 0;
    }

    fn scroll_up(&mut self, amount: usize) {
        self.scroll = self.scroll.saturating_sub(amount);
    }

    fn scroll_down(&mut self, amount: usize) {
        let max_scroll = self.lines.len().saturating_sub(1);
        self.scroll = (self.scroll + amount).min(max_scroll);
    }
}

#[derive(Debug, Clone, Copy)]
struct StudioLayout {
    width: usize,
    height: usize,
    tree_width: usize,
    editor_width: usize,
    output_width: usize,
    main_body_height: usize,
    bottom_body_height: usize,
    input_width: usize,
    command_width: usize,
}

impl StudioLayout {
    fn current() -> Self {
        let (width, height) = terminal::size().unwrap_or((160, 40));
        let width = width as usize;
        let height = height as usize;
        let tree_width = width.saturating_div(5).clamp(18, 32);
        let output_width = width.saturating_div(3).clamp(28, 48);
        let editor_width = width
            .saturating_sub(tree_width)
            .saturating_sub(output_width)
            .saturating_sub(6)
            .max(26);
        let bottom_body_height = height.saturating_div(7).clamp(5, 8);
        let reserved = bottom_body_height + 5;
        let main_body_height = height.saturating_sub(reserved).max(8);
        let available_bottom = width.saturating_sub(3);
        let min_input = 30;
        let min_command = 30;
        let (input_width, command_width) = if available_bottom >= min_input + min_command {
            let command_width = available_bottom
                .saturating_mul(2)
                .saturating_div(5)
                .clamp(34, 54);
            let input_width = available_bottom.saturating_sub(command_width);
            (input_width, command_width)
        } else {
            let input_width = available_bottom.saturating_mul(3).saturating_div(5);
            let command_width = available_bottom.saturating_sub(input_width);
            (input_width.max(1), command_width.max(1))
        };
        Self {
            width,
            height,
            tree_width,
            editor_width,
            output_width,
            main_body_height,
            bottom_body_height,
            input_width,
            command_width,
        }
    }

    fn status_row(&self) -> usize {
        2 + self.main_body_height
    }

    fn bottom_title_row(&self) -> usize {
        self.status_row() + 1
    }

    fn bottom_body_start(&self) -> usize {
        self.bottom_title_row() + 1
    }

    fn footer_row(&self) -> usize {
        self.height.saturating_sub(1)
    }
}

#[derive(Debug)]
struct StudioApp {
    root: PathBuf,
    entry: PathBuf,
    options: VmOptions,
    focus: Focus,
    output_tab: OutputTab,
    tree: Vec<TreeEntry>,
    tree_index: usize,
    tree_scroll: usize,
    open_path: PathBuf,
    editor: TextBuffer,
    program_input: TextBuffer,
    run_log: LogBuffer,
    build_log: LogBuffer,
    security_log: LogBuffer,
    terminal_log: LogBuffer,
    security_policy: SecurityPolicy,
    security_blockers: usize,
    status: String,
    dirty: bool,
    prompt: Option<PromptState>,
    command_input: String,
    command_cursor: usize,
    command_history: Vec<String>,
    command_history_index: Option<usize>,
    last_command_status: String,
    last_frame: Option<ScreenFrame>,
    run_session: Option<InteractiveRunSession>,
}

pub fn run_studio(root: &Path, entry: &Path, options: VmOptions) -> Result<()> {
    let mut app = StudioApp::new(root, entry, options)?;
    let _guard = TerminalGuard::enter()?;
    app.run()?;
    Ok(())
}

impl StudioApp {
    fn new(root: &Path, entry: &Path, options: VmOptions) -> Result<Self> {
        let root = root.to_path_buf();
        fs::create_dir_all(&root)?;
        let entry = entry.to_path_buf();
        let initial_input = options.stdin.clone();
        let security_policy = security_policy_for_entry(&entry)?;
        if let Some(parent) = entry.parent() {
            fs::create_dir_all(parent)?;
        }
        if !entry.exists() {
            fs::write(&entry, default_file_template(&entry))?;
        }

        let mut app = Self {
            root,
            entry: entry.clone(),
            options,
            focus: Focus::Editor,
            output_tab: OutputTab::Run,
            tree: Vec::new(),
            tree_index: 0,
            tree_scroll: 0,
            open_path: entry,
            editor: TextBuffer::default(),
            program_input: TextBuffer::from_text(&initial_input),
            run_log: LogBuffer::with_lines(vec!["SNSX Studio ready".to_string()]),
            build_log: LogBuffer::with_lines(vec![
                "Compiler idle. Press Ctrl-B or use :build.".to_string()
            ]),
            security_log: LogBuffer::with_lines(vec![
                "Strict audit idle. Press Ctrl-T or use :audit.".to_string(),
            ]),
            terminal_log: LogBuffer::with_lines(vec![
                "SNSX shell ready.".to_string(),
                "Type `help`, `package list`, `module list`, or `!git status`.".to_string(),
            ]),
            security_policy,
            security_blockers: 0,
            status: studio_key_help().to_string(),
            dirty: false,
            prompt: None,
            command_input: String::new(),
            command_cursor: 0,
            command_history: Vec::new(),
            command_history_index: None,
            last_command_status: "No command executed yet".to_string(),
            last_frame: None,
            run_session: None,
        };
        app.refresh_tree()?;
        app.open_file(&app.open_path.clone())?;
        if !initial_input.is_empty() {
            app.program_input.cursor_y = app.program_input.lines.len().saturating_sub(1);
            app.program_input.cursor_x = app
                .program_input
                .lines
                .get(app.program_input.cursor_y)
                .map(|line| text_char_len(line))
                .unwrap_or(0);
        }
        app.run_program()?;
        Ok(app)
    }

    fn run(&mut self) -> Result<()> {
        self.draw()?;
        loop {
            match event::read()? {
                Event::Key(key) => {
                    let should_exit = match self.handle_key(key) {
                        Ok(exit) => exit,
                        Err(error) => {
                            self.report_error(error);
                            false
                        }
                    };
                    if should_exit {
                        break;
                    }
                    self.draw()?;
                }
                Event::Resize(_, _) => self.draw()?,
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if let Some(prompt) = self.prompt.clone() {
            return self.handle_prompt_key(prompt, key);
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('q') => {
                    self.save_if_dirty()?;
                    return Ok(true);
                }
                KeyCode::Char('s') => {
                    self.save_current_file()?;
                    self.run_program()?;
                    return Ok(false);
                }
                KeyCode::Char('r') => {
                    self.save_if_dirty()?;
                    self.run_program()?;
                    return Ok(false);
                }
                KeyCode::Char('b') => {
                    self.save_if_dirty()?;
                    self.build_program_for(Target::Vm)?;
                    return Ok(false);
                }
                KeyCode::Char('t') => {
                    self.save_if_dirty()?;
                    self.align_entry_to_execution_target()?;
                    self.refresh_security_report()?;
                    self.focus = Focus::Output;
                    self.output_tab = OutputTab::Security;
                    self.status = self.security_action_status("Audit completed");
                    return Ok(false);
                }
                KeyCode::Char('n') => {
                    self.begin_create_prompt(PromptKind::NewFile);
                    return Ok(false);
                }
                KeyCode::Char('g') => {
                    self.begin_create_prompt(PromptKind::NewFolder);
                    return Ok(false);
                }
                KeyCode::Char('w') => {
                    self.begin_rename_prompt();
                    return Ok(false);
                }
                KeyCode::Char('d') => {
                    self.begin_delete_prompt();
                    return Ok(false);
                }
                KeyCode::Char('l') => {
                    self.clear_active_output();
                    return Ok(false);
                }
                KeyCode::Char('p') => {
                    self.prime_command("open ", "Quick open: enter a workspace path");
                    return Ok(false);
                }
                KeyCode::Char('f') => {
                    self.prime_command("find ", "Workspace find: type a file fragment");
                    return Ok(false);
                }
                KeyCode::Char('e') => {
                    self.prime_command("search ", "Workspace search: type text to search");
                    return Ok(false);
                }
                KeyCode::Char('k') => {
                    self.prime_command(
                        "",
                        "Command deck ready: try agent, doctor, outline, snippet list, or build wasm",
                    );
                    return Ok(false);
                }
                KeyCode::Char('a') => {
                    self.prime_command(
                        "agent ",
                        "SNS AI coding agent ready: describe the SNSX program you want",
                    );
                    return Ok(false);
                }
                KeyCode::Char('o') => {
                    self.prime_command(
                        &format!("outline {}", self.display_workspace_path(&self.open_path)),
                        "Outline prepared for the current file",
                    );
                    return Ok(false);
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::F(1) => self.focus = Focus::Tree,
            KeyCode::F(2) => self.focus = Focus::Editor,
            KeyCode::F(3) => self.focus = Focus::Input,
            KeyCode::F(4) => {
                self.focus = Focus::Output;
                self.output_tab = OutputTab::Run;
            }
            KeyCode::F(5) => {
                self.focus = Focus::Output;
                self.output_tab = OutputTab::Build;
            }
            KeyCode::F(6) => {
                self.focus = Focus::Command;
                self.output_tab = OutputTab::Terminal;
            }
            KeyCode::F(7) => {
                self.focus = Focus::Output;
                self.output_tab = OutputTab::Security;
            }
            KeyCode::Tab => self.cycle_focus(),
            _ => match self.focus {
                Focus::Tree => self.handle_tree_key(key)?,
                Focus::Editor => self.handle_editor_key(key),
                Focus::Input => self.handle_input_key(key),
                Focus::Output => self.handle_output_key(key),
                Focus::Command => self.handle_command_key(key)?,
            },
        }

        self.ensure_visibility();
        Ok(false)
    }

    fn handle_prompt_key(&mut self, prompt: PromptState, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.prompt = None;
                self.status = "Prompt cancelled".to_string();
            }
            KeyCode::Backspace => {
                if let Some(current) = &mut self.prompt {
                    current.input.pop();
                }
            }
            KeyCode::Enter => {
                let input = prompt.input.trim().to_string();
                if input.is_empty() {
                    self.prompt = None;
                    self.status = "Prompt cancelled".to_string();
                    return Ok(false);
                }
                let result = match prompt.kind {
                    PromptKind::NewFile => {
                        self.prompt = None;
                        self.create_path(prompt.target.as_deref(), &input, false)
                    }
                    PromptKind::NewFolder => {
                        self.prompt = None;
                        self.create_path(prompt.target.as_deref(), &input, true)
                    }
                    PromptKind::Rename => {
                        self.prompt = None;
                        self.rename_path(prompt.target.as_deref(), &input)
                    }
                    PromptKind::Delete => {
                        self.prompt = None;
                        self.delete_path(prompt.target.as_deref(), &input)
                    }
                };
                if let Err(error) = result {
                    self.report_error(error);
                }
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(current) = &mut self.prompt {
                    current.input.push(ch);
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_tree_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Up => {
                self.tree_index = self.tree_index.saturating_sub(1);
                self.status = self.selection_status();
            }
            KeyCode::Down => {
                if self.tree_index + 1 < self.tree.len() {
                    self.tree_index += 1;
                }
                self.status = self.selection_status();
            }
            KeyCode::Enter => {
                if let Some(entry) = self.tree.get(self.tree_index).cloned() {
                    if !entry.is_dir {
                        if entry.path != self.open_path {
                            self.save_if_dirty()?;
                        }
                        self.open_file(&entry.path)?;
                        self.focus = Focus::Editor;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_editor_key(&mut self, key: KeyEvent) {
        let layout = StudioLayout::current();
        if apply_text_key(
            &mut self.editor,
            key,
            layout.main_body_height,
            layout.editor_width,
        ) {
            self.dirty = true;
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent) {
        let layout = StudioLayout::current();
        let _ = apply_text_key(
            &mut self.program_input,
            key,
            layout.bottom_body_height,
            layout.input_width,
        );
    }

    fn handle_output_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.active_log_mut().scroll_up(1),
            KeyCode::Down => self.active_log_mut().scroll_down(1),
            KeyCode::PageUp => self.active_log_mut().scroll_up(10),
            KeyCode::PageDown => self.active_log_mut().scroll_down(10),
            KeyCode::Home => self.active_log_mut().scroll = 0,
            KeyCode::End => {
                let max_scroll = self.active_log().lines.len().saturating_sub(1);
                self.active_log_mut().scroll = max_scroll;
            }
            KeyCode::Left => self.output_tab = self.output_tab.previous(),
            KeyCode::Right => self.output_tab = self.output_tab.next(),
            _ => {}
        }
    }

    fn handle_command_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.run_session.is_some()
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c'))
        {
            self.cancel_run_session("Interactive program input cancelled");
            return Ok(());
        }
        match key.code {
            KeyCode::Enter => self.execute_command()?,
            KeyCode::Backspace => {
                if self.command_cursor > 0 {
                    remove_char_at(&mut self.command_input, self.command_cursor - 1);
                    self.command_cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if self.command_cursor < text_char_len(&self.command_input) {
                    remove_char_at(&mut self.command_input, self.command_cursor);
                }
            }
            KeyCode::Left => {
                self.command_cursor = self.command_cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                if self.command_cursor < text_char_len(&self.command_input) {
                    self.command_cursor += 1;
                }
            }
            KeyCode::Home => self.command_cursor = 0,
            KeyCode::End => self.command_cursor = text_char_len(&self.command_input),
            KeyCode::Up => {
                if self.run_session.is_none() {
                    self.command_history_previous();
                }
            }
            KeyCode::Down => {
                if self.run_session.is_none() {
                    self.command_history_next();
                }
            }
            KeyCode::Esc => {
                if self.run_session.is_some() {
                    self.cancel_run_session("Interactive program input cancelled");
                } else {
                    self.command_input.clear();
                    self.command_cursor = 0;
                    self.command_history_index = None;
                }
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                insert_char_at(&mut self.command_input, self.command_cursor, ch);
                self.command_cursor += 1;
            }
            _ => {}
        }
        Ok(())
    }

    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Tree => Focus::Editor,
            Focus::Editor => Focus::Input,
            Focus::Input => Focus::Output,
            Focus::Output => Focus::Command,
            Focus::Command => Focus::Tree,
        };
        if self.focus == Focus::Command {
            self.output_tab = OutputTab::Terminal;
        }
    }

    fn refresh_tree(&mut self) -> Result<()> {
        let current_path = self
            .tree
            .get(self.tree_index)
            .map(|entry| entry.path.clone());
        self.tree.clear();
        collect_tree_entries(&self.root, &self.root, 0, &mut self.tree)?;
        if self.tree.is_empty() {
            self.tree.push(TreeEntry {
                path: self.root.clone(),
                depth: 0,
                is_dir: true,
            });
        }
        if let Some(path) = current_path {
            if let Some(index) = self.tree.iter().position(|entry| entry.path == path) {
                self.tree_index = index;
                return Ok(());
            }
        }
        if let Some(index) = self
            .tree
            .iter()
            .position(|entry| entry.path == self.open_path)
        {
            self.tree_index = index;
        } else {
            self.tree_index = 0;
        }
        Ok(())
    }

    fn open_file(&mut self, path: &Path) -> Result<()> {
        self.open_path = path.to_path_buf();
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to open {}", path.display()))?;
        self.editor.set_text(&raw);
        self.editor.cursor_x = 0;
        self.editor.cursor_y = 0;
        self.editor.scroll = 0;
        self.editor.hscroll = 0;
        self.dirty = false;
        self.status = format!("Opened {}", self.display_workspace_path(path));
        self.refresh_tree()?;
        self.select_path(path);
        Ok(())
    }

    fn save_if_dirty(&mut self) -> Result<()> {
        if self.dirty {
            self.save_current_file()?;
        }
        Ok(())
    }

    fn save_current_file(&mut self) -> Result<()> {
        if let Some(parent) = self.open_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut raw = self.editor.to_text();
        if !raw.ends_with('\n') {
            raw.push('\n');
        }
        fs::write(&self.open_path, raw)?;
        self.dirty = false;
        self.status = format!("Saved {}", self.display_workspace_path(&self.open_path));
        self.refresh_tree()?;
        self.select_path(&self.open_path.clone());
        if self.open_path == self.entry || self.is_manifest_path(&self.open_path) {
            let _ = self.refresh_security_report()?;
        }
        Ok(())
    }

    fn execution_target(&self) -> &Path {
        if self.open_path.extension().and_then(|ext| ext.to_str()) == Some("snsx")
            && !self.is_manifest_path(&self.open_path)
        {
            &self.open_path
        } else {
            &self.entry
        }
    }

    fn align_entry_to_execution_target(&mut self) -> Result<()> {
        let target = self.execution_target().to_path_buf();
        if target != self.entry {
            self.entry = target.clone();
            self.refresh_tree()?;
            self.select_path(&target);
            self.status = format!(
                "Execution target switched to {}",
                self.display_workspace_path(&target)
            );
        }
        Ok(())
    }

    fn run_program(&mut self) -> Result<()> {
        self.save_if_dirty()?;
        self.align_entry_to_execution_target()?;
        self.run_session = None;
        if !self.refresh_security_report()? {
            self.output_tab = OutputTab::Security;
            self.focus = Focus::Output;
            self.status = self.security_action_status("Run blocked");
            return Ok(());
        }
        let source = fs::read_to_string(&self.entry)?;
        let module = match compile_vm_with_policy(
            &self.entry.display().to_string(),
            &source,
            &self.security_policy,
        ) {
            Ok(module) => module,
            Err(diags) => {
                self.run_log.set_lines(
                    format_diagnostics(&diags)
                        .lines()
                        .map(ToString::to_string)
                        .collect(),
                );
                self.output_tab = OutputTab::Run;
                self.status = "Run failed during compilation".to_string();
                return Ok(());
            }
        };

        let mut options = self.options.clone();
        options.stdin = self.program_input.to_text();
        let mut vm = VirtualMachine::new(module, options);
        match vm.execute_main(Vec::new()) {
            Ok(result) => {
                self.run_session = None;
                let mut lines = if result.stdout.trim().is_empty() {
                    vec!["Program finished without stdout".to_string()]
                } else {
                    result
                        .stdout
                        .lines()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                };
                lines.push(String::new());
                lines.push(format!("Return value: {}", result.value));
                if !self.program_input.to_text().trim().is_empty() {
                    lines.push(format!(
                        "Input lines available: {}",
                        self.program_input.lines.len()
                    ));
                }
                if !result.trace.is_empty() {
                    lines.push(String::new());
                    lines.push("Trace:".to_string());
                    lines.extend(result.trace);
                }
                self.run_log.set_lines(lines);
                self.status = format!(
                    "Run succeeded for {}",
                    self.display_workspace_path(&self.entry)
                );
            }
            Err(error) => {
                if let Some(VmTrap::InputRequested(request)) = error.downcast_ref::<VmTrap>() {
                    self.show_input_request(request);
                    self.enter_run_session(request.consumed_lines);
                    self.status = format!(
                        "Program is waiting for input line {}",
                        request.consumed_lines + 1
                    );
                } else {
                    self.run_session = None;
                    self.run_log
                        .set_lines(error.to_string().lines().map(ToString::to_string).collect());
                    self.status = "Run failed during execution".to_string();
                }
            }
        }
        self.output_tab = OutputTab::Run;
        Ok(())
    }

    fn show_input_request(&mut self, request: &InputRequest) {
        let mut lines = if request.stdout.trim().is_empty() {
            vec!["Program is waiting for input".to_string()]
        } else {
            request
                .stdout
                .lines()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        };
        lines.push(String::new());
        lines.push(format!(
            "Program is waiting for input line {}.",
            request.consumed_lines + 1
        ));
        if !request.trace.is_empty() {
            lines.push(String::new());
            lines.push("Trace:".to_string());
            lines.extend(request.trace.iter().cloned());
        }
        self.run_log.set_lines(lines);
        self.output_tab = OutputTab::Run;
        self.focus = Focus::Command;
    }

    fn enter_run_session(&mut self, line_index: usize) {
        self.run_session = Some(InteractiveRunSession { line_index });
        self.focus = Focus::Command;
        self.output_tab = OutputTab::Run;
        self.command_history_index = None;
        self.command_input = self
            .program_input
            .lines
            .get(line_index)
            .cloned()
            .unwrap_or_default();
        self.command_cursor = text_char_len(&self.command_input);
        self.last_command_status = format!("Program waiting for input line {}", line_index + 1);
    }

    fn cancel_run_session(&mut self, message: &str) {
        self.run_session = None;
        self.command_input.clear();
        self.command_cursor = 0;
        self.command_history_index = None;
        self.last_command_status = message.to_string();
        self.status = message.to_string();
    }

    fn continue_run_session(&mut self, input: String) -> Result<()> {
        let Some(session) = self.run_session.take() else {
            return Ok(());
        };
        let mut lines = self.program_input.lines.clone();
        while lines.len() < session.line_index {
            lines.push(String::new());
        }
        if lines.len() == session.line_index {
            lines.push(input);
        } else {
            lines[session.line_index] = input;
        }
        self.program_input.set_text(&lines.join("\n"));
        self.command_input.clear();
        self.command_cursor = 0;
        self.command_history_index = None;
        self.last_command_status =
            format!("Submitted program input line {}", session.line_index + 1);
        self.run_program()
    }

    fn build_program_for(&mut self, target: Target) -> Result<()> {
        self.save_if_dirty()?;
        self.align_entry_to_execution_target()?;
        if !self.refresh_security_report()? {
            self.output_tab = OutputTab::Security;
            self.focus = Focus::Output;
            self.status = self.security_action_status("Build blocked");
            return Ok(());
        }
        let source = fs::read_to_string(&self.entry)?;
        match compile_source_with_policy(
            &self.entry.display().to_string(),
            &source,
            &CompileOptions {
                target,
                optimize: true,
                deterministic: self.options.deterministic,
            },
            &self.security_policy,
        ) {
            Ok(compilation) => {
                let output = self.default_output_path(target);
                if let Some(parent) = output.parent() {
                    fs::create_dir_all(parent)?;
                }
                write_artifact(&output, &compilation.artifact)?;
                let mut lines = vec![
                    format!(
                        "Build succeeded for {}",
                        self.display_workspace_path(&self.entry)
                    ),
                    format!("Target: {}", target_name(target)),
                    format!("Functions: {}", compilation.ir.functions.len()),
                    format!(
                        "Blocks: {}",
                        compilation
                            .ir
                            .functions
                            .iter()
                            .map(|function| function.blocks.len())
                            .sum::<usize>()
                    ),
                    format!(
                        "Values: {}",
                        compilation
                            .ir
                            .functions
                            .iter()
                            .map(|function| function.values)
                            .sum::<usize>()
                    ),
                    format!("Artifact: {}", output.display()),
                    String::new(),
                ];
                lines.extend(render_artifact_preview(&compilation.artifact));
                self.build_log.set_lines(lines);
                self.status = format!("Build succeeded ({})", target_name(target));
            }
            Err(diags) => {
                self.build_log.set_lines(
                    format_diagnostics(&diags)
                        .lines()
                        .map(ToString::to_string)
                        .collect(),
                );
                self.status = format!("Build failed ({})", target_name(target));
            }
        }
        self.output_tab = OutputTab::Build;
        Ok(())
    }

    fn execute_command(&mut self) -> Result<()> {
        if self.run_session.is_some() {
            let input = self.command_input.clone();
            return self.continue_run_session(input);
        }

        let command = self.command_input.trim().to_string();
        if command.is_empty() {
            self.status = "Command prompt is empty".to_string();
            return Ok(());
        }

        self.terminal_log
            .append_lines(vec![format!("snsx> {command}")]);
        self.command_history.push(command.clone());
        self.command_history_index = None;
        self.command_input.clear();
        self.command_cursor = 0;
        self.output_tab = OutputTab::Terminal;

        let shell_command = parse_shell_command(&command)?;
        self.execute_snsx_shell_command(shell_command)?;
        Ok(())
    }

    fn execute_snsx_shell_command(&mut self, command: ShellCommand) -> Result<()> {
        match command {
            ShellCommand::Help => {
                self.terminal_log.append_lines(shell_help_lines());
                self.last_command_status = "Displayed SNSX shell help".to_string();
            }
            ShellCommand::Tools => {
                self.terminal_log.append_lines(shell_tool_lines());
                self.last_command_status = "Displayed SNSX terminal tools".to_string();
            }
            ShellCommand::Docs => {
                self.terminal_log.append_lines(documentation_lines());
                self.last_command_status = "Displayed SNSX documentation catalog".to_string();
            }
            ShellCommand::Manual => {
                self.terminal_log.append_lines(vec![
                    "SNSX manual".to_string(),
                    documentation_lines()
                        .into_iter()
                        .nth(1)
                        .unwrap_or_else(|| "manual unavailable".to_string()),
                ]);
                self.last_command_status = "Displayed SNSX manual location".to_string();
            }
            ShellCommand::Spec => {
                self.terminal_log.append_lines(vec![
                    "SNSX language specification".to_string(),
                    documentation_lines()
                        .into_iter()
                        .nth(2)
                        .unwrap_or_else(|| "spec unavailable".to_string()),
                ]);
                self.last_command_status = "Displayed SNSX specification location".to_string();
            }
            ShellCommand::TechSpec => {
                self.terminal_log.append_lines(vec![
                    "SNSX technical specification".to_string(),
                    documentation_lines()
                        .into_iter()
                        .nth(3)
                        .unwrap_or_else(|| "techspec unavailable".to_string()),
                ]);
                self.last_command_status =
                    "Displayed SNSX technical specification location".to_string();
            }
            ShellCommand::History => {
                let mut lines = vec!["SNSX shell history:".to_string()];
                if self.command_history.is_empty() {
                    lines.push("(no commands yet)".to_string());
                } else {
                    lines.extend(
                        self.command_history
                            .iter()
                            .enumerate()
                            .map(|(index, item)| format!("{:>3}  {}", index + 1, item)),
                    );
                }
                self.terminal_log.append_lines(lines);
                self.last_command_status = "Displayed SNSX shell history".to_string();
            }
            ShellCommand::Status => {
                self.save_if_dirty()?;
                self.terminal_log
                    .append_lines(terminal_status_report(&self.root, &self.entry)?);
                self.last_command_status = "Displayed SNSX terminal status".to_string();
            }
            ShellCommand::PermissionsShow => {
                self.save_if_dirty()?;
                self.terminal_log
                    .append_lines(permissions_report(&self.entry)?);
                self.last_command_status = "Displayed SNSX permission policy".to_string();
            }
            ShellCommand::DependenciesShow => {
                self.save_if_dirty()?;
                self.terminal_log
                    .append_lines(dependencies_report(&self.root)?);
                self.last_command_status = "Displayed SNSX dependency report".to_string();
            }
            ShellCommand::Agent(prompt) => {
                let artifact = agent_preview(&prompt)?;
                let mut lines = vec![
                    format!("SNS AI agent preview {}", artifact.title),
                    format!("Recipe {}", artifact.recipe),
                    format!("Suggested path {}", artifact.suggested_path),
                    format!("Generated files {}", artifact.files.len().max(1)),
                    format!("Summary {}", artifact.summary),
                    format!(
                        "Dependencies {}",
                        if artifact.dependencies.is_empty() {
                            "none".to_string()
                        } else {
                            artifact.dependencies.join(", ")
                        }
                    ),
                ];
                if !artifact.notes.is_empty() {
                    lines.push("Notes:".to_string());
                    lines.extend(artifact.notes.iter().map(|note| format!("- {note}")));
                }
                if artifact.files.len() > 1 {
                    for file in &artifact.files {
                        lines.extend([
                            String::new(),
                            format!("File {}", file.path),
                            format!("Purpose {}", file.summary),
                        ]);
                        lines.extend(file.code.lines().map(ToString::to_string));
                    }
                } else {
                    lines.extend([String::new(), "Code preview:".to_string()]);
                    lines.extend(artifact.code.lines().map(ToString::to_string));
                }
                self.terminal_log.append_lines(lines);
                self.last_command_status = "Previewed SNS AI generated SNSX code".to_string();
            }
            ShellCommand::AgentWrite { path, prompt } => {
                let (_, target, lines) = agent_apply(
                    &self.root,
                    &self.entry,
                    self.root.as_path(),
                    &prompt,
                    Some(&path),
                )?;
                self.terminal_log.append_lines(lines);
                self.refresh_tree()?;
                self.open_file(&target)?;
                self.focus = Focus::Editor;
                self.last_command_status =
                    format!("SNS AI wrote {}", self.display_workspace_path(&target));
            }
            ShellCommand::Run => {
                self.save_if_dirty()?;
                self.run_program()?;
                self.terminal_log.append_lines(self.run_log.lines.clone());
                self.last_command_status = "Executed current SNSX file".to_string();
            }
            ShellCommand::Build(target) => {
                self.save_if_dirty()?;
                self.build_program_for(target)?;
                self.terminal_log.append_lines(self.build_log.lines.clone());
                self.last_command_status =
                    format!("Built current SNSX file for {}", target_name(target));
            }
            ShellCommand::Audit => {
                self.save_if_dirty()?;
                self.align_entry_to_execution_target()?;
                self.refresh_security_report()?;
                self.terminal_log
                    .append_lines(self.security_log.lines.clone());
                self.output_tab = OutputTab::Security;
                self.focus = Focus::Output;
                self.last_command_status = self.security_action_status("Audit completed");
            }
            ShellCommand::Doctor => {
                self.save_if_dirty()?;
                self.align_entry_to_execution_target()?;
                self.terminal_log
                    .append_lines(doctor_report(&self.root, &self.entry)?);
                self.last_command_status = "Rendered workspace doctor report".to_string();
            }
            ShellCommand::Clear => {
                self.terminal_log.clear("SNSX shell cleared");
                self.last_command_status = "Cleared SNSX shell output".to_string();
            }
            ShellCommand::Open(path) => {
                let path = self.resolve_workspace_input(self.root.as_path(), &path)?;
                self.save_if_dirty()?;
                self.open_file(&path)?;
                self.focus = Focus::Editor;
                self.last_command_status = format!("Opened {}", self.display_workspace_path(&path));
            }
            ShellCommand::Pwd => {
                self.terminal_log
                    .append_lines(vec![self.root.display().to_string()]);
                self.last_command_status = "Displayed workspace root".to_string();
            }
            ShellCommand::Ls(path) => {
                self.terminal_log.append_lines(list_directory(
                    &self.root,
                    self.root.as_path(),
                    path.as_deref(),
                )?);
                self.last_command_status = "Listed workspace path".to_string();
            }
            ShellCommand::Tree(path) => {
                self.terminal_log.append_lines(render_tree(
                    &self.root,
                    self.root.as_path(),
                    path.as_deref(),
                )?);
                self.last_command_status = "Rendered workspace tree".to_string();
            }
            ShellCommand::Find(query) => {
                self.terminal_log
                    .append_lines(find_workspace_paths(&self.root, &query)?);
                self.last_command_status = format!("Searched file paths for '{query}'");
            }
            ShellCommand::Search(query) => {
                self.terminal_log
                    .append_lines(search_workspace_text(&self.root, &query)?);
                self.last_command_status = format!("Searched workspace text for '{query}'");
            }
            ShellCommand::Outline(path) => {
                let target = match path {
                    Some(path) => self.resolve_workspace_input(self.root.as_path(), &path)?,
                    None => self.open_path.clone(),
                };
                self.terminal_log
                    .append_lines(outline_workspace_file(&self.root, &target)?);
                self.last_command_status = format!(
                    "Rendered outline for {}",
                    self.display_workspace_path(&target)
                );
            }
            ShellCommand::Cat(path) => {
                self.terminal_log.append_text(&read_workspace_file(
                    &self.root,
                    self.root.as_path(),
                    &path,
                )?);
                self.last_command_status = format!(
                    "Displayed {}",
                    self.display_workspace_path(
                        &self.resolve_workspace_input(self.root.as_path(), &path)?
                    )
                );
            }
            ShellCommand::Touch(path) => {
                let base = self.root.clone();
                self.create_path(Some(base.as_path()), &path, false)?;
                self.last_command_status = format!("Created {}", path);
            }
            ShellCommand::Mkdir(path) => {
                let base = self.root.clone();
                self.create_path(Some(base.as_path()), &path, true)?;
                self.last_command_status = format!("Created folder {}", path);
            }
            ShellCommand::Move { from, to } => {
                let source = self.resolve_workspace_input(self.root.as_path(), &from)?;
                self.rename_path(Some(&source), &to)?;
                self.last_command_status = format!("Moved {} to {}", from, to);
            }
            ShellCommand::Copy { from, to } => {
                let lines = copy_workspace_path(&self.root, self.root.as_path(), &from, &to)?;
                self.refresh_tree()?;
                let destination = self.resolve_workspace_input(self.root.as_path(), &to)?;
                self.select_path(&destination);
                self.terminal_log.append_lines(lines);
                self.last_command_status = format!("Copied {} to {}", from, to);
            }
            ShellCommand::Remove(path) => {
                let target = self.resolve_workspace_input(self.root.as_path(), &path)?;
                self.delete_path(Some(&target), "DELETE")?;
                self.last_command_status = format!("Deleted {}", path);
            }
            ShellCommand::EntryShow => {
                self.terminal_log.append_lines(vec![format!(
                    "Entry {}",
                    self.display_workspace_path(&self.entry)
                )]);
                self.last_command_status = "Displayed current entry".to_string();
            }
            ShellCommand::EntrySet(path) => {
                let path = self.resolve_workspace_input(self.root.as_path(), &path)?;
                self.set_entry_path(&path)?;
                self.last_command_status =
                    format!("Set entry to {}", self.display_workspace_path(&path));
            }
            ShellCommand::PackageList => {
                self.terminal_log.append_lines(list_packages(&self.root)?);
                self.last_command_status = "Listed SNSX packages".to_string();
            }
            ShellCommand::PackageAdd(spec) => {
                let lines = install_package(&self.root, &self.entry, &spec)?;
                self.terminal_log.append_lines(lines);
                self.refresh_tree()?;
                let _ = self.refresh_security_report()?;
                self.last_command_status = format!("Installed package {}", spec);
            }
            ShellCommand::PackageRemove(spec) => {
                let lines = remove_package(&self.root, &self.entry, &spec)?;
                self.terminal_log.append_lines(lines);
                self.refresh_tree()?;
                let _ = self.refresh_security_report()?;
                self.last_command_status = format!("Removed package {}", spec);
            }
            ShellCommand::ModuleList => {
                self.terminal_log.append_lines(list_modules(&self.root)?);
                self.last_command_status = "Listed SNSX modules".to_string();
            }
            ShellCommand::ModuleInstall(spec) => {
                let lines = install_package(&self.root, &self.entry, &spec)?;
                self.terminal_log.append_lines(lines);
                self.refresh_tree()?;
                let _ = self.refresh_security_report()?;
                self.last_command_status = format!("Installed module {}", spec);
            }
            ShellCommand::ModuleScaffold(name) => {
                let target = scaffold_module(&self.root, self.root.as_path(), &name)?;
                self.refresh_tree()?;
                self.open_file(&target)?;
                self.focus = Focus::Editor;
                self.terminal_log.append_lines(vec![format!(
                    "Scaffolded module {}",
                    self.display_workspace_path(&target)
                )]);
                self.last_command_status = format!("Scaffolded module {}", name);
            }
            ShellCommand::SnippetList => {
                let mut lines = vec!["SNSX snippets:".to_string()];
                lines.extend(
                    snippet_catalog()
                        .iter()
                        .map(|snippet| format!("{:<8} {}", snippet.name, snippet.description)),
                );
                self.terminal_log.append_lines(lines);
                self.last_command_status = "Listed SNSX snippets".to_string();
            }
            ShellCommand::SnippetCreate { kind, path } => {
                let target = scaffold_snippet(&self.root, self.root.as_path(), &kind, &path)?;
                self.refresh_tree()?;
                self.open_file(&target)?;
                self.focus = Focus::Editor;
                self.terminal_log.append_lines(vec![format!(
                    "Scaffolded {} snippet at {}",
                    kind,
                    self.display_workspace_path(&target)
                )]);
                self.last_command_status = format!("Scaffolded {} snippet", kind);
            }
            ShellCommand::ManifestShow => {
                self.terminal_log
                    .append_text(&manifest_snapshot(&self.root)?);
                self.last_command_status = "Displayed snsx.toml".to_string();
            }
            ShellCommand::PermSet { name, enabled } => {
                let lines = set_permission(&self.root, &self.entry, &name, enabled)?;
                self.terminal_log.append_lines(lines);
                let _ = self.refresh_security_report()?;
                self.last_command_status = format!(
                    "Set permission {} {}",
                    name,
                    if enabled { "on" } else { "off" }
                );
            }
            ShellCommand::Host(command) => {
                self.execute_shell_command(&command)?;
            }
        }
        Ok(())
    }

    fn prime_command(&mut self, command: &str, status: &str) {
        self.focus = Focus::Command;
        self.output_tab = OutputTab::Terminal;
        self.command_input = command.to_string();
        self.command_cursor = text_char_len(&self.command_input);
        self.command_history_index = None;
        self.status = status.to_string();
    }

    fn execute_shell_command(&mut self, command: &str) -> Result<()> {
        validate_host_shell_command(command)?;
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let output = Command::new(&shell)
            .arg("-lc")
            .arg(command)
            .current_dir(&self.root)
            .output()
            .with_context(|| format!("failed to execute shell command via {}", shell))?;

        if !output.stdout.is_empty() {
            self.terminal_log
                .append_text(&String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            self.terminal_log
                .append_text(&String::from_utf8_lossy(&output.stderr));
        }
        let status = match output.status.code() {
            Some(code) => format!("Shell command exited with status {code}"),
            None => "Shell command terminated by signal".to_string(),
        };
        self.terminal_log.append_lines(vec![status.clone()]);
        self.last_command_status = status;
        Ok(())
    }

    fn set_entry_path(&mut self, path: &Path) -> Result<()> {
        if path.is_dir() {
            bail!("entry must point to a file, not a folder");
        }
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, default_file_template(path))?;
        }
        self.entry = path.to_path_buf();
        self.sync_manifest_entry(path)?;
        self.open_file(path)?;
        self.focus = Focus::Editor;
        let _ = self.refresh_security_report()?;
        self.status = format!("Entry set to {}", self.display_workspace_path(path));
        Ok(())
    }

    fn clear_active_output(&mut self) {
        match self.output_tab {
            OutputTab::Run => self.run_log.clear("Run output cleared"),
            OutputTab::Build => self.build_log.clear("Build output cleared"),
            OutputTab::Security => self.security_log.clear("Security output cleared"),
            OutputTab::Terminal => self.terminal_log.clear("Terminal cleared"),
        }
        self.status = format!("Cleared {} panel", self.output_tab.title().to_lowercase());
    }

    fn command_history_previous(&mut self) {
        if self.command_history.is_empty() {
            return;
        }
        let next = match self.command_history_index {
            Some(index) if index > 0 => index - 1,
            Some(index) => index,
            None => self.command_history.len().saturating_sub(1),
        };
        self.command_history_index = Some(next);
        self.command_input = self.command_history[next].clone();
        self.command_cursor = text_char_len(&self.command_input);
    }

    fn command_history_next(&mut self) {
        if self.command_history.is_empty() {
            return;
        }
        let Some(index) = self.command_history_index else {
            return;
        };
        if index + 1 >= self.command_history.len() {
            self.command_history_index = None;
            self.command_input.clear();
            self.command_cursor = 0;
            return;
        }
        let next = index + 1;
        self.command_history_index = Some(next);
        self.command_input = self.command_history[next].clone();
        self.command_cursor = text_char_len(&self.command_input);
    }

    fn active_log(&self) -> &LogBuffer {
        match self.output_tab {
            OutputTab::Run => &self.run_log,
            OutputTab::Build => &self.build_log,
            OutputTab::Security => &self.security_log,
            OutputTab::Terminal => &self.terminal_log,
        }
    }

    fn active_log_mut(&mut self) -> &mut LogBuffer {
        match self.output_tab {
            OutputTab::Run => &mut self.run_log,
            OutputTab::Build => &mut self.build_log,
            OutputTab::Security => &mut self.security_log,
            OutputTab::Terminal => &mut self.terminal_log,
        }
    }

    fn create_path(&mut self, base: Option<&Path>, relative: &str, directory: bool) -> Result<()> {
        let base = base.unwrap_or(self.root.as_path());
        let target = self.resolve_workspace_input(base, relative)?;
        if directory {
            fs::create_dir_all(&target)?;
            self.refresh_tree()?;
            self.select_path(&target);
            self.status = format!("Created folder {}", self.display_workspace_path(&target));
        } else {
            self.save_if_dirty()?;
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            if !target.exists() {
                fs::write(&target, default_file_template(&target))?;
            }
            self.open_file(&target)?;
            self.focus = Focus::Editor;
            self.status = format!("Created file {}", self.display_workspace_path(&target));
        }
        Ok(())
    }

    fn rename_path(&mut self, selected: Option<&Path>, relative: &str) -> Result<()> {
        let Some(selected) = selected else {
            self.status = "Nothing selected to rename".to_string();
            return Ok(());
        };
        if selected == self.root {
            self.status = "Workspace root cannot be renamed".to_string();
            return Ok(());
        }
        if self.is_manifest_path(selected) {
            self.status =
                "snsx.toml is managed by the workspace and cannot be renamed here".to_string();
            return Ok(());
        }

        let destination = self.resolve_workspace_input(self.root.as_path(), relative)?;
        if destination == selected {
            self.status = "Rename skipped because the path is unchanged".to_string();
            return Ok(());
        }
        if destination.exists() {
            bail!("target already exists: {}", destination.display());
        }

        self.save_if_dirty()?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }

        let previous_open = self.open_path.clone();
        let previous_entry = self.entry.clone();
        fs::rename(selected, &destination)?;

        if let Some(next_entry) = remap_workspace_path(&previous_entry, selected, &destination) {
            self.entry = next_entry.clone();
            self.sync_manifest_entry(&next_entry)?;
        }
        if let Some(next_open) = remap_workspace_path(&previous_open, selected, &destination) {
            self.open_file(&next_open)?;
        } else {
            self.refresh_tree()?;
            self.select_path(&destination);
        }

        self.status = format!(
            "Renamed {} to {}",
            self.display_workspace_path(selected),
            self.display_workspace_path(&destination)
        );
        Ok(())
    }

    fn delete_path(&mut self, selected: Option<&Path>, confirmation: &str) -> Result<()> {
        let Some(selected) = selected else {
            self.status = "Nothing selected to delete".to_string();
            return Ok(());
        };
        if confirmation.trim() != "DELETE" {
            self.status = "Delete cancelled. Type DELETE to confirm removal".to_string();
            return Ok(());
        }
        if selected == self.root {
            self.status = "Workspace root cannot be deleted".to_string();
            return Ok(());
        }
        if self.is_manifest_path(selected) {
            self.status = "snsx.toml is protected from deletion inside the studio".to_string();
            return Ok(());
        }
        if self.entry == selected || self.entry.starts_with(selected) {
            self.status =
                "Cannot delete the active entry file or one of its parent folders".to_string();
            return Ok(());
        }

        self.save_if_dirty()?;
        let removed_label = self.display_workspace_path(selected);
        let reopen_entry = self.open_path == selected || self.open_path.starts_with(selected);
        if selected.is_dir() {
            fs::remove_dir_all(selected)?;
        } else {
            fs::remove_file(selected)?;
        }

        if reopen_entry {
            self.open_file(&self.entry.clone())?;
        } else {
            self.refresh_tree()?;
            self.select_path(&self.entry.clone());
        }
        self.status = format!("Deleted {}", removed_label);
        Ok(())
    }

    fn selected_base_dir(&self) -> PathBuf {
        self.tree
            .get(self.tree_index)
            .map(|entry| {
                if entry.is_dir {
                    entry.path.clone()
                } else {
                    entry.path.parent().unwrap_or(&self.root).to_path_buf()
                }
            })
            .unwrap_or_else(|| self.root.clone())
    }

    fn begin_create_prompt(&mut self, kind: PromptKind) {
        let base = self.selected_base_dir();
        let label = match kind {
            PromptKind::NewFile => format!("New file in {} > ", self.display_workspace_path(&base)),
            PromptKind::NewFolder => {
                format!("New folder in {} > ", self.display_workspace_path(&base))
            }
            PromptKind::Rename | PromptKind::Delete => return,
        };
        self.prompt = Some(PromptState {
            kind,
            input: String::new(),
            target: Some(base.clone()),
            label,
        });
        self.status = format!("Creating inside {}", self.display_workspace_path(&base));
    }

    fn begin_rename_prompt(&mut self) {
        let Some(selected) = self.selected_tree_path() else {
            self.status = "Select a file or folder in the tree to rename".to_string();
            return;
        };
        if selected == self.root {
            self.status = "Workspace root cannot be renamed".to_string();
            return;
        }
        if self.is_manifest_path(&selected) {
            self.status =
                "snsx.toml is managed by the workspace and cannot be renamed here".to_string();
            return;
        }
        let current = self.display_workspace_path(&selected);
        self.prompt = Some(PromptState {
            kind: PromptKind::Rename,
            input: current.clone(),
            target: Some(selected),
            label: format!("Rename {current} to workspace path > "),
        });
        self.status = "Rename or move the selected file/folder inside the workspace".to_string();
    }

    fn begin_delete_prompt(&mut self) {
        let Some(selected) = self.selected_tree_path() else {
            self.status = "Select a file or folder in the tree to delete".to_string();
            return;
        };
        if selected == self.root {
            self.status = "Workspace root cannot be deleted".to_string();
            return;
        }
        if self.is_manifest_path(&selected) {
            self.status = "snsx.toml is protected from deletion inside the studio".to_string();
            return;
        }
        self.prompt = Some(PromptState {
            kind: PromptKind::Delete,
            input: String::new(),
            target: Some(selected.clone()),
            label: format!(
                "Type DELETE to remove {} > ",
                self.display_workspace_path(&selected)
            ),
        });
        self.status = "Delete is guarded. Type DELETE exactly to confirm removal".to_string();
    }

    fn selected_tree_path(&self) -> Option<PathBuf> {
        self.tree
            .get(self.tree_index)
            .map(|entry| entry.path.clone())
    }

    fn selection_status(&self) -> String {
        self.tree
            .get(self.tree_index)
            .map(|entry| {
                let kind = if entry.is_dir { "Folder" } else { "File" };
                format!(
                    "{kind} selected: {}",
                    self.display_workspace_path(&entry.path)
                )
            })
            .unwrap_or_else(|| "Workspace is empty".to_string())
    }

    fn display_workspace_path(&self, path: &Path) -> String {
        if path == self.root {
            ".".to_string()
        } else {
            path.strip_prefix(&self.root)
                .unwrap_or(path)
                .display()
                .to_string()
        }
    }

    fn resolve_workspace_input(&self, base: &Path, input: &str) -> Result<PathBuf> {
        let input = input.trim();
        if input.is_empty() {
            bail!("path cannot be empty");
        }
        let raw = Path::new(input);
        if raw.is_absolute() {
            bail!("absolute paths are not allowed inside the studio");
        }
        let mut target = base.to_path_buf();
        for component in raw.components() {
            match component {
                Component::CurDir => {}
                Component::Normal(part) => target.push(part),
                Component::ParentDir => bail!("parent directory segments are not allowed"),
                Component::RootDir | Component::Prefix(_) => {
                    bail!("path must stay inside the current workspace")
                }
            }
        }
        if !target.starts_with(&self.root) {
            bail!("path must stay inside the current workspace");
        }
        Ok(target)
    }

    fn is_manifest_path(&self, path: &Path) -> bool {
        path == self.root.join("snsx.toml")
    }

    fn sync_manifest_entry(&self, entry: &Path) -> Result<()> {
        let manifest_path = self.root.join("snsx.toml");
        if !manifest_path.exists() {
            return Ok(());
        }
        let raw = fs::read_to_string(&manifest_path)?;
        let mut manifest = raw.parse::<toml::Value>()?;
        let relative = entry
            .strip_prefix(&self.root)
            .unwrap_or(entry)
            .display()
            .to_string()
            .replace('\\', "/");
        let package = manifest
            .get_mut("package")
            .and_then(|value| value.as_table_mut())
            .context("invalid snsx.toml: missing [package] table")?;
        package.insert("entry".to_string(), toml::Value::String(relative));
        fs::write(&manifest_path, toml::to_string_pretty(&manifest)?)?;
        Ok(())
    }

    fn select_path(&mut self, path: &Path) {
        if let Some(index) = self.tree.iter().position(|entry| entry.path == path) {
            self.tree_index = index;
            self.ensure_visibility();
        }
    }

    fn ensure_visibility(&mut self) {
        let layout = StudioLayout::current();
        self.editor
            .ensure_visible(layout.main_body_height, layout.editor_width);
        self.program_input
            .ensure_visible(layout.bottom_body_height, layout.input_width);
        if self.tree_index < self.tree_scroll {
            self.tree_scroll = self.tree_index;
        }
        if self.tree_index >= self.tree_scroll + layout.main_body_height {
            self.tree_scroll = self
                .tree_index
                .saturating_sub(layout.main_body_height.saturating_sub(1));
        }
        clamp_log_scroll(&mut self.run_log, layout.main_body_height);
        clamp_log_scroll(&mut self.build_log, layout.main_body_height);
        clamp_log_scroll(&mut self.security_log, layout.main_body_height);
        clamp_log_scroll(&mut self.terminal_log, layout.main_body_height);
    }

    fn refresh_security_report(&mut self) -> Result<bool> {
        self.security_policy = security_policy_for_entry(&self.entry)?;
        let source = fs::read_to_string(&self.entry)?;
        match audit_source(
            &self.entry.display().to_string(),
            &source,
            &self.security_policy,
        ) {
            Ok(_) => {
                self.security_blockers = 0;
                self.security_log.set_lines(vec![
                    "Security audit passed".to_string(),
                    format!("Policy: {}", self.security_policy_label()),
                    "The entry file satisfies strict SNSX audit rules.".to_string(),
                    "Run and build are unlocked.".to_string(),
                ]);
                Ok(true)
            }
            Err(diags) => {
                self.security_blockers = diags.len();
                let mut lines = vec![
                    format!("Security audit found {} blocker(s)", self.security_blockers),
                    format!("Policy: {}", self.security_policy_label()),
                    "SNSX strict mode will not run or build this project until every blocker is fixed."
                        .to_string(),
                    String::new(),
                ];
                lines.extend(
                    format_diagnostics(&diags)
                        .lines()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>(),
                );
                self.security_log.set_lines(lines);
                Ok(false)
            }
        }
    }

    fn security_policy_label(&self) -> String {
        format!(
            "strict | fs {} | net {} | ai {}",
            on_off(self.security_policy.allow_fs),
            on_off(self.security_policy.allow_network),
            on_off(self.security_policy.allow_ai)
        )
    }

    fn security_status_label(&self) -> String {
        if self.security_blockers == 0 {
            "security clean".to_string()
        } else {
            format!("security {} blocker(s)", self.security_blockers)
        }
    }

    fn security_action_status(&self, prefix: &str) -> String {
        if self.security_blockers == 0 {
            format!("{prefix}: strict audit is clean")
        } else {
            format!(
                "{prefix}: {} blocker(s) remain in the Security console",
                self.security_blockers
            )
        }
    }

    fn default_output_path(&self, target: Target) -> PathBuf {
        let stem = self
            .entry
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("main");
        let ext = match target {
            Target::Vm => "snsxbc",
            Target::Llvm => "ll",
            Target::Wasm => "wat",
            Target::NativeX86_64 => "x86_64.s",
            Target::NativeAarch64 => "aarch64.s",
        };
        self.entry
            .parent()
            .unwrap_or(self.root.as_path())
            .join("dist")
            .join(format!("{stem}.{ext}"))
    }

    fn report_error(&mut self, error: impl std::fmt::Display) {
        let message = error.to_string();
        self.terminal_log
            .append_lines(vec![format!("error: {message}")]);
        self.last_command_status = format!("error: {message}");
        self.status = format!("Action failed: {message}");
        self.output_tab = OutputTab::Terminal;
    }

    fn draw(&mut self) -> Result<()> {
        let frame = self.build_screen_frame();
        self.paint_frame(frame)
    }

    fn build_screen_frame(&mut self) -> ScreenFrame {
        let layout = StudioLayout::current();
        self.ensure_visibility();

        let tree_lines = self.render_tree_lines(layout.main_body_height, layout.tree_width);
        let editor_lines = self.render_editor_lines(layout.main_body_height, layout.editor_width);
        let output_lines = self.render_output_lines(layout.main_body_height, layout.output_width);
        let input_lines = self.render_input_lines(layout.bottom_body_height, layout.input_width);
        let command_lines =
            self.render_command_lines(layout.bottom_body_height, layout.command_width);

        let mut lines = Vec::with_capacity(layout.height);
        lines.push(clip_line(
            &format!(
                "SNSX Studio | root: {} | entry: {} | {} | {} | console: {}",
                self.root.display(),
                self.entry.display(),
                self.security_status_label(),
                self.security_policy_label(),
                self.output_tab.title()
            ),
            layout.width,
        ));
        lines.push(format!(
            "{:<tree_w$} | {:<editor_w$} | {:<output_w$}",
            panel_title("Files", self.focus == Focus::Tree),
            panel_title(
                &format!(
                    "Editor {}{}",
                    self.open_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("untitled"),
                    if self.dirty { " *" } else { "" }
                ),
                self.focus == Focus::Editor
            ),
            panel_title(&self.output_title(), self.focus == Focus::Output),
            tree_w = layout.tree_width,
            editor_w = layout.editor_width,
            output_w = layout.output_width,
        ));

        for row in 0..layout.main_body_height {
            lines.push(format!(
                "{:<tree_w$} | {:<editor_w$} | {:<output_w$}",
                tree_lines.get(row).cloned().unwrap_or_default(),
                editor_lines.get(row).cloned().unwrap_or_default(),
                output_lines.get(row).cloned().unwrap_or_default(),
                tree_w = layout.tree_width,
                editor_w = layout.editor_width,
                output_w = layout.output_width,
            ));
        }

        lines.push(clip_line(&self.status, layout.width));
        lines.push(format!(
            "{:<input_w$} | {:<command_w$}",
            panel_title("Program Input", self.focus == Focus::Input),
            panel_title(self.command_panel_title(), self.focus == Focus::Command),
            input_w = layout.input_width,
            command_w = layout.command_width,
        ));

        for row in 0..layout.bottom_body_height {
            lines.push(format!(
                "{:<input_w$} | {:<command_w$}",
                input_lines.get(row).cloned().unwrap_or_default(),
                command_lines.get(row).cloned().unwrap_or_default(),
                input_w = layout.input_width,
                command_w = layout.command_width,
            ));
        }

        if let Some(prompt) = &self.prompt {
            lines.push(clip_line(
                &format!("{}{}", prompt.label, prompt.input),
                layout.width,
            ));
        } else {
            lines.push(clip_line(studio_key_help(), layout.width));
        }

        while lines.len() < layout.height {
            lines.push(String::new());
        }
        lines.truncate(layout.height);

        ScreenFrame {
            lines,
            cursor: self.cursor_position(layout),
        }
    }

    fn paint_frame(&mut self, frame: ScreenFrame) -> Result<()> {
        let mut out = stdout();
        let previous_len = self
            .last_frame
            .as_ref()
            .map(|frame| frame.lines.len())
            .unwrap_or(0);
        let current_len = frame.lines.len();
        let full_redraw = self.last_frame.is_none() || previous_len != current_len;
        if full_redraw {
            execute!(out, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        }

        let previous = self.last_frame.as_ref();
        let row_count = if full_redraw {
            current_len
        } else {
            previous_len.max(current_len)
        };
        for row in 0..row_count {
            let next_line = frame.lines.get(row);
            let previous_line = previous.and_then(|prev| prev.lines.get(row));
            if !full_redraw && next_line == previous_line {
                continue;
            }
            queue!(
                out,
                cursor::MoveTo(0, row as u16),
                terminal::Clear(ClearType::CurrentLine)
            )?;
            if let Some(line) = next_line {
                queue!(out, Print(line))?;
            }
        }

        match frame.cursor {
            Some((x, y)) => queue!(out, cursor::MoveTo(x, y), cursor::Show)?,
            None => queue!(out, cursor::Hide)?,
        }

        out.flush()?;
        self.last_frame = Some(frame);
        Ok(())
    }

    fn cursor_position(&self, layout: StudioLayout) -> Option<(u16, u16)> {
        if let Some(prompt) = &self.prompt {
            let x = text_char_len(&prompt.label) + text_char_len(&prompt.input);
            return Some((
                clamp_cursor_axis(x, layout.width),
                clamp_cursor_axis(layout.footer_row(), layout.height),
            ));
        }

        match self.focus {
            Focus::Editor => Some((
                clamp_cursor_axis(
                    layout.tree_width
                        + 3
                        + TEXT_BUFFER_GUTTER_WIDTH
                        + self.editor.cursor_x.saturating_sub(self.editor.hscroll),
                    layout.width,
                ),
                clamp_cursor_axis(
                    2 + self.editor.cursor_y.saturating_sub(self.editor.scroll),
                    layout.height,
                ),
            )),
            Focus::Input => Some((
                clamp_cursor_axis(
                    TEXT_BUFFER_GUTTER_WIDTH
                        + self
                            .program_input
                            .cursor_x
                            .saturating_sub(self.program_input.hscroll),
                    layout.width,
                ),
                clamp_cursor_axis(
                    layout.bottom_body_start()
                        + self
                            .program_input
                            .cursor_y
                            .saturating_sub(self.program_input.scroll),
                    layout.height,
                ),
            )),
            Focus::Command => {
                let (_, cursor_offset) = render_command_prompt(
                    &self.command_prompt_prefix(),
                    &self.command_input,
                    self.command_cursor,
                    layout.command_width,
                );
                Some((
                    clamp_cursor_axis(layout.input_width + 3 + cursor_offset, layout.width),
                    clamp_cursor_axis(
                        layout.bottom_body_start() + layout.bottom_body_height.saturating_sub(1),
                        layout.height,
                    ),
                ))
            }
            _ => None,
        }
    }

    fn render_tree_lines(&self, height: usize, width: usize) -> Vec<String> {
        let end = (self.tree_scroll + height).min(self.tree.len());
        let mut lines = Vec::with_capacity(height);
        for index in self.tree_scroll..end {
            let entry = &self.tree[index];
            let name = if entry.path == self.root {
                self.root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(".")
                    .to_string()
            } else {
                entry
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("?")
                    .to_string()
            };
            let marker = if index == self.tree_index { ">" } else { " " };
            let open = if entry.path == self.open_path {
                "*"
            } else {
                " "
            };
            let kind = if entry.is_dir { "d" } else { "f" };
            let line = format!(
                "{}{} {} {}",
                marker,
                open,
                "  ".repeat(entry.depth) + kind,
                name
            );
            lines.push(clip_line(&line, width));
        }
        while lines.len() < height {
            lines.push(String::new());
        }
        lines
    }

    fn render_editor_lines(&self, height: usize, width: usize) -> Vec<String> {
        render_text_buffer(&self.editor, height, width)
    }

    fn render_input_lines(&self, height: usize, width: usize) -> Vec<String> {
        render_text_buffer(&self.program_input, height, width)
    }

    fn render_output_lines(&self, height: usize, width: usize) -> Vec<String> {
        render_log_buffer(self.active_log(), height, width)
    }

    fn render_command_lines(&self, height: usize, width: usize) -> Vec<String> {
        let (command_line, _) = render_command_prompt(
            &self.command_prompt_prefix(),
            &self.command_input,
            self.command_cursor,
            width,
        );
        let mut lines = if let Some(session) = self.run_session {
            vec![
                clip_line("Interactive SNSX program session", width),
                clip_line(
                    &format!("Waiting for input line {}", session.line_index + 1),
                    width,
                ),
                clip_line(
                    "Type the next program input and press Enter to continue execution.",
                    width,
                ),
                clip_line(
                    "Ctrl-C or Esc cancels this wait and returns the terminal to shell mode.",
                    width,
                ),
                command_line,
            ]
        } else {
            vec![
                clip_line(&format!("Root: {}", self.root.display()), width),
                clip_line(&format!("Last: {}", self.last_command_status), width),
                clip_line(
                    "Try tools, status, permissions, deps, agent <prompt>, package list, or !git status",
                    width,
                ),
                clip_line(
                    "Protected host shell: destructive host commands are blocked here",
                    width,
                ),
                command_line,
            ]
        };
        while lines.len() < height {
            lines.insert(lines.len().saturating_sub(1), String::new());
        }
        lines.truncate(height);
        lines
    }

    fn command_panel_title(&self) -> &'static str {
        if self.run_session.is_some() {
            "Program Console"
        } else {
            "SNSX Terminal"
        }
    }

    fn command_prompt_prefix(&self) -> String {
        match self.run_session {
            Some(session) => format!("in{}> ", session.line_index + 1),
            None => "> ".to_string(),
        }
    }

    fn output_title(&self) -> String {
        let tabs = [
            OutputTab::Run,
            OutputTab::Build,
            OutputTab::Security,
            OutputTab::Terminal,
        ]
        .into_iter()
        .map(|tab| {
            if tab == self.output_tab {
                format!("[{}]", tab.title())
            } else {
                tab.title().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
        format!("Console {}", tabs)
    }
}

fn render_text_buffer(buffer: &TextBuffer, height: usize, width: usize) -> Vec<String> {
    let content_width = width.saturating_sub(TEXT_BUFFER_GUTTER_WIDTH);
    let mut lines = Vec::with_capacity(height);
    for row in 0..height {
        let source_index = buffer.scroll + row;
        if let Some(line) = buffer.lines.get(source_index) {
            let visible = if content_width == 0 {
                String::new()
            } else {
                slice_chars(line, buffer.hscroll, content_width)
            };
            lines.push(clip_line(
                &format!("{:>4} {}", source_index + 1, visible),
                width,
            ));
        } else {
            lines.push(String::new());
        }
    }
    lines
}

fn apply_text_key(
    buffer: &mut TextBuffer,
    key: KeyEvent,
    visible_height: usize,
    visible_width: usize,
) -> bool {
    let mut edited = false;
    match key.code {
        KeyCode::Up => buffer.move_up(),
        KeyCode::Down => buffer.move_down(),
        KeyCode::Left => buffer.move_left(),
        KeyCode::Right => buffer.move_right(),
        KeyCode::Home => buffer.move_home(),
        KeyCode::End => buffer.move_end(),
        KeyCode::PageUp => {
            buffer.cursor_y = buffer.cursor_y.saturating_sub(visible_height);
            buffer.clamp_cursor();
        }
        KeyCode::PageDown => {
            buffer.cursor_y =
                (buffer.cursor_y + visible_height).min(buffer.lines.len().saturating_sub(1));
            buffer.clamp_cursor();
        }
        KeyCode::Backspace => edited = buffer.backspace(),
        KeyCode::Delete => edited = buffer.delete_forward(),
        KeyCode::Enter => {
            buffer.insert_newline();
            edited = true;
        }
        KeyCode::Tab => {
            buffer.insert_str("    ");
            edited = true;
        }
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            buffer.insert_char(ch);
            edited = true;
        }
        _ => {}
    }
    buffer.ensure_visible(visible_height, visible_width);
    edited
}

fn render_log_buffer(buffer: &LogBuffer, height: usize, width: usize) -> Vec<String> {
    let max_start = buffer.lines.len().saturating_sub(height);
    let start = buffer.scroll.min(max_start);
    let end = (start + height).min(buffer.lines.len());
    let mut lines = buffer.lines[start..end]
        .iter()
        .map(|line| clip_line(line, width))
        .collect::<Vec<_>>();
    while lines.len() < height {
        lines.push(String::new());
    }
    lines
}

fn clamp_log_scroll(buffer: &mut LogBuffer, height: usize) {
    let max_start = buffer.lines.len().saturating_sub(height.max(1));
    if buffer.scroll > max_start {
        buffer.scroll = max_start;
    }
}

fn panel_title(title: &str, active: bool) -> String {
    if active {
        format!("[{}]", title)
    } else {
        title.to_string()
    }
}

fn clip_line(line: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if text_char_len(line) <= width {
        return line.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }
    format!("{}...", slice_chars(line, 0, width - 3))
}

fn render_command_prompt(
    prefix: &str,
    command: &str,
    cursor: usize,
    width: usize,
) -> (String, usize) {
    if width == 0 {
        return (String::new(), 0);
    }

    let prompt_width = text_char_len(prefix);
    let content_width = width.saturating_sub(prompt_width);
    let command_len = text_char_len(command);
    let cursor = cursor.min(command_len);
    let start = cursor.saturating_sub(content_width.saturating_sub(1));
    let visible = slice_chars(command, start, content_width);
    let cursor_offset = prompt_width + cursor.saturating_sub(start);
    (
        clip_line(&format!("{prefix}{visible}"), width),
        cursor_offset,
    )
}

fn clamp_cursor_axis(value: usize, limit: usize) -> u16 {
    value.min(limit.saturating_sub(1)) as u16
}

fn text_char_len(text: &str) -> usize {
    text.chars().count()
}

fn slice_chars(text: &str, start: usize, width: usize) -> String {
    text.chars().skip(start).take(width).collect()
}

fn byte_index_for_char(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

fn insert_char_at(text: &mut String, char_index: usize, ch: char) {
    let byte_index = byte_index_for_char(text, char_index);
    text.insert(byte_index, ch);
}

fn remove_char_at(text: &mut String, char_index: usize) {
    let start = byte_index_for_char(text, char_index);
    let end = byte_index_for_char(text, char_index + 1);
    if start < end && start < text.len() {
        text.drain(start..end);
    }
}

fn collect_tree_entries(
    root: &Path,
    current: &Path,
    depth: usize,
    out: &mut Vec<TreeEntry>,
) -> Result<()> {
    out.push(TreeEntry {
        path: current.to_path_buf(),
        depth,
        is_dir: true,
    });

    let mut entries = fs::read_dir(current)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            !matches!(name.as_ref(), ".git" | "target" | "dist")
        })
        .collect::<Vec<_>>();

    entries.sort_by_key(|entry| {
        (
            !entry.file_type().map(|ty| ty.is_dir()).unwrap_or(false),
            entry.file_name().to_string_lossy().to_lowercase(),
        )
    });

    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_tree_entries(root, &path, depth + 1, out)?;
        } else {
            out.push(TreeEntry {
                path,
                depth: depth + 1,
                is_dir: false,
            });
        }
    }

    if current == root && out.len() == 1 {
        out.push(TreeEntry {
            path: root.join("src"),
            depth: 1,
            is_dir: true,
        });
    }

    Ok(())
}

fn render_artifact_preview(artifact: &Artifact) -> Vec<String> {
    match artifact {
        Artifact::Vm(module) => vec![
            "Bytecode preview:".to_string(),
            format!("Symbols: {}", module.symbols.len()),
            format!("Functions: {}", module.functions.len()),
            format!("Entry symbol index: {}", module.entry),
        ],
        Artifact::Text(text) => {
            let mut lines = vec!["Artifact preview:".to_string()];
            lines.extend(text.lines().take(14).map(ToString::to_string));
            lines
        }
        Artifact::Binary(bytes) => vec![
            "Binary artifact preview:".to_string(),
            format!("Bytes: {}", bytes.len()),
        ],
    }
}

fn target_name(target: Target) -> &'static str {
    match target {
        Target::Vm => "vm",
        Target::Llvm => "llvm",
        Target::Wasm => "wasm",
        Target::NativeX86_64 => "native-x86_64",
        Target::NativeAarch64 => "native-aarch64",
    }
}

fn on_off(enabled: bool) -> &'static str {
    if enabled {
        "on"
    } else {
        "off"
    }
}

fn studio_key_help() -> &'static str {
    "Keys: F1 tree | F2 editor | F3 input | F4 run | F5 build | F6 terminal | F7 security | Ctrl-S save+run current file | Ctrl-B build | Ctrl-T audit | Ctrl-A agent | Ctrl-P open | Ctrl-F find | Ctrl-E search | Ctrl-O outline | Ctrl-K command deck | Ctrl-N file | Ctrl-G folder | Ctrl-W rename | Ctrl-D delete | Ctrl-L clear | Ctrl-Q quit"
}

fn default_file_template(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("snsx") => "bring <module/std.io> as io\n\nentry\n    show \"SNSX Studio\"\n    0\n",
        _ => "",
    }
}

fn remap_workspace_path(current: &Path, previous: &Path, next: &Path) -> Option<PathBuf> {
    if current == previous {
        return Some(next.to_path_buf());
    }
    if !current.starts_with(previous) {
        return None;
    }
    let suffix = current.strip_prefix(previous).ok()?;
    Some(next.join(suffix))
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode()?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen, cursor::Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let mut out = stdout();
        let _ = execute!(out, LeaveAlternateScreen, cursor::Show);
    }
}
