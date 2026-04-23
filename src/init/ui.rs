use anyhow::Result;
use std::io::{self, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, ClearType},
};

use super::{CategoryItem, LinterGroup};

fn run_arrow_selector<T>(
    items: &mut [T],
    print_fn: fn(&mut dyn Write, &[T], usize) -> Result<usize>,
    toggle_fn: fn(&mut T),
) -> Result<bool> {
    let mut cursor = 0usize;
    terminal::enable_raw_mode()?;
    let result = (|| -> Result<bool> {
        let mut stdout = io::stdout();
        let mut n_lines = print_fn(&mut stdout, items, cursor)?;
        loop {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Up if cursor > 0 => cursor -= 1,
                    KeyCode::Down if cursor + 1 < items.len() => cursor += 1,
                    KeyCode::Char(' ') => toggle_fn(&mut items[cursor]),
                    KeyCode::Enter => {
                        execute!(
                            stdout,
                            cursor::MoveUp(n_lines as u16),
                            terminal::Clear(ClearType::FromCursorDown)
                        )?;
                        return Ok(true);
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        execute!(
                            stdout,
                            cursor::MoveUp(n_lines as u16),
                            terminal::Clear(ClearType::FromCursorDown)
                        )?;
                        return Ok(false);
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        execute!(
                            stdout,
                            cursor::MoveUp(n_lines as u16),
                            terminal::Clear(ClearType::FromCursorDown)
                        )?;
                        return Ok(false);
                    }
                    _ => continue,
                }
                execute!(
                    stdout,
                    cursor::MoveUp(n_lines as u16),
                    terminal::Clear(ClearType::FromCursorDown)
                )?;
                n_lines = print_fn(&mut stdout, items, cursor)?;
            }
        }
    })();
    let _ = terminal::disable_raw_mode();
    println!();
    result
}

// --- Step 1: category selection ---

pub(super) fn select_categories_arrow(items: &mut [CategoryItem]) -> Result<bool> {
    run_arrow_selector(items, print_cat_selector, |item| {
        item.selected = !item.selected
    })
}

fn print_cat_selector(
    stdout: &mut dyn Write,
    items: &[CategoryItem],
    cursor: usize,
) -> Result<usize> {
    let mut lines = 0usize;
    write!(stdout, "Select categories:\r\n\r\n")?;
    lines += 2;
    for (i, item) in items.iter().enumerate() {
        let arrow = if i == cursor { ">" } else { " " };
        let sel = if item.selected { "✓" } else { " " };
        write!(stdout, "  {}  [{}]  {}\r\n", arrow, sel, item.label)?;
        lines += 1;
    }
    write!(
        stdout,
        "\r\n  ↑↓ navigate   space toggle   enter continue   q abort\r\n"
    )?;
    lines += 2;
    stdout.flush()?;
    Ok(lines)
}

// --- Step 2: linter table selection ---

/// Maps a flat row index (across all checks in all groups) to `(group_idx, check_idx)`.
fn flat_to_group_check(groups: &[LinterGroup], flat: usize) -> (usize, usize) {
    let mut remaining = flat;
    for (gi, group) in groups.iter().enumerate() {
        if remaining < group.checks.len() {
            return (gi, remaining);
        }
        remaining -= group.checks.len();
    }
    (0, 0)
}

pub(super) fn interactive_select_linters(groups: &mut Vec<LinterGroup>) -> Result<bool> {
    let total_rows = |gs: &[LinterGroup]| gs.iter().map(|g| g.checks.len()).sum::<usize>();
    let mut cursor = 0usize;
    terminal::enable_raw_mode()?;
    let result = (|| -> Result<bool> {
        let mut stdout = io::stdout();
        let mut n_lines = print_linter_table(&mut stdout, groups, cursor)?;
        loop {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Up if cursor > 0 => cursor -= 1,
                    KeyCode::Down if cursor + 1 < total_rows(groups) => cursor += 1,
                    KeyCode::Char(' ') => {
                        let (gi, ci) = flat_to_group_check(groups, cursor);
                        groups[gi].check_selected[ci] = !groups[gi].check_selected[ci];
                    }
                    KeyCode::Enter => {
                        execute!(
                            stdout,
                            cursor::MoveUp(n_lines as u16),
                            terminal::Clear(ClearType::FromCursorDown)
                        )?;
                        return Ok(true);
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        execute!(
                            stdout,
                            cursor::MoveUp(n_lines as u16),
                            terminal::Clear(ClearType::FromCursorDown)
                        )?;
                        return Ok(false);
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        execute!(
                            stdout,
                            cursor::MoveUp(n_lines as u16),
                            terminal::Clear(ClearType::FromCursorDown)
                        )?;
                        return Ok(false);
                    }
                    _ => continue,
                }
                execute!(
                    stdout,
                    cursor::MoveUp(n_lines as u16),
                    terminal::Clear(ClearType::FromCursorDown)
                )?;
                n_lines = print_linter_table(&mut stdout, groups, cursor)?;
            }
        }
    })();
    let _ = terminal::disable_raw_mode();
    println!();
    result
}

fn print_linter_table(
    stdout: &mut dyn Write,
    groups: &[LinterGroup],
    cursor: usize,
) -> Result<usize> {
    let name_w = groups
        .iter()
        .flat_map(|g| &g.checks)
        .map(|c| c.name.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let bin_w = groups
        .iter()
        .flat_map(|g| &g.checks)
        .map(|c| c.bin_name.len())
        .max()
        .unwrap_or(6)
        .max(6);

    let mut lines = 0usize;
    write!(
        stdout,
        "     {:<5}  {:<name_w$}  {:<bin_w$}  {:<4}  {:<30}  ACTION\r\n",
        "SEL",
        "NAME",
        "BINARY",
        "SPEED",
        "PATTERNS",
        name_w = name_w,
        bin_w = bin_w,
    )?;
    write!(
        stdout,
        "     {}\r\n",
        "-".repeat(5 + 2 + name_w + 2 + bin_w + 2 + 4 + 2 + 30 + 2 + 6)
    )?;
    lines += 2;

    let mut flat_idx = 0usize;
    for group in groups.iter() {
        let action = group.action();
        for (ci, check) in group.checks.iter().enumerate() {
            let sel_mark = if group.check_selected[ci] {
                "[✓]"
            } else {
                "[ ]"
            };
            let cursor_mark = if flat_idx == cursor { ">" } else { " " };
            let speed = match check.run_policy {
                crate::registry::RunPolicy::Fast => "fast",
                crate::registry::RunPolicy::Slow => "slow",
                crate::registry::RunPolicy::Adaptive => "adaptive",
            };
            let patterns = check.patterns.join(" ");
            write!(
                stdout,
                "  {}  {}  {:<name_w$}  {:<bin_w$}  {:<8}  {:<30}  {}\r\n",
                cursor_mark,
                sel_mark,
                check.name,
                check.bin_name,
                speed,
                patterns,
                action,
                name_w = name_w,
                bin_w = bin_w,
            )?;
            lines += 1;
            flat_idx += 1;
        }
    }

    write!(
        stdout,
        "\r\n  ↑↓ navigate   space toggle   enter apply   q abort\r\n"
    )?;
    lines += 2;
    stdout.flush()?;
    Ok(lines)
}
