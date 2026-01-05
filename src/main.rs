use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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
const ICON_AGENTS: &str = "\u{ee0d}"; // robot
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

#[derive(Debug, Clone)]
struct RunningTool {
    name: String,
    target: Option<String>,
}

#[derive(Debug, Default)]
struct ToolState {
    running: Vec<RunningTool>,
    completed: HashMap<String, u32>,
}

#[derive(Debug, Clone)]
struct AgentEntry {
    agent_type: String,
    status: Status,
    start_time: Option<String>,
    end_time: Option<String>,
}

#[derive(Debug, Clone)]
struct SkillEntry {
    name: String,
    status: Status,
}

#[derive(Debug, Default)]
struct TodoState {
    current: Option<String>,
    done: u32,
    total: u32,
}

#[derive(Debug, Default)]
struct TranscriptState {
    tools: ToolState,
    agents: Vec<AgentEntry>,
    skills: Vec<SkillEntry>,
    todos: TodoState,
}

// ============================================================================
// JSONL Parsing
// ============================================================================

#[derive(Debug, Deserialize)]
struct TodoItem {
    status: Option<String>,
    #[serde(rename = "activeForm")]
    active_form: Option<String>,
}

fn extract_target(name: &str, input: Option<&Value>) -> Option<String> {
    let input = input?;

    match name {
        "Read" | "Write" | "Edit" | "NotebookEdit" => {
            let path = input
                .get("file_path")
                .or_else(|| input.get("notebook_path"))
                .and_then(|v| v.as_str())?;
            Some(basename(path))
        }
        "Glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|s| truncate(s, 20)),
        "Grep" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|s| truncate(s, 20)),
        "Bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .map(|s| truncate(s, 25)),
        "Task" => input
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| truncate(s, 30)),
        "WebFetch" | "WebSearch" => input
            .get("url")
            .or_else(|| input.get("query"))
            .and_then(|v| v.as_str())
            .map(|s| truncate(s, 25)),
        _ => None,
    }
}

fn basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

fn update_todos(state: &mut TodoState, todos: &[TodoItem]) {
    state.total = todos.len() as u32;
    state.done = todos
        .iter()
        .filter(|t| t.status.as_deref() == Some("completed"))
        .count() as u32;
    state.current = todos
        .iter()
        .find(|t| t.status.as_deref() == Some("in_progress"))
        .and_then(|t| t.active_form.clone());
}

// ============================================================================
// Transcript Parsing
// ============================================================================

fn parse_transcript(path: &Path) -> TranscriptState {
    let mut state = TranscriptState::default();
    let mut tool_starts: HashMap<String, (String, Option<String>)> = HashMap::new();
    let mut agent_starts: HashMap<String, AgentEntry> = HashMap::new();
    let mut skill_starts: HashMap<String, SkillEntry> = HashMap::new();

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
        let timestamp = value.get("timestamp").and_then(|v| v.as_str()).map(String::from);

        // Check if this is an agent-level message (has agentId) vs top-level conversation
        let is_top_level = value.get("agentId").is_none();

        if line_type == "user" && is_top_level {
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

            // Check if this is an agent notification (background task completion)
            let is_agent_notification = value
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
                .map(|s| s.starts_with("<agent-notification>"))
                .unwrap_or(false);

            if !is_tool_result && !is_meta && !is_skill_content && !is_agent_notification {
                pending_reset = true;
            }
        }

        // Reset activity when assistant starts responding (new turn)
        if line_type == "assistant" && is_top_level && pending_reset {
            tool_starts.clear();
            // Keep running agents, only clear completed/errored ones
            agent_starts.retain(|_, agent| agent.status == Status::Running);
            skill_starts.clear();
            state.tools.completed.clear();
            state.tools.running.clear();
            state.agents.clear();
            state.skills.clear();
            pending_reset = false;
        }

        // Process todos from user messages
        if let Some(todos) = value.get("todos").and_then(|v| v.as_array()) {
            let todo_items: Vec<TodoItem> = todos
                .iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect();
            update_todos(&mut state.todos, &todo_items);
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

                        // Handle TodoWrite
                        if name == "TodoWrite" {
                            if let Some(input) = input {
                                if let Some(todos_arr) = input.get("todos").and_then(|v| v.as_array()) {
                                    let todo_items: Vec<TodoItem> = todos_arr
                                        .iter()
                                        .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                        .collect();
                                    update_todos(&mut state.todos, &todo_items);
                                }
                            }
                        }

                        // Handle Task (agents)
                        if name == "Task" {
                            if let Some(input) = input {
                                let agent_type = input
                                    .get("subagent_type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("agent");

                                agent_starts.insert(
                                    id.to_string(),
                                    AgentEntry {
                                        agent_type: agent_type.to_string(),
                                        status: Status::Running,
                                        start_time: timestamp.clone(),
                                        end_time: None,
                                    },
                                );
                            }
                        } else if name == "Skill" {
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
                                    },
                                );
                            }
                        } else {
                            // Regular tool
                            let target = extract_target(name, input);
                            tool_starts.insert(id.to_string(), (name.to_string(), target));
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

                        // Check if it's an agent
                        if let Some(agent) = agent_starts.get_mut(tool_use_id) {
                            agent.status = if is_error {
                                Status::Error
                            } else {
                                Status::Completed
                            };
                            agent.end_time = timestamp.clone();
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

                        // Regular tool - move from running to completed
                        if let Some((name, _)) = tool_starts.remove(tool_use_id) {
                            *state.tools.completed.entry(name).or_insert(0) += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Convert remaining tool_starts to running tools
    state.tools.running = tool_starts
        .into_values()
        .map(|(name, target)| RunningTool { name, target })
        .collect();

    // Convert agents
    state.agents = agent_starts.into_values().collect();

    // Convert skills
    state.skills = skill_starts.into_values().collect();

    // Limit to recent entries
    if state.tools.running.len() > 10 {
        let len = state.tools.running.len();
        state.tools.running = state.tools.running.split_off(len - 10);
    }
    if state.agents.len() > 5 {
        let len = state.agents.len();
        state.agents = state.agents.split_off(len - 5);
    }
    if state.skills.len() > 3 {
        let len = state.skills.len();
        state.skills = state.skills.split_off(len - 3);
    }

    state
}

// ============================================================================
// Output Formatting
// ============================================================================

fn format_output(state: &TranscriptState) -> String {
    let mut parts: Vec<String> = vec![];

    if let Some(todo_str) = format_todos(&state.todos) {
        parts.push(todo_str);
    }

    if let Some(skill_str) = format_skills(&state.skills) {
        parts.push(skill_str);
    }

    if let Some(agent_str) = format_agents(&state.agents) {
        parts.push(agent_str);
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

            format!("{color}{icon}{NC} {}", s.name)
        })
        .collect();

    Some(format!("{LAVENDER}{ICON_SKILLS}{NC} {}", parts.join(" ")))
}

fn format_todos(todos: &TodoState) -> Option<String> {
    if todos.total == 0 {
        return None;
    }

    let (color, icon) = if todos.done == todos.total {
        (GREEN, ICON_CHECK)
    } else {
        (YELLOW, ICON_SPINNER)
    };

    let text = if let Some(ref current) = todos.current {
        if todos.done < todos.total {
            format!(
                "{LAVENDER}{ICON_TODOS}{NC} {color}{icon}{NC} {current} ({}/{})",
                todos.done, todos.total
            )
        } else {
            format!(
                "{LAVENDER}{ICON_TODOS}{NC} {color}{icon}{NC} All done ({}/{})",
                todos.done, todos.total
            )
        }
    } else {
        format!(
            "{LAVENDER}{ICON_TODOS}{NC} {color}{icon}{NC} {}/{}",
            todos.done, todos.total
        )
    };

    Some(text)
}

fn format_agents(agents: &[AgentEntry]) -> Option<String> {
    if agents.is_empty() {
        return None;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let parts: Vec<String> = agents
        .iter()
        .map(|a| {
            let (color, icon) = match a.status {
                Status::Running => (YELLOW, ICON_SPINNER),
                Status::Completed => (GREEN, ICON_CHECK),
                Status::Error => (RED, ICON_ERROR),
            };

            let elapsed = calculate_elapsed(&a.start_time, &a.end_time, now);
            let elapsed_str = if elapsed > 0 {
                format!(" ({}s)", elapsed)
            } else {
                String::new()
            };

            format!("{color}{icon}{NC} {}{}", a.agent_type, elapsed_str)
        })
        .collect();

    Some(format!("{LAVENDER}{ICON_AGENTS}{NC} {}", parts.join(" ")))
}

fn format_tools(tools: &ToolState) -> Option<String> {
    let mut parts: Vec<String> = vec![];

    let mut completed: Vec<_> = tools.completed.iter().collect();
    completed.sort_by(|a, b| b.1.cmp(a.1));

    for (name, count) in completed.iter().take(5) {
        let suffix = if **count > 1 {
            format!(" ×{}", count)
        } else {
            String::new()
        };
        parts.push(format!("{GREEN}{ICON_CHECK}{NC} {}{}", name, suffix));
    }

    for tool in tools.running.iter().take(3) {
        let target = tool
            .target
            .as_ref()
            .map(|t| format!(" {}", t))
            .unwrap_or_default();
        parts.push(format!("{YELLOW}{ICON_SPINNER}{NC} {}{}", tool.name, target));
    }

    if parts.is_empty() {
        None
    } else {
        Some(format!("{LAVENDER}{ICON_TOOLS}{NC} {}", parts.join(" ")))
    }
}

fn calculate_elapsed(start: &Option<String>, end: &Option<String>, now_secs: u64) -> u64 {
    let start_secs = start
        .as_ref()
        .and_then(|s| parse_timestamp(s))
        .unwrap_or(now_secs);

    let end_secs = end
        .as_ref()
        .and_then(|s| parse_timestamp(s))
        .unwrap_or(now_secs);

    end_secs.saturating_sub(start_secs)
}

fn parse_timestamp(ts: &str) -> Option<u64> {
    let ts = ts.trim_end_matches('Z');
    let parts: Vec<&str> = ts.split('T').collect();
    if parts.len() != 2 {
        return None;
    }

    let date_parts: Vec<u32> = parts[0]
        .split('-')
        .filter_map(|s| s.parse().ok())
        .collect();
    let time_str = parts[1].split('.').next()?;
    let time_parts: Vec<u32> = time_str
        .split(':')
        .filter_map(|s| s.parse().ok())
        .collect();

    if date_parts.len() != 3 || time_parts.len() != 3 {
        return None;
    }

    let days_since_epoch = (date_parts[0] - 1970) as u64 * 365
        + date_parts[1] as u64 * 30
        + date_parts[2] as u64;
    let seconds_in_day =
        time_parts[0] as u64 * 3600 + time_parts[1] as u64 * 60 + time_parts[2] as u64;

    Some(days_since_epoch * 86400 + seconds_in_day)
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: claude-status <transcript_path>");
        std::process::exit(1);
    }

    let path = Path::new(&args[1]);
    if !path.exists() {
        std::process::exit(0);
    }

    let state = parse_transcript(path);
    let output = format_output(&state);

    if !output.is_empty() {
        println!("{}", output);
    }
}
