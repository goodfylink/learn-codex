use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

#[derive(Debug, Default)]
pub struct SkillLoadOutcome {
    pub skills: Vec<SkillMetadata>,
    pub warnings: Vec<String>,
}

pub fn load_skills(root: &Path) -> SkillLoadOutcome {
    let mut outcome = SkillLoadOutcome::default();
    discover_skills(root, &mut outcome);
    outcome.skills.sort_by(|a, b| a.name.cmp(&b.name));
    outcome
}

pub fn render_skills_section(skills: &[SkillMetadata]) -> Option<String> {
    if skills.is_empty() {
        return None;
    }

    let mut lines = vec![
        "## Skills".to_string(),
        "A skill is a local instruction file stored as `SKILL.md`.".to_string(),
        "### Available skills".to_string(),
    ];

    for skill in skills {
        lines.push(format!(
            "- {}: {} (file: {})",
            skill.name,
            skill.description,
            skill.path.display()
        ));
    }

    lines.push("### How to use skills".to_string());
    lines.push(
        "- If the user explicitly mentions a skill with `$skill-name`, you should use that skill for the current turn."
            .to_string(),
    );
    lines.push(
        "- When a skill is selected, read and follow its `SKILL.md` instructions before taking action."
            .to_string(),
    );
    lines.push(
        "- Do not assume a skill is active unless the user explicitly mentions it for the turn."
            .to_string(),
    );

    Some(lines.join("\n"))
}

pub fn collect_explicit_skill_mentions(
    input: &str,
    skills: &[SkillMetadata],
) -> Vec<SkillMetadata> {
    let mentioned_names = extract_skill_mentions(input);
    skills
        .iter()
        .filter(|skill| mentioned_names.iter().any(|name| name == &skill.name))
        .cloned()
        .collect()
}

pub fn build_skill_injection_messages(skills: &[SkillMetadata]) -> (Vec<String>, Vec<String>) {
    let mut messages = Vec::new();
    let mut warnings = Vec::new();

    for skill in skills {
        match fs::read_to_string(&skill.path) {
            Ok(contents) => messages.push(format!(
                "<skill>\n<name>{}</name>\n<path>{}</path>\n{}\n</skill>",
                skill.name,
                skill.path.display(),
                contents
            )),
            Err(error) => warnings.push(format!(
                "failed to load skill {} at {}: {}",
                skill.name,
                skill.path.display(),
                error
            )),
        }
    }

    (messages, warnings)
}

fn discover_skills(root: &Path, outcome: &mut SkillLoadOutcome) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            discover_skills(&path, outcome);
            continue;
        }

        if path.file_name().and_then(|name| name.to_str()) != Some("SKILL.md") {
            continue;
        }

        match parse_skill_file(&path) {
            Ok(skill) => outcome.skills.push(skill),
            Err(error) => {
                outcome
                    .warnings
                    .push(format!("failed to parse {}: {}", path.display(), error))
            }
        }
    }
}

fn parse_skill_file(path: &Path) -> Result<SkillMetadata, String> {
    let contents = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let frontmatter = extract_frontmatter(&contents);
    let fallback_name = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or("skill")
        .to_string();

    let name = frontmatter
        .as_deref()
        .and_then(|frontmatter| frontmatter_value(frontmatter, "name"))
        .unwrap_or(fallback_name);
    let description = frontmatter
        .as_deref()
        .and_then(|frontmatter| frontmatter_value(frontmatter, "description"))
        .unwrap_or_else(|| "No description provided.".to_string());

    Ok(SkillMetadata {
        name,
        description,
        path: path.to_path_buf(),
    })
}

fn extract_frontmatter(contents: &str) -> Option<String> {
    let mut lines = contents.lines();
    if lines.next()? != "---" {
        return None;
    }

    let mut frontmatter = Vec::new();
    for line in lines {
        if line == "---" {
            return Some(frontmatter.join("\n"));
        }
        frontmatter.push(line);
    }

    None
}

fn frontmatter_value(frontmatter: &str, key: &str) -> Option<String> {
    frontmatter.lines().find_map(|line| {
        let (lhs, rhs) = line.split_once(':')?;
        (lhs.trim() == key).then(|| rhs.trim().trim_matches('"').to_string())
    })
}

fn extract_skill_mentions(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut mentions = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'$' {
            index += 1;
            continue;
        }

        let start = index + 1;
        let mut end = start;
        while end < bytes.len() {
            let current = bytes[end] as char;
            if current.is_ascii_alphanumeric() || matches!(current, '-' | '_' | ':') {
                end += 1;
            } else {
                break;
            }
        }

        if end > start {
            mentions.push(text[start..end].to_string());
        }
        index = end;
    }

    mentions
}
