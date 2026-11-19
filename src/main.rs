use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

// ============================================================================
// ANSI Colors (Catppuccin Mocha)
// ============================================================================

const GREEN: &str = "\x1b[38;2;166;227;161m"; // Complete - #a6e3a1
const YELLOW: &str = "\x1b[38;2;249;226;175m"; // Running - #f9e2af
const RED: &str = "\x1b[38;2;243;139;168m"; // Error - #f38ba8
const LAVENDER: &str = "\x1b[38;2;180;190;254m"; // Section icons - #b4befe
const GRAY: &str = "\x1b[0;37m"; // Separators
const NC: &str = "\x1b[0m"; // No color (reset)

// Nerd Font icons - status
const ICON_SPINNER: &str = "\u{f110}";
const ICON_CHECK: &str = "\u{f00c}";
const ICON_ERROR: &str = "\u{f00d}";

// Nerd Font icons - sections
const ICON_TODOS: &str = "\u{f14a}"; // checkbox
const ICON_TOOLS: &str = "\u{f0ad}"; // wrench
const ICON_SKILLS: &str = "\u{f0e7}"; // lightning bolt (skills)

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
enum Status {
    Running,
    Completed,
    Error,
}

impl Status {
    fn parse(s: &str, default: Status) -> Status {
        match s {
            "running" => Status::Running,
            "completed" => Status::Completed,
            "error" => Status::Error,
            _ => default,
        }
    }
}

#[derive(Debug, Default)]
struct ToolState {
    completed: HashMap<String, u32>,
}

#[derive(Debug, Clone)]
struct SkillEntry {
    name: String,
    status: Status,
    progress: Option<String>,
}

#[derive(Debug, Clone)]
struct TaskItem {
    subject: String,
    active_form: Option<String>,
    status: String, // "pending", "in_progress", "completed"
}

#[derive(Debug, Default)]
struct TaskState {
    tasks: Vec<(String, TaskItem)>, // (id, item) in creation order
}

#[derive(Debug, Default)]
struct TranscriptState {
    tools: ToolState,
    skills: Vec<SkillEntry>,
    tasks: TaskState,
}

// ============================================================================
// Transcript Parsing
// ============================================================================

fn parse_transcript(path: &Path) -> TranscriptState {
    let mut state = TranscriptState::default();
    let mut tool_starts: HashMap<String, String> = HashMap::new();
    let mut skill_starts: HashMap<String, SkillEntry> = HashMap::new();

    // Task ID counter (TaskCreate assigns sequential IDs: "1", "2", ...)
    let mut next_task_id: u32 = 1;

    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return state,
    };

    let reader = BufReader::new(file);

    // Track if we've seen a user message (pending new turn)
    let mut pending_reset = false;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        if line.trim().is_empty() {
            continue;
        }

        let value: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let line_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");

        if line_type == "user" {
            // Check if this is actually a tool result message (not a real user message)
            let is_tool_result = value
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
                .map(|arr| {
                    arr.iter().any(|block| {
                        block.get("type").and_then(|t| t.as_str()) == Some("tool_result")
                    })
                })
                .unwrap_or(false);

            // Check if this is a meta message (skill content injection, system message, etc.)
            let is_meta = value.get("isMeta").and_then(|v| v.as_bool()).unwrap_or(false);

            // Check if this is a skill content message (has sourceToolUseID)
            let is_skill_content = value.get("sourceToolUseID").is_some();

            if !is_tool_result && !is_meta && !is_skill_content {
                pending_reset = true;
            }
        }

        // Reset activity when assistant starts responding (new turn)
        if line_type == "assistant" && pending_reset {
            tool_starts.clear();
            skill_starts.clear();
            state.tools.completed.clear();
            state.skills.clear();
            state.tasks = TaskState::default();
            next_task_id = 1;
            pending_reset = false;
        }

        // Process message content
        if let Some(content) = value.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
            for block in content {
                let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");

                match block_type {
                    "tool_use" => {
                        let id = block.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let input = block.get("input");

                        if id.is_empty() || name.is_empty() {
                            continue;
                        }

                        // Handle TaskCreate: add task with sequential ID
                        if name == "TaskCreate" {
                            if let Some(input) = input {
                                let subject = input
                                    .get("subject")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("task");
                                let active_form = input
                                    .get("activeForm")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());

                                state.tasks.tasks.push((
                                    next_task_id.to_string(),
                                    TaskItem {
                                        subject: subject.to_string(),
                                        active_form,
                                        status: "pending".to_string(),
                                    },
                                ));
                                next_task_id += 1;
                            }
                            continue;
                        }

                        // Handle TaskUpdate: modify existing task status
                        if name == "TaskUpdate" {
                            if let Some(input) = input {
                                let task_id = input
                                    .get("taskId")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let status = input
                                    .get("status")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");

                                if !task_id.is_empty() && !status.is_empty() {
                                    if let Some((_, task)) = state.tasks.tasks.iter_mut().find(|(id, _)| id == task_id) {
                                        task.status = status.to_string();
                                    }
                                }
                            }
                            continue;
                        }

                        // Skip other task management tools from tool display
                        if name == "TaskOutput" || name == "TaskStop" || name == "TaskGet" || name == "TaskList" {
                            continue;
                        }

                        if name == "Skill" {
                            // Handle Skill invocations
                            // Use skill name as key to deduplicate (only show most recent per skill)
                            if let Some(input) = input {
                                let skill_name = input
                                    .get("skill")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("skill");

                                // Remove any previous entry for this skill name
                                skill_starts.retain(|_, entry| entry.name != skill_name);

                                skill_starts.insert(
                                    id.to_string(),
                                    SkillEntry {
                                        name: skill_name.to_string(),
                                        status: Status::Running,
                                        progress: None,
                                    },
                                );
                            }
                        } else {
                            // Regular tool
                            tool_starts.insert(id.to_string(), name.to_string());
                        }
                    }
                    "tool_result" => {
                        let tool_use_id = block
                            .get("tool_use_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let is_error = block.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);

                        if tool_use_id.is_empty() {
                            continue;
                        }

                        // Check if it's a skill
                        if let Some(skill) = skill_starts.get_mut(tool_use_id) {
                            skill.status = if is_error {
                                Status::Error
                            } else {
                                Status::Completed
                            };
                            continue;
                        }

                        // Regular tool - move to completed
                        if let Some(name) = tool_starts.remove(tool_use_id) {
                            *state.tools.completed.entry(name).or_insert(0) += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Convert skills
    state.skills = skill_starts.into_values().collect();

    // Keep only the most recent entries
    state.skills.truncate(3);

    state
}

// ============================================================================
// Session-Env Skill Status
// ============================================================================

#[derive(Debug, Deserialize)]
struct SessionSkillStatus {
    status: Option<String>,
    progress: Option<String>,
}

/// Read skill status from session-env directory.
/// Skills can write their own progress to ~/.claude/session-env/{session_id}/skill-status.json
fn read_session_skills(session_id: &str) -> HashMap<String, SessionSkillStatus> {
    if session_id.is_empty() {
        return HashMap::new();
    }

    let home = match env::var("HOME") {
        Ok(h) => h,
        Err(_) => return HashMap::new(),
    };

    let path = format!("{}/.claude/session-env/{}/skill-status.json", home, session_id);
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(_) => return HashMap::new(),
    };

    let reader = BufReader::new(file);
    match serde_json::from_reader(reader) {
        Ok(skills) => skills,
        Err(_) => HashMap::new(),
    }
}

/// Merge session-env skill status into transcript-parsed skills.
/// Session-env status overrides transcript status for running skills.
fn merge_session_skills(skills: &mut Vec<SkillEntry>, session_skills: &HashMap<String, SessionSkillStatus>) {
    for skill in skills.iter_mut() {
        if let Some(session_status) = session_skills.get(&skill.name) {
            if let Some(status_str) = &session_status.status {
                skill.status = Status::parse(status_str, skill.status.clone());
            }
            if session_status.progress.is_some() {
                skill.progress = session_status.progress.clone();
            }
        }
    }

    // Also add skills that exist only in session-env (not in transcript)
    for (name, session_status) in session_skills {
        if !skills.iter().any(|s| &s.name == name) {
            let status = session_status
                .status
                .as_ref()
                .map(|s| Status::parse(s, Status::Running))
                .unwrap_or(Status::Running);

            skills.push(SkillEntry {
                name: name.clone(),
                status,
                progress: session_status.progress.clone(),
            });
        }
    }
}

// ============================================================================
// Output Formatting
// ============================================================================

fn format_output(state: &TranscriptState) -> String {
    let mut parts: Vec<String> = vec![];

    if let Some(task_str) = format_tasks(&state.tasks) {
        parts.push(task_str);
    }

    if let Some(skill_str) = format_skills(&state.skills) {
        parts.push(skill_str);
    }

    if let Some(tool_str) = format_tools(&state.tools) {
        parts.push(tool_str);
    }

    if parts.is_empty() {
        return String::new();
    }

    parts.join(&format!(" {GRAY}|{NC} "))
}

fn format_skills(skills: &[SkillEntry]) -> Option<String> {
    if skills.is_empty() {
        return None;
    }

    let parts: Vec<String> = skills
        .iter()
        .map(|s| {
            let (color, icon) = match s.status {
                Status::Running => (YELLOW, ICON_SPINNER),
                Status::Completed => (GREEN, ICON_CHECK),
                Status::Error => (RED, ICON_ERROR),
            };

            // Show progress if available: "brainstorming (3/5)"
            let progress_str = s
                .progress
                .as_ref()
                .map(|p| format!(" ({})", p))
                .unwrap_or_default();

            format!("{color}{icon}{NC} {}{}", s.name, progress_str)
        })
        .collect();

    Some(format!("{LAVENDER}{ICON_SKILLS}{NC} {}", parts.join(" ")))
}

fn format_tasks(tasks: &TaskState) -> Option<String> {
    if tasks.tasks.is_empty() {
        return None;
    }

    let total = tasks.tasks.len() as u32;
    let done = tasks.tasks.iter().filter(|(_, t)| t.status == "completed").count() as u32;
    let current = tasks.tasks.iter().find(|(_, t)| t.status == "in_progress");

    let (color, icon) = if done == total {
        (GREEN, ICON_CHECK)
    } else {
        (YELLOW, ICON_SPINNER)
    };

    let text = if let Some((_, task)) = current {
        let display = task.active_form.as_deref().unwrap_or(&task.subject);
        format!(
            "{LAVENDER}{ICON_TODOS}{NC} {color}{icon}{NC} {display} ({done}/{total})",
        )
    } else if done == total {
        format!(
            "{LAVENDER}{ICON_TODOS}{NC} {color}{icon}{NC} All done ({done}/{total})",
        )
    } else {
        format!(
            "{LAVENDER}{ICON_TODOS}{NC} {color}{icon}{NC} {done}/{total}",
        )
    };

    Some(text)
}

fn format_tools(tools: &ToolState) -> Option<String> {
    if tools.completed.is_empty() {
        return None;
    }

    let mut completed: Vec<_> = tools.completed.iter().collect();
    completed.sort_by(|a, b| b.1.cmp(a.1));

    let parts: Vec<String> = completed
        .iter()
        .take(5)
        .map(|(name, count)| {
            let suffix = if **count > 1 {
                format!(" x{}", count)
            } else {
                String::new()
            };
            format!("{GREEN}{ICON_CHECK}{NC} {}{}", name, suffix)
        })
        .collect();

    Some(format!("{LAVENDER}{ICON_TOOLS}{NC} {}", parts.join(" ")))
}


// ============================================================================
// Main
// ============================================================================

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: claude-status <transcript_path> [session_id]");
        std::process::exit(1);
    }

    let path = std::path::Path::new(&args[1]);
    if !path.exists() {
        std::process::exit(0);
    }

    // Optional session_id for reading skill progress from session-env
    let session_id = args.get(2).map(|s| s.as_str()).unwrap_or("");

    let mut state = parse_transcript(path);

    // Merge session-env skill status (skills can report their own progress)
    if !session_id.is_empty() {
        let session_skills = read_session_skills(session_id);
        merge_session_skills(&mut state.skills, &session_skills);
    }

    let output = format_output(&state);

    if !output.is_empty() {
        println!("{}", output);
    }
}
