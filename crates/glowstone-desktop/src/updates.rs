use serde::Deserialize;

const COMMITS_URL: &str =
    "https://api.github.com/repos/q933598-ai/GlowstoneCL/commits?sha=test%2Fglowstonecl&per_page=8";

#[derive(Debug, Clone)]
pub struct UpdateEntry {
    pub message: String,
    pub date: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
struct GithubCommit {
    html_url: String,
    commit: GithubCommitInfo,
}

#[derive(Debug, Deserialize)]
struct GithubCommitInfo {
    message: String,
    author: Option<GithubAuthor>,
}

#[derive(Debug, Deserialize)]
struct GithubAuthor {
    date: String,
}

/// Reads recent public commit messages for the active development branch.
/// This is a changelog check, not a binary self-updater.
pub async fn fetch_recent_changes() -> Result<Vec<UpdateEntry>, String> {
    let client = reqwest::Client::builder()
        .user_agent("GlowstoneCL/0.1.0")
        .build()
        .map_err(|error| format!("Не удалось создать HTTP-клиент обновлений: {error}"))?;

    let response = client
        .get(COMMITS_URL)
        .send()
        .await
        .map_err(|error| format!("Не удалось проверить изменения GitHub: {error}"))?
        .error_for_status()
        .map_err(|error| format!("GitHub не отдал журнал изменений: {error}"))?;

    let commits: Vec<GithubCommit> = response
        .json()
        .await
        .map_err(|error| format!("Не удалось прочитать журнал изменений GitHub: {error}"))?;

    Ok(commits
        .into_iter()
        .map(|commit| UpdateEntry {
            message: commit.commit.message.lines().next().unwrap_or("Без описания").to_owned(),
            date: commit.commit.author.map(|author| author.date).unwrap_or_default(),
            url: commit.html_url,
        })
        .collect())
}
