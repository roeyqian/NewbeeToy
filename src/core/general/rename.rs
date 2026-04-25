use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use regex::RegexBuilder;
use slint::{ComponentHandle, ModelRc, VecModel};

use super::super::lang::{sanitize_ui_text, t, tf};
use crate::{MainWindow, RenamePreviewRow};

#[derive(Clone)]
struct RenamePair {
    old_path: PathBuf,
    new_path: PathBuf,
}

#[derive(Clone)]
struct PreviewBuild {
    rows: Vec<PreviewRow>,
    plan: Vec<RenamePair>,
    errors: Vec<String>,
}

#[derive(Clone)]
struct PreviewRow {
    entry_type: String,
    old_name: String,
    new_name: String,
    row_error: Option<String>,
    old_path: PathBuf,
    new_path: Option<PathBuf>,
}

#[derive(Clone, PartialEq, Eq)]
struct PreviewKey {
    folder: String,
    find_text: String,
    replace_text: String,
    use_regex: bool,
    case_sensitive: bool,
    count_syntax: bool,
}

#[derive(Clone)]
struct PreviewState {
    key: PreviewKey,
    rows: Vec<PreviewRow>,
    excluded_indices: HashSet<usize>,
    plan: Vec<RenamePair>,
    has_errors: bool,
}

#[derive(Clone)]
struct Candidate {
    old_path: PathBuf,
    old_name: String,
    new_name: String,
    is_dir: bool,
    error: Option<String>,
}

fn parse_counter_token(token: &str, row_index: usize, lang_en: bool) -> Result<String, String> {
    let body = token.trim();
    let parts = body.split(':').collect::<Vec<_>>();
    if parts.is_empty() || parts[0] != "IncNr" {
        return Err(tf(
            lang_en,
            "rename.msg.count_token_unsupported",
            &[("token", token)],
        ));
    }

    let start_raw = parts.get(1).copied().unwrap_or("1").trim();
    let step_raw = parts.get(2).copied().unwrap_or("1").trim();
    let pad_raw = parts.get(3).copied().unwrap_or("").trim();

    let start = start_raw.parse::<i64>().map_err(|_| {
        tf(
            lang_en,
            "rename.msg.count_start_invalid",
            &[("value", start_raw)],
        )
    })?;
    let step = step_raw.parse::<i64>().map_err(|_| {
        tf(
            lang_en,
            "rename.msg.count_step_invalid",
            &[("value", step_raw)],
        )
    })?;

    let inferred_pad = if start_raw.starts_with('0') && start_raw.len() > 1 {
        start_raw.len()
    } else {
        0
    };
    let pad_width = if pad_raw.is_empty() {
        inferred_pad
    } else {
        pad_raw.parse::<usize>().map_err(|_| {
            tf(
                lang_en,
                "rename.msg.count_pad_invalid",
                &[("value", pad_raw)],
            )
        })?
    };

    let value = start + step * row_index as i64;
    if pad_width > 0 {
        Ok(format!("{:0width$}", value, width = pad_width))
    } else {
        Ok(value.to_string())
    }
}

fn apply_count_syntax(
    replace_text: &str,
    row_index: usize,
    count_syntax: bool,
    lang_en: bool,
) -> Result<String, String> {
    if !count_syntax {
        return Ok(replace_text.to_string());
    }

    let mut result = String::new();
    let mut rest = replace_text;

    while let Some(start) = rest.find("<IncNr") {
        result.push_str(&rest[..start]);
        let token_with_tail = &rest[start + 1..];
        let Some(end) = token_with_tail.find('>') else {
            return Err(t(lang_en, "rename.msg.count_missing_closing"));
        };

        let token_body = &token_with_tail[..end];
        let rendered = parse_counter_token(token_body, row_index, lang_en)?;
        result.push_str(&rendered);
        rest = &token_with_tail[end + 1..];
    }

    result.push_str(rest);
    Ok(result)
}

fn validate_file_name(name: &str, lang_en: bool) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err(t(lang_en, "rename.msg.empty_file_name"));
    }

    if name.ends_with(' ') || name.ends_with('.') {
        return Err(tf(
            lang_en,
            "rename.msg.target_name_invalid_suffix",
            &[("name", name)],
        ));
    }

    let invalid = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
    if name.chars().any(|c| invalid.contains(&c)) {
        return Err(tf(
            lang_en,
            "rename.msg.target_name_illegal_char",
            &[("name", name)],
        ));
    }

    Ok(())
}

fn collect_files(dir: &Path, lang_en: bool) -> Result<Vec<PathBuf>, String> {
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(dir).map_err(|e| {
        let error = e.to_string();
        tf(lang_en, "rename.msg.read_dir_failed", &[("error", &error)])
    })?;

    for entry in read_dir {
        let entry = entry.map_err(|e| {
            let error = e.to_string();
            tf(
                lang_en,
                "rename.msg.read_dir_entry_failed",
                &[("error", &error)],
            )
        })?;
        let path = entry.path();
        if path.is_file() || path.is_dir() {
            entries.push(path);
        }
    }

    entries.sort_by_key(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });

    Ok(entries)
}

fn build_preview_and_plan(
    folder: &str,
    find_text: &str,
    replace_text: &str,
    use_regex: bool,
    case_sensitive: bool,
    count_syntax: bool,
    lang_en: bool,
) -> Result<PreviewBuild, String> {
    let folder_path = PathBuf::from(folder);
    if folder_path.as_os_str().is_empty() {
        return Err(t(lang_en, "rename.msg.choose_folder_first"));
    }
    if !folder_path.is_dir() {
        return Err(t(lang_en, "rename.msg.invalid_folder"));
    }

    let entries = collect_files(&folder_path, lang_en)?;
    if entries.is_empty() {
        return Err(t(lang_en, "rename.msg.folder_no_entries"));
    }

    let mut errors = Vec::new();

    let find_is_empty = find_text.is_empty();
    let matcher_regex = if find_is_empty {
        None
    } else if use_regex {
        match RegexBuilder::new(find_text)
            .case_insensitive(!case_sensitive)
            .build()
        {
            Ok(re) => Some(re),
            Err(e) => {
                let error = e.to_string();
                errors.push(tf(
                    lang_en,
                    "rename.msg.invalid_regex",
                    &[("error", &error)],
                ));
                None
            }
        }
    } else if !case_sensitive {
        match RegexBuilder::new(&regex::escape(find_text))
            .case_insensitive(true)
            .build()
        {
            Ok(re) => Some(re),
            Err(e) => {
                let error = e.to_string();
                errors.push(tf(
                    lang_en,
                    "rename.msg.matcher_build_failed",
                    &[("error", &error)],
                ));
                None
            }
        }
    } else {
        None
    };

    let mut candidates = Vec::with_capacity(entries.len());
    let mut matched_counter_index: usize = 0;

    for old_path in entries {
        let old_name = old_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let is_match_target = if find_is_empty {
            false
        } else if use_regex || !case_sensitive {
            matcher_regex
                .as_ref()
                .map(|re| re.is_match(&old_name))
                .unwrap_or(false)
        } else {
            old_name.contains(find_text)
        };

        let rendered_replace_text = if count_syntax && is_match_target {
            match apply_count_syntax(replace_text, matched_counter_index, true, lang_en) {
                Ok(text) => {
                    matched_counter_index += 1;
                    text
                }
                Err(err) => {
                    candidates.push(Candidate {
                        old_path,
                        old_name: old_name.clone(),
                        new_name: old_name,
                        is_dir: false,
                        error: Some(err),
                    });
                    continue;
                }
            }
        } else {
            replace_text.to_string()
        };

        let new_file_name = if !is_match_target {
            old_name.clone()
        } else {
            match &matcher_regex {
                Some(re) => re
                    .replace_all(&old_name, &rendered_replace_text)
                    .to_string(),
                None => old_name.replace(find_text, &rendered_replace_text),
            }
        };

        let mut error = None;
        if let Err(err) = validate_file_name(&new_file_name, lang_en) {
            error = Some(err);
        }

        candidates.push(Candidate {
            old_path,
            old_name,
            new_name: new_file_name,
            is_dir: false,
            error,
        });
    }

    for candidate in &mut candidates {
        candidate.is_dir = candidate.old_path.is_dir();
    }

    let mut final_names = HashSet::with_capacity(candidates.len());
    for candidate in &candidates {
        if candidate.error.is_none() && !final_names.insert(candidate.new_name.clone()) {
            errors.push(tf(
                lang_en,
                "rename.msg.name_conflict_after_replace",
                &[("name", &candidate.new_name)],
            ));
        }
    }

    let duplicate_names: HashSet<String> = {
        let mut seen = HashSet::new();
        let mut dup = HashSet::new();
        for candidate in &candidates {
            if seen.contains(&candidate.new_name) {
                dup.insert(candidate.new_name.clone());
            } else {
                seen.insert(candidate.new_name.clone());
            }
        }
        dup
    };

    let mut preview_rows = Vec::with_capacity(candidates.len());
    let mut plans = Vec::new();

    for candidate in candidates {
        let mut row_error = candidate.error.clone();
        if row_error.is_none() && duplicate_names.contains(&candidate.new_name) {
            row_error = Some(tf(
                lang_en,
                "rename.msg.name_conflict_row",
                &[("name", &candidate.new_name)],
            ));
        }

        let row_head = if candidate.is_dir {
            t(lang_en, "rename.entry.dir")
        } else {
            t(lang_en, "rename.entry.file")
        };
        match row_error {
            Some(err) => {
                errors.push(err.clone());
                preview_rows.push(PreviewRow {
                    entry_type: row_head,
                    old_name: candidate.old_name,
                    new_name: candidate.new_name,
                    row_error: Some(err),
                    old_path: candidate.old_path,
                    new_path: None,
                });
            }
            None => {
                let computed_new_path = if candidate.new_name != candidate.old_name {
                    let parent = candidate.old_path.parent().unwrap_or(folder_path.as_path());
                    Some(parent.join(candidate.new_name.clone()))
                } else {
                    None
                };

                preview_rows.push(PreviewRow {
                    entry_type: row_head,
                    old_name: candidate.old_name.clone(),
                    new_name: candidate.new_name.clone(),
                    row_error: None,
                    old_path: candidate.old_path.clone(),
                    new_path: computed_new_path.clone(),
                });

                if let Some(new_path) = computed_new_path {
                    plans.push(RenamePair {
                        old_path: candidate.old_path.clone(),
                        new_path,
                    });
                }
            }
        }
    }

    errors.sort();
    errors.dedup();

    Ok(PreviewBuild {
        rows: preview_rows,
        plan: plans,
        errors,
    })
}

fn apply_rename_plan(plans: &[RenamePair], lang_en: bool) -> Result<(), String> {
    // Stage into temp names first to prevent source/target name collisions.
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(plans.len());
    for (idx, pair) in plans.iter().enumerate() {
        let temp_path = pair
            .old_path
            .with_file_name(format!(".__newbeetoy_tmp__{}.tmp", idx));
        fs::rename(&pair.old_path, &temp_path).map_err(|e| {
            let path = pair.old_path.display().to_string();
            let error = e.to_string();
            tf(
                lang_en,
                "rename.msg.temp_rename_failed",
                &[("path", &path), ("error", &error)],
            )
        })?;
        staged.push((temp_path, pair.new_path.clone()));
    }

    for (temp_path, final_path) in staged {
        fs::rename(&temp_path, &final_path).map_err(|e| {
            let path = final_path.display().to_string();
            let error = e.to_string();
            tf(
                lang_en,
                "rename.msg.final_rename_failed",
                &[("path", &path), ("error", &error)],
            )
        })?;
    }

    Ok(())
}

pub fn append_status_log(ui: &MainWindow, level: &str, message: &str) {
    let current = ui.get_status_text().to_string();
    let mut lines: Vec<String> = if current.trim().is_empty() {
        Vec::new()
    } else {
        current.lines().map(|s| s.to_string()).collect()
    };

    lines.push(format!("[{}] {}", level, sanitize_ui_text(message)));

    const MAX_LOG_LINES: usize = 100;
    if lines.len() > MAX_LOG_LINES {
        let drop_count = lines.len() - MAX_LOG_LINES;
        lines.drain(0..drop_count);
    }

    ui.set_status_text(lines.join("\n").into());
}

fn set_preview_rows(ui: &MainWindow, rows: Vec<PreviewRow>) {
    let mapped = rows
        .into_iter()
        .map(|row| {
            let has_error = row.row_error.is_some();
            RenamePreviewRow {
                entry_type: sanitize_ui_text(&row.entry_type).into(),
                old_name: sanitize_ui_text(&row.old_name).into(),
                new_name: sanitize_ui_text(&row.new_name).into(),
                row_error: sanitize_ui_text(&row.row_error.unwrap_or_default()).into(),
                has_error,
            }
        })
        .collect::<Vec<_>>();
    ui.set_preview_rows(ModelRc::new(VecModel::from(mapped)));
}

fn apply_preview_exclusions(ui: &MainWindow, state: &mut PreviewState) {
    let mut visible_rows = Vec::new();
    let mut plan = Vec::new();
    let mut has_errors = false;

    for (idx, row) in state.rows.iter().enumerate() {
        if state.excluded_indices.contains(&idx) {
            continue;
        }

        if row.row_error.is_some() {
            has_errors = true;
        } else if let Some(new_path) = &row.new_path {
            plan.push(RenamePair {
                old_path: row.old_path.clone(),
                new_path: new_path.clone(),
            });
        }

        visible_rows.push(row.clone());
    }

    state.plan = plan;
    state.has_errors = has_errors;
    ui.set_preview_has_error(has_errors);
    set_preview_rows(ui, visible_rows);
}

fn refresh_preview(
    ui: &MainWindow,
    preview_state: &Rc<RefCell<Option<PreviewState>>>,
    key: PreviewKey,
    lang_en: bool,
) {
    match build_preview_and_plan(
        &key.folder,
        &key.find_text,
        &key.replace_text,
        key.use_regex,
        key.case_sensitive,
        key.count_syntax,
        lang_en,
    ) {
        Ok(build) => {
            ui.set_preview_text("".into());
            set_preview_rows(ui, build.rows.clone());
            let has_errors = !build.errors.is_empty();
            ui.set_preview_has_error(has_errors);

            *preview_state.borrow_mut() = Some(PreviewState {
                key,
                rows: build.rows.clone(),
                excluded_indices: HashSet::new(),
                plan: build.plan.clone(),
                has_errors,
            });

            let status = if build.plan.is_empty() {
                let entries = build.rows.len().to_string();
                tf(
                    lang_en,
                    "rename.msg.preview_refreshed_no_actions",
                    &[("entries", &entries)],
                )
            } else {
                let entries = build.rows.len().to_string();
                let renames = build.plan.len().to_string();
                tf(
                    lang_en,
                    "rename.msg.preview_refreshed_with_actions",
                    &[("entries", &entries), ("renames", &renames)],
                )
            };
            append_status_log(ui, "INFO", &status);

            if has_errors {
                for err in &build.errors {
                    append_status_log(ui, "ERROR", err);
                }
            }
        }
        Err(err) => {
            ui.set_preview_has_error(true);
            let error_prefix = t(lang_en, "rename.log.error_prefix");
            ui.set_preview_text(format!("[{}] {}", error_prefix, err).into());
            set_preview_rows(ui, Vec::new());
            *preview_state.borrow_mut() = Some(PreviewState {
                key,
                rows: Vec::new(),
                excluded_indices: HashSet::new(),
                plan: Vec::new(),
                has_errors: true,
            });
            append_status_log(
                ui,
                "ERROR",
                &tf(lang_en, "rename.msg.preview_failed", &[("error", &err)]),
            );
        }
    }
}

pub fn setup_rename_handlers(ui: &MainWindow) {
    let last_success_plan: Rc<RefCell<Option<Vec<RenamePair>>>> = Rc::new(RefCell::new(None));
    let latest_preview_state: Rc<RefCell<Option<PreviewState>>> = Rc::new(RefCell::new(None));

    ui.set_status_text("".into());
    ui.set_preview_text("".into());
    ui.set_preview_rows(ModelRc::new(VecModel::from(Vec::<RenamePreviewRow>::new())));
    append_status_log(ui, "INFO", &t(ui.get_lang_en(), "rename.msg.ready"));

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&latest_preview_state);
        ui.on_preview_request(
            move |folder, find_text, replace_text, use_regex, case_sensitive, count_syntax| {
                let Some(ui) = ui_handle.upgrade() else {
                    return;
                };

                let key = PreviewKey {
                    folder: folder.as_str().to_string(),
                    find_text: find_text.as_str().to_string(),
                    replace_text: replace_text.as_str().to_string(),
                    use_regex,
                    case_sensitive,
                    count_syntax,
                };

                refresh_preview(&ui, &preview_state, key, ui.get_lang_en());
            },
        );
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&latest_preview_state);
        ui.on_remove_preview_row_request(move |index| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let mut borrowed = preview_state.borrow_mut();
            let Some(state) = borrowed.as_mut() else {
                append_status_log(
                    &ui,
                    "ERROR",
                    &t(ui.get_lang_en(), "rename.msg.generate_preview_first"),
                );
                return;
            };

            let visible_indices = state
                .rows
                .iter()
                .enumerate()
                .filter_map(|(idx, _)| (!state.excluded_indices.contains(&idx)).then_some(idx))
                .collect::<Vec<_>>();

            let row_index = index as usize;
            if row_index >= visible_indices.len() {
                return;
            }

            let removed_idx = visible_indices[row_index];
            let removed_name = state.rows[removed_idx].old_name.clone();
            state.excluded_indices.insert(removed_idx);
            apply_preview_exclusions(&ui, state);

            append_status_log(
                &ui,
                "INFO",
                &tf(
                    ui.get_lang_en(),
                    "rename.msg.row_removed",
                    &[("name", &removed_name)],
                ),
            );
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&latest_preview_state);
        ui.on_clear_preview_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            ui.set_preview_text("".into());

            let mut borrowed = preview_state.borrow_mut();
            if let Some(state) = borrowed.as_mut() {
                state.excluded_indices = (0..state.rows.len()).collect();
                apply_preview_exclusions(&ui, state);
            } else {
                ui.set_preview_has_error(false);
                ui.set_preview_rows(ModelRc::new(VecModel::from(Vec::<RenamePreviewRow>::new())));
            }

            append_status_log(
                &ui,
                "INFO",
                &t(ui.get_lang_en(), "rename.msg.cleared"),
            );
        });
    }

    {
        let ui_handle = ui.as_weak();
        let last_plan = Rc::clone(&last_success_plan);
        let preview_state = Rc::clone(&latest_preview_state);
        ui.on_apply_request(
            move || {
                let Some(ui) = ui_handle.upgrade() else {
                    return;
                };

                let snapshot = preview_state.borrow().clone();
                let Some(snapshot) = snapshot else {
                    append_status_log(
                        &ui,
                        "ERROR",
                        &t(ui.get_lang_en(), "rename.msg.generate_preview_first"),
                    );
                    return;
                };

                if snapshot.has_errors {
                    append_status_log(
                        &ui,
                        "ERROR",
                        &t(ui.get_lang_en(), "rename.msg.preview_has_errors"),
                    );
                    return;
                }

                if snapshot.plan.is_empty() {
                    append_status_log(
                        &ui,
                        "INFO",
                        &t(ui.get_lang_en(), "rename.msg.no_rename_actions"),
                    );
                    return;
                }

                let refresh_key = snapshot.key.clone();
                match apply_rename_plan(&snapshot.plan, ui.get_lang_en()) {
                    Ok(()) => {
                        *last_plan.borrow_mut() = Some(snapshot.plan.clone());
                        let count = snapshot.plan.len().to_string();
                        append_status_log(
                            &ui,
                            "INFO",
                            &tf(
                                ui.get_lang_en(),
                                "rename.msg.apply_success",
                                &[("count", &count)],
                            ),
                        );

                        refresh_preview(&ui, &preview_state, refresh_key, ui.get_lang_en());
                    }
                    Err(err) => {
                        append_status_log(
                            &ui,
                            "ERROR",
                            &tf(
                                ui.get_lang_en(),
                                "rename.msg.apply_failed",
                                &[("error", &err)],
                            ),
                        );
                    }
                }
            },
        );
    }

    {
        let ui_handle = ui.as_weak();
        let last_plan = Rc::clone(&last_success_plan);
        let preview_state = Rc::clone(&latest_preview_state);
        ui.on_undo_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let undo_plan = {
                let borrowed = last_plan.borrow();
                let Some(previous) = borrowed.as_ref() else {
                    append_status_log(
                        &ui,
                        "ERROR",
                        &t(ui.get_lang_en(), "rename.msg.no_undo_history"),
                    );
                    return;
                };

                previous
                    .iter()
                    .map(|pair| RenamePair {
                        old_path: pair.new_path.clone(),
                        new_path: pair.old_path.clone(),
                    })
                    .collect::<Vec<_>>()
            };

            match apply_rename_plan(&undo_plan, ui.get_lang_en()) {
                Ok(()) => {
                    *last_plan.borrow_mut() = None;
                    let count = undo_plan.len().to_string();
                    append_status_log(
                        &ui,
                        "INFO",
                        &tf(
                            ui.get_lang_en(),
                            "rename.msg.undo_success",
                            &[("count", &count)],
                        ),
                    );

                    let key = PreviewKey {
                        folder: ui.get_folder_path().to_string(),
                        find_text: ui.get_find_text().to_string(),
                        replace_text: ui.get_replace_text().to_string(),
                        use_regex: ui.get_use_regex(),
                        case_sensitive: ui.get_case_sensitive(),
                        count_syntax: ui.get_count_syntax(),
                    };
                    refresh_preview(&ui, &preview_state, key, ui.get_lang_en());
                }
                Err(err) => {
                    append_status_log(
                        &ui,
                        "ERROR",
                        &tf(
                            ui.get_lang_en(),
                            "rename.msg.undo_failed",
                            &[("error", &err)],
                        ),
                    );
                }
            }
        });
    }
}

