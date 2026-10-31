//! Data model + on-disk storage (`.pumba/`).
//!
//! Layout:
//!   .pumba/board.json     columns, column rules, project rules, sprint
//!   .pumba/tasks/<ID>.md  one task per file (frontmatter + markdown body)
//!   .pumba/AGENTS.md      generated instructions for AI agents
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub type Res<T> = Result<T, Box<dyn std::error::Error>>;

// ---------------------------------------------------------------- config

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Column {
    pub id: String,
    pub name: String,
    /// «Перед работой»: start conditions read before any executor runs (what to check, what to
    /// ask, when to stop). Empty on old boards: nothing to do in that phase.
    #[serde(default)]
    pub before: String,
    /// «Работа»: what the executor does while the card is in this column (the old single rule).
    #[serde(default)]
    pub rule: String,
    /// «После работы»: what to verify after the work, what to do on failure and where to go.
    #[serde(default)]
    pub after: String,
    #[serde(default)]
    pub color: String,
    /// Subagent that executes the column's work (empty = none: the driving agent or a human does it).
    #[serde(default)]
    pub agent: String,
    /// Column id a card moves to after the rule succeeded (empty = stop and report).
    #[serde(default)]
    pub next: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct RuleOption {
    pub id: String,
    pub label: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct RuleSet {
    pub id: String,
    pub title: String,
    pub value: String,
    pub options: Vec<RuleOption>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Config {
    #[serde(default = "one")]
    pub version: u32,
    pub columns: Vec<Column>,
    #[serde(default = "default_models")]
    pub models: Vec<String>,
    #[serde(default = "default_efforts")]
    pub efforts: Vec<String>,
    #[serde(default = "one")]
    pub sprint: u32,
    #[serde(default = "default_rules")]
    pub rules: Vec<RuleSet>,
    #[serde(default)]
    pub custom_rules: String,
    /// Model preselected in the new-card forms (`default` = no preference).
    #[serde(default = "default_word")]
    pub default_model: String,
    /// Effort preselected in the new-card forms (`default` = no preference).
    #[serde(default = "default_word")]
    pub default_effort: String,
    /// Ids of the AI tools the agent pack was installed for (`claude`, ...); the default
    /// selection of the installer prompt.
    #[serde(default)]
    pub tools: Vec<String>,
}

fn one() -> u32 {
    1
}

fn default_word() -> String {
    "default".into()
}

fn default_models() -> Vec<String> {
    ["opus", "sonnet", "haiku"].iter().map(|s| s.to_string()).collect()
}

fn default_efforts() -> Vec<String> {
    ["low", "medium", "high", "xhigh", "max"].iter().map(|s| s.to_string()).collect()
}

fn opt(id: &str, label: &str, text: &str) -> RuleOption {
    RuleOption { id: id.into(), label: label.into(), text: text.into() }
}

fn set(id: &str, title: &str, value: &str, options: Vec<RuleOption>) -> RuleSet {
    RuleSet { id: id.into(), title: title.into(), value: value.into(), options }
}

fn default_rules() -> Vec<RuleSet> {
    vec![
        set("commits", "Стиль коммитов", "conventional", vec![
            opt("conventional", "Conventional Commits", "Пиши сообщения коммитов в виде `type(scope): subject` (feat, fix, docs, refactor, test, chore). Повелительное наклонение, тема короче 72 символов."),
            opt("gitmoji", "Gitmoji", "Начинай каждое сообщение коммита с gitmoji, затем короткая тема в повелительном наклонении."),
            opt("ticket", "Префикс с ID задачи", "Начинай тему каждого коммита с ID задачи, например `T-001: добавить форму входа`."),
            opt("free", "Свободный формат", "Без соглашения о сообщениях коммитов. Пиши коротко и по делу."),
        ]),
        set("branching", "Модель ветвления", "per-task", vec![
            opt("github-flow", "GitHub Flow", "Для каждого изменения создавай ветку от `main`, держи ветки короткоживущими, сливай через pull request."),
            opt("gitflow", "Git Flow", "Используй Git Flow: `feature/*` от `develop`, `release/*`, `hotfix/*` от `main`. Никогда не коммить напрямую в `main` или `develop`."),
            opt("trunk", "Trunk-based", "Небольшие изменения коммить сразу в `main`. Незавершённую работу прячь за флагами."),
            opt("per-task", "Ветка на задачу", "Создавай по одной ветке на задачу с именем `pumba/<task-id>-<slug>`: одна задача, одна ветка."),
        ]),
        set("merge", "Стратегия слияния", "squash", vec![
            opt("squash", "Squash-слияние", "Предпочитай squash-слияние, чтобы каждая задача попадала одним коммитом."),
            opt("merge", "Merge-коммит", "Используй merge-коммиты и сохраняй полную историю ветки."),
            opt("rebase", "Rebase", "Делай rebase на целевую ветку и fast-forward. Держи историю линейной."),
        ]),
        set("tests", "Тестирование", "required", vec![
            opt("required", "Тесты обязательны", "Добавляй или обновляй тесты при каждом изменении поведения и запускай их, прежде чем считать задачу выполненной."),
            opt("tdd", "TDD", "Сначала пиши падающий тест, затем реализацию."),
            opt("optional", "Тесты по желанию", "Тесты необязательны. Добавляй их только для рискованной логики."),
        ]),
        set("autonomy", "Автономность", "ask", vec![
            opt("ask", "Сначала спрашивать", "Перед крупными или разрушительными изменениями (много файлов, удаления, смена зависимостей) изложи план и дождись подтверждения."),
            opt("autonomous", "Автономно", "Работай автономно до завершения. Останавливайся с вопросом, только если не можешь продолжить."),
        ]),
        set("board", "Обновление доски", "auto", vec![
            opt("manual", "Вручную", "Не редактируй `.pumba/` по своей инициативе. Предложи перенос карточки в нужную колонку и дождись ответа пользователя. Прямая просьба пользователя перенести или создать карточку разрешает это сделать."),
            opt("auto", "Агент двигает задачи", "После успешного выполнения работы колонки перенеси карточку в колонку `next` текущей колонки; при неудаче — в колонку, названную в правиле. Правь только строку `column:` и добавляй заметки в раздел `## Ход работы` в конце тела карточки; остальной файл не меняй."),
        ]),
    ]
}

impl Default for Config {
    fn default() -> Self {
        // (id, name, color, agent, next, «Перед работой», «Работа», «После работы»);
        // the same texts are in templates/README.md («Колонки по умолчанию»)
        let col = |(id, name, color, agent, next): (&str, &str, &str, &str, &str), before: &str, rule: &str, after: &str| Column {
            id: id.into(),
            name: name.into(),
            before: before.into(),
            rule: rule.into(),
            after: after.into(),
            color: color.into(),
            agent: agent.into(),
            next: next.into(),
        };
        Config {
            version: 1,
            columns: vec![
                col(("backlog", "Бэклог", "purple", "", ""),
                    "Эта задача в БЭКЛОГЕ: она ещё не запланирована. Код не трогай. Предупреди пользователя, что задача в бэклоге, и спроси, брать ли её в работу. Продолжай, только если пользователь явно ответил «да». Ответ «нет» или ответа нет — остановись, карточку не трогай. Пока работаешь над карточкой: если ты решил что-то сам, пользователь принял дополнительное решение или работа идёт не так, как написано в карточке, всегда добавляй в раздел `## Ход работы` тела карточки очень краткую запись (1–3 строки: что и почему), чтобы след остался; при ручном обновлении доски `.pumba/` не правь, а скажи запись пользователю.",
                    "",
                    "После «да» перенеси карточку в «В работе» (`in-progress`) и продолжай по правилам той колонки; при ручном обновлении доски попроси перенести её."),
                col(("in-progress", "В работе", "orange", "pumba-worker", "review"),
                    "Эта задача В РАБОТЕ: её разрешено выполнять. Исполнителя запускай только по плану `pumba-planner` (шаг 1 работы): план с пометкой «Крупное изменение: да» при правиле «Автономность: Сначала спрашивать» покажи пользователю и до его подтверждения исполнителя не запускай; подтверждения нет — остановись.",
                    "1) `pumba-planner` (режим task) составляет план. 2) Если правило «Тестирование» — «Тесты обязательны» или «TDD», `pumba-tester` (режим tests) пишет тесты до реализации. 3) Исполнитель колонки `pumba-worker` реализует план. 4) Запусти проверки из плана сам. Коммита в этой колонке нет. Эпик: исполнителя не запускай, веди дочерние карточки по их колонкам в порядке из `pumba-planner` (режим epic). Если субагентов нет (другой ИИ-агент), выполни эти шаги сам.",
                    "Успех: исполнитель вернул `ИТОГ: PASS`, проверки зелёные, тесты тестировщика не изменены; для эпика — все дочерние карточки в «Готово». Неудача: свежий `pumba-tester` (режим triage) называет виновника. Код — повтори `pumba-worker` с вердиктом на модели выше (haiku → sonnet → opus). Тест — `pumba-tester` (режим tests) правит тест по вердикту. Окружение — почини один раз по документации проекта. После правки проверь снова. Не помогло на opus, окружение не чинится или нужен ответ человека — запиши причину, перенеси карточку в «Ждёт разработчика» (`on-dev`) и остановись."),
                col(("review", "На ревью", "pink", "pumba-reviewer", "done"),
                    "Эта задача НА РЕВЬЮ: работа сделана и ждёт проверки. Код в этой колонке сам не пиши. Пользователь просит только проверить карточку — сделай ревью и объясни результат, без правок кода, коммита и переноса.",
                    "1) Если риск в плане не «низкий» и правило «Тестирование» не «Тесты по желанию», свежий `pumba-tester` (режим accept) принимает задачу. 2) Исполнитель колонки `pumba-reviewer` проверяет незакоммиченные правки задачи от точки старта; для эпика — все коммиты эпика.",
                    "Приёмка вернула `ИТОГ: принято` (если была) и ревью — `ИТОГ: одобрено`: `pumba-committer` коммитит файлы задачи одним коммитом по правилу «Стиль коммитов»; успех — коммиттер вернул `ИТОГ: закоммичено`. Коммит упал на хуке — перенеси в «Ждёт разработчика» (`on-dev`). Замечания «блокирует» или «стоит поправить»: верни карточку в «В работе» (`in-progress`) и передай замечания исполнителю дословно; при втором возврате из ревью за прогон вместо этого перенеси в `on-dev`. «Мелочи» карточку не возвращают, они идут в отчёт. Эпик: замечания исправляет `pumba-worker` прямо здесь, затем проверки, повторное ревью новых правок и отдельный коммит правок; не больше двух раундов."),
                col(("on-dev", "Ждёт разработчика", "red", "", ""),
                    "Эта задача ЖДЁТ РАЗРАБОТЧИКА: агент остановился и ждёт решения человека. Причина — в последней записи раздела «Ход работы» карточки или в последнем отчёте. Решения нет — не продолжай работу и не меняй код: напомни пользователю, какое решение нужно, и остановись. Правки в рабочей копии не откатывай.",
                    "",
                    "Пользователь дал решение и попросил продолжить — перенеси карточку в «В работе» (`in-progress`) и передай решение исполнителю."),
                col(("done", "Готово", "green", "", ""),
                    "Эта задача ГОТОВА: по доске она выполнена, код прошёл ревью и закоммичен. Ничего не меняй. Скажи пользователю, что задача уже готова, и назови карточки, которые она разблокирует (колонка `blocks` индекса). Пользователь не просил проверку — предложи проверить результат по коду и тестам и остановись. Новая работа по этой теме — отдельная карточка.",
                    "Только по просьбе пользователя: проверь результат — `pumba-reviewer` и проверки проекта, без правок кода и без переноса.",
                    ""),
            ],
            models: default_models(),
            efforts: default_efforts(),
            sprint: 1,
            rules: default_rules(),
            custom_rules: String::new(),
            default_model: default_word(),
            default_effort: default_word(),
            tools: Vec::new(),
        }
    }
}

// ----------------------------------------------------------------- tasks

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Epic,
    Task,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Epic => "epic",
            Kind::Task => "task",
        }
    }
    pub fn prefix(self) -> &'static str {
        match self {
            Kind::Epic => "E-",
            Kind::Task => "T-",
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Task {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    pub column: String,
    pub parent: Option<String>,
    pub model: String,
    pub effort: String,
    pub sprint: Option<u32>,
    pub hidden: bool,
    pub order: i64,
    /// Ids of cards this one depends on; blocked until they all sit in the last column.
    pub depends: Vec<String>,
    /// Soft delete: the card sits in the trash. The file is never removed from disk.
    pub deleted: bool,
    /// Unix seconds when the card was moved to the trash.
    pub deleted_at: Option<u64>,
    pub body: String,
}

impl Task {
    pub fn to_md(&self) -> String {
        let one_line = |s: &str| s.replace(['\r', '\n'], " ");
        let mut s = String::from("---\n");
        s += &format!("id: {}\n", self.id);
        s += &format!("type: {}\n", self.kind.as_str());
        s += &format!("title: {}\n", one_line(&self.title));
        s += &format!("column: {}\n", self.column);
        if let Some(p) = &self.parent {
            s += &format!("parent: {p}\n");
        }
        s += &format!("model: {}\n", self.model);
        s += &format!("effort: {}\n", self.effort);
        if let Some(n) = self.sprint {
            s += &format!("sprint: {n}\n");
        }
        s += &format!("hidden: {}\n", self.hidden);
        s += &format!("order: {}\n", self.order);
        if !self.depends.is_empty() {
            s += &format!("depends: {}\n", self.depends.join(", "));
        }
        if self.deleted {
            s += "deleted: true\n";
            if let Some(at) = self.deleted_at {
                s += &format!("deleted_at: {at}\n");
            }
        }
        s += "---\n\n";
        s += self.body.trim_end();
        s += "\n";
        s
    }

    pub fn from_md(text: &str, stem: &str) -> Option<Task> {
        // an editor or a Windows tool may prepend a BOM and use CRLF; both are accepted
        let text = text.strip_prefix('\u{feff}').unwrap_or(text).replace("\r\n", "\n");
        let rest = text.strip_prefix("---\n")?;
        let end = rest.find("\n---")?;
        let head = &rest[..end];
        let body = rest[end + 4..].trim_start_matches('\n').trim_end().to_string();
        let mut t = Task {
            id: stem.to_string(),
            kind: Kind::Task,
            title: String::new(),
            column: String::new(),
            parent: None,
            model: "default".into(),
            effort: "default".into(),
            sprint: None,
            hidden: false,
            order: 0,
            depends: Vec::new(),
            deleted: false,
            deleted_at: None,
            body,
        };
        for line in head.lines() {
            let Some((k, v)) = line.split_once(':') else { continue };
            let v = v.trim();
            match k.trim() {
                "type" => t.kind = if v == "epic" { Kind::Epic } else { Kind::Task },
                "title" => t.title = v.to_string(),
                "column" => t.column = v.to_string(),
                "parent" if !v.is_empty() => t.parent = Some(v.to_string()),
                "model" if !v.is_empty() => t.model = v.to_string(),
                "effort" if !v.is_empty() => t.effort = v.to_string(),
                "sprint" => t.sprint = v.parse().ok(),
                "hidden" => t.hidden = v == "true",
                "order" => t.order = v.parse().unwrap_or(0),
                "deleted" => t.deleted = v == "true",
                "deleted_at" => t.deleted_at = v.parse().ok(),
                "depends" => {
                    t.depends = v.split(',').map(str::trim).filter(|d| !d.is_empty()).map(String::from).collect()
                }
                _ => {}
            }
        }
        Some(t)
    }

    /// Three-way merge for a card edited on disk behind our back. `base` is the card as this app
    /// last loaded or wrote it, `disk` is the file now. A field this app did not change since `base`
    /// (`self.f == base.f`) takes the value from disk; a field it did change keeps ours.
    pub fn merge_disk(&mut self, base: &Task, disk: &Task) {
        macro_rules! field {
            ($($f:ident),*) => { $( if self.$f == base.$f { self.$f = disk.$f.clone(); } )* };
        }
        field!(kind, title, column, parent, model, effort, sprint, hidden, order, depends, deleted, deleted_at, body);
    }
}

/// `dd.mm HH:MM` (UTC) from unix seconds, plain calendar arithmetic (no deps).
pub fn fmt_utc(secs: u64) -> String {
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{day:02}.{month:02} {:02}:{:02}", rem / 3_600, rem % 3_600 / 60)
}

// ----------------------------------------------------------------- store

/// Write `data` to `path` through a temporary file in the same folder and a rename, so a reader
/// (the TUI, an agent) never sees a half-written file. `fs::rename` replaces an existing file on
/// Windows too; if it still refuses, the old file is removed and the rename is tried once more.
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    // the suffix is not `.md`, so a half-finished temp file is never taken for a card
    let tmp = path.with_file_name(format!("{name}.tmp{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    fs::write(&tmp, data)?;
    if fs::rename(&tmp, path).is_ok() {
        return Ok(());
    }
    let _ = fs::remove_file(path);
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

pub struct Store {
    pub dir: PathBuf,
}

/// Result of `Store::load_full`.
pub struct Loaded {
    pub cfg: Config,
    pub tasks: Vec<Task>,
    /// File names (`T-007.md`) of cards that could not be read or parsed, sorted.
    pub unreadable: Vec<String>,
    /// Raw file text of every loaded card by id.
    pub raws: HashMap<String, String>,
}

impl Store {
    pub fn new(root: &Path) -> Self {
        Store { dir: root.join(".pumba") }
    }

    /// Walk up from `start` looking for a directory that contains `.pumba`.
    pub fn find(start: &Path) -> Option<PathBuf> {
        let mut cur = Some(start);
        while let Some(d) = cur {
            if d.join(".pumba").join("board.json").is_file() {
                return Some(d.to_path_buf());
            }
            cur = d.parent();
        }
        None
    }

    fn tasks_dir(&self) -> PathBuf {
        self.dir.join("tasks")
    }

    pub fn init(&self) -> Res<()> {
        fs::create_dir_all(self.tasks_dir())?;
        let cfg = Config::default();
        self.save_config(&cfg)?;
        self.write_agents(&cfg, &[])?;
        Ok(())
    }

    /// Config and cards; cards that cannot be read are skipped (see `load_full` for their names).
    #[allow(dead_code)]
    pub fn load(&self) -> Res<(Config, Vec<Task>)> {
        let l = self.load_full()?;
        Ok((l.cfg, l.tasks))
    }

    /// Like `load`, plus the names of card files that failed to read or parse and the raw text
    /// of every loaded card (the base for merging with later external edits).
    pub fn load_full(&self) -> Res<Loaded> {
        let raw = fs::read_to_string(self.dir.join("board.json"))?;
        let cfg: Config = serde_json::from_str(&raw).map_err(|e| format!("board.json: {e}"))?;
        let mut tasks = Vec::new();
        let mut unreadable = Vec::new();
        let mut raws = HashMap::new();
        if let Ok(rd) = fs::read_dir(self.tasks_dir()) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) != Some("md") || !p.is_file() {
                    continue;
                }
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                match fs::read_to_string(&p).ok().and_then(|text| Task::from_md(&text, &stem).map(|t| (text, t))) {
                    Some((text, t)) => {
                        raws.insert(stem, text);
                        tasks.push(t);
                    }
                    None => unreadable.push(name),
                }
            }
        }
        tasks.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        unreadable.sort();
        Ok(Loaded { cfg, tasks, unreadable, raws })
    }

    /// Current text of a card file (None when missing or unreadable).
    pub fn read_task_raw(&self, id: &str) -> Option<String> {
        fs::read_to_string(self.tasks_dir().join(format!("{id}.md"))).ok()
    }

    pub fn task_exists(&self, id: &str) -> bool {
        self.tasks_dir().join(format!("{id}.md")).exists()
    }

    /// Tool ids saved in board.json (empty when the board is missing or unreadable).
    pub fn saved_tools(&self) -> Vec<String> {
        fs::read_to_string(self.dir.join("board.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<Config>(&raw).ok())
            .map(|c| c.tools)
            .unwrap_or_default()
    }

    /// Record the chosen tool ids in board.json (the rest of the file is kept as parsed).
    pub fn set_tools(&self, ids: &[String]) -> Res<()> {
        let raw = fs::read_to_string(self.dir.join("board.json"))?;
        let mut cfg: Config = serde_json::from_str(&raw).map_err(|e| format!("board.json: {e}"))?;
        if cfg.tools != ids {
            cfg.tools = ids.to_vec();
            self.save_config(&cfg)?;
        }
        Ok(())
    }

    pub fn save_config(&self, cfg: &Config) -> Res<()> {
        write_atomic(&self.dir.join("board.json"), (serde_json::to_string_pretty(cfg)? + "\n").as_bytes())?;
        Ok(())
    }

    pub fn save_task(&self, t: &Task) -> Res<()> {
        fs::create_dir_all(self.tasks_dir())?;
        write_atomic(&self.tasks_dir().join(format!("{}.md", t.id)), t.to_md().as_bytes())?;
        Ok(())
    }

    pub fn write_agents(&self, cfg: &Config, tasks: &[Task]) -> Res<()> {
        write_atomic(&self.dir.join("AGENTS.md"), agents_md(cfg, tasks).as_bytes())?;
        Ok(())
    }

    /// Cheap change detector for external edits (agents, git checkout, ...).
    pub fn signature(&self) -> u128 {
        let mtime = |p: &Path| -> u128 {
            fs::metadata(p)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        };
        let mut sig = mtime(&self.dir.join("board.json"));
        if let Ok(rd) = fs::read_dir(self.tasks_dir()) {
            for e in rd.flatten() {
                sig = sig.wrapping_add(mtime(&e.path())).wrapping_add(1);
            }
        }
        sig
    }
}

// ------------------------------------------------------------ AGENTS.md

pub fn agents_md(cfg: &Config, tasks: &[Task]) -> String {
    let mut s = String::new();
    s += "# Доска Pumba: инструкции для ИИ-агентов\n\n";
    s += "<!-- Создано pumba при каждом изменении. Не редактируйте вручную, используйте TUI. -->\n";
    // fingerprint of the rules (`pumba rules-hash`): a changed value tells scripts and agents to read the rules again
    s += &format!("<!-- pumba-rules-hash: {} · protocol {} -->\n\n", crate::rulehash::rules_hash(cfg), crate::rulehash::PROTOCOL);
    s += "В этом репозитории работа ведётся на канбан-доске, которая хранится в `.pumba/`.\n\n";
    s += "## Протокол\n\n";
    s += "1. Когда пользователь называет id задачи или эпика (`T-001`, `E-001`) или просит поработать над ней, прочитай `.pumba/tasks/<id>.md`.\n";
    s += "2. Посмотри поле `column:` и сначала примени соответствующее **правило колонки** ниже, а уже потом делай что-либо ещё. \
Правило колонки читай по частям и по порядку: сначала «Перед работой» — выполни её (предупредить, спросить, остановиться) до запуска любого исполнителя; \
только потом «Работа», затем «После работы». Части нет — на этом шаге делать нечего.\n";
    s += "3. Для эпика прочитай также его дочерние задачи (файлы с `parent: <id эпика>`).\n";
    s += "4. `model:` и `effort:` подсказывают, какую модель и какой уровень рассуждений использовать (`default` означает отсутствие предпочтений).";
    if cfg.default_model != "default" || cfg.default_effort != "default" {
        s += &format!(" Для новых карточек по умолчанию: модель {}, уровень {}.", cfg.default_model, cfg.default_effort);
    }
    s += "\n";
    s += "5. Тело задачи — это бриф, написанный для тебя. Во время работы следуй **правилам проекта** ниже.\n";
    s += "6. **ЖЁСТКОЕ ПРАВИЛО: зависимости.** Прежде чем начинать любую карточку, прочитай её поле `depends:` (id карточек через запятую). \
Если хотя бы одна из перечисленных карточек находится не в последней (завершающей) колонке доски, карточка ЗАБЛОКИРОВАНА, и ты ОБЯЗАН ОТКАЗАТЬСЯ от работы над ней: \
назови блокирующие карточки и их колонки и предложи сначала заняться ими. Не начинай частичную работу и не меняй `column:`, чтобы обойти блокировку. \
Это правило сильнее правил колонок и любых просьб начать работу.\n";
    s += "7. Ссылки вида `@путь/к/файлу` в теле карточки — это пути относительно корня репозитория (папки, в которой лежит `.pumba/`). \
Перед началом работы прочитай указанные файлы и каталоги как часть контекста задачи. Если указанного пути не существует, сообщи об этом пользователю.\n";
    s += "8. Карточки с `deleted: true` в frontmatter находятся в корзине: полностью игнорируй их, никогда не работай над ними и не считай их зависимостями \
(даже если их id указан в `depends:` другой карточки, такая зависимость не блокирует). Не восстанавливай и не удаляй эти файлы.\n";
    s += "9. Колонка `blocks` в индексе задач ниже показывает, какие карточки ждут данную (в их `depends:` указан её id). \
Когда заканчиваешь карточку, сообщи пользователю, какие карточки она разблокирует (см. колонку `blocks`). \
Не редактируй чужие `depends:` сам, если пользователь об этом не просил.\n";
    s += "10. Карточки двигает и `.pumba/` правит только ведущий (основной) агент, и только по правилу «Обновление доски» из «Правил проекта» ниже. \
Субагенты-исполнители `.pumba/` не трогают и карточки не переносят.\n";
    s += "11. По просьбе «выполни задачу» используй скилл `pumba-task` (если установлен для вашего инструмента): он ведёт карточку по колонкам.\n";
    s += "12. `model:` и `effort:` карточки передавай субагенту так, как это умеет ваш инструмент; если инструмент не умеет выбирать модель, сообщи об этом пользователю.\n";
    s += "13. Пока работаешь над карточкой: если ты решил что-то сам, пользователь принял дополнительное решение или работа идёт не так, как написано в карточке, \
всегда добавляй в раздел `## Ход работы` тела карточки очень краткую запись (1–3 строки: что и почему), чтобы след остался. \
При ручном обновлении доски `.pumba/` не правь, а скажи запись пользователю.\n\n";

    s += "## Правила колонок\n\n";
    if let Some(last) = cfg.columns.last() {
        s += &format!("Завершающая колонка: «{}» (`{}`).\n\n", last.name, last.id);
    }
    for c in &cfg.columns {
        s += &format!("### {} (`{}`)\n\n", c.name, c.id);
        // the three parts in reading order; empty parts are left out
        if !c.before.trim().is_empty() {
            s += &format!("**Перед работой:** {}\n\n", c.before.trim());
        }
        let (agent, rule) = (c.agent.trim(), c.rule.trim());
        match (agent.is_empty(), rule.is_empty()) {
            (false, false) => s += &format!("**Работа** (исполнитель: `{agent}`): {rule}\n\n"),
            (false, true) => s += &format!("**Работа** (исполнитель: `{agent}`).\n\n"),
            (true, false) => {
                s += &format!("**Работа** (исполнитель: нет, работу колонки делает сам агент-драйвер или человек): {rule}\n\n")
            }
            (true, true) => {}
        }
        let then = if c.next.trim().is_empty() {
            "После успеха: остановиться и отчитаться; карточку переносит человек.".to_string()
        } else {
            let name = cfg.columns.iter().find(|o| o.id == c.next).map_or(c.next.as_str(), |o| o.name.as_str());
            format!("После успеха: переместить в «{}» (`{}`).", name, c.next)
        };
        match c.after.trim() {
            "" => s += &format!("**После работы:** {then}\n\n"),
            after => s += &format!("**После работы:** {after} {then}\n\n"),
        }
    }

    s += "## Правила проекта\n\n";
    for r in &cfg.rules {
        if let Some(o) = r.options.iter().find(|o| o.id == r.value) {
            s += &format!("- **{}: {}.** {}\n", r.title, o.label, o.text);
        }
    }
    s += "\n";
    if !cfg.custom_rules.trim().is_empty() {
        s += "### Свои правила\n\n";
        s += cfg.custom_rules.trim();
        s += "\n\n";
    }

    s += &format!("## Индекс задач\n\nТекущий спринт: {}\n\n", cfg.sprint);
    s += "| id | тип | колонка | спринт | depends | blocks | название |\n|---|---|---|---|---|---|---|\n";
    for t in tasks.iter().filter(|t| !t.deleted) {
        // derived from the other cards' `depends:`; nothing is stored for it
        let mut blocks: Vec<&str> =
            tasks.iter().filter(|o| !o.deleted && o.depends.contains(&t.id)).map(|o| o.id.as_str()).collect();
        blocks.sort_unstable();
        s += &format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            t.id,
            t.kind.as_str(),
            t.column,
            t.sprint.map(|n| n.to_string()).unwrap_or_else(|| "-".into()),
            if t.depends.is_empty() { "-".to_string() } else { t.depends.join(", ") },
            if blocks.is_empty() { "-".to_string() } else { blocks.join(", ") },
            t.title.replace('|', "\\|")
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_roundtrip() {
        let t = Task {
            id: "T-001".into(),
            kind: Kind::Task,
            title: "Fix: login".into(),
            column: "backlog".into(),
            parent: Some("E-001".into()),
            model: "opus".into(),
            effort: "high".into(),
            sprint: Some(2),
            hidden: true,
            order: 4,
            depends: vec!["T-002".into(), "E-003".into()],
            deleted: false,
            deleted_at: None,
            body: "line one\n\n---\nline two".into(),
        };
        let back = Task::from_md(&t.to_md(), "T-001").unwrap();
        assert_eq!(back.title, "Fix: login");
        assert_eq!(back.parent.as_deref(), Some("E-001"));
        assert_eq!(back.sprint, Some(2));
        assert!(back.hidden);
        assert_eq!(back.body, t.body);
        assert_eq!(back.depends, ["T-002", "E-003"]);
        assert!(t.to_md().contains("depends: T-002, E-003\n"));
        // empty list: key not written; sloppy input is trimmed, empties skipped
        let mut none = t.clone();
        none.depends.clear();
        assert!(!none.to_md().contains("depends"));
        let loose = "---\ntitle: x\ncolumn: backlog\ndepends:  T-001 ,, T-002 ,\n---\n\nbody\n";
        assert_eq!(Task::from_md(loose, "T-009").unwrap().depends, ["T-001", "T-002"]);
        // not trashed: neither key is written
        assert!(!t.to_md().contains("deleted"));
    }

    #[test]
    fn deleted_roundtrip() {
        let mut t = Task::from_md("---\ntitle: x\ncolumn: backlog\n---\n\nbody\n", "T-001").unwrap();
        assert!(!t.deleted && t.deleted_at.is_none());
        t.deleted = true;
        t.deleted_at = Some(1_793_479_260);
        let md = t.to_md();
        assert!(md.contains("deleted: true\n") && md.contains("deleted_at: 1793479260\n"));
        let back = Task::from_md(&md, "T-001").unwrap();
        assert!(back.deleted);
        assert_eq!(back.deleted_at, Some(1_793_479_260));
        // flag without a timestamp is fine
        t.deleted_at = None;
        let back = Task::from_md(&t.to_md(), "T-001").unwrap();
        assert!(back.deleted && back.deleted_at.is_none());
        assert!(!t.to_md().contains("deleted_at"));
    }

    #[test]
    fn utc_format() {
        assert_eq!(fmt_utc(0), "01.01 00:00");
        assert_eq!(fmt_utc(1_793_479_260), "31.10 20:41");
        assert_eq!(fmt_utc(1_709_251_140), "29.02 23:59"); // leap day
    }

    #[test]
    fn agents_md_excludes_trashed() {
        let mk = |id: &str, deleted: bool| {
            let mut t = Task::from_md("---\ntitle: Name\ncolumn: backlog\n---\n", id).unwrap();
            t.deleted = deleted;
            t
        };
        let md = agents_md(&Config::default(), &[mk("T-001", false), mk("T-002", true)]);
        assert!(md.contains("| T-001 |") && !md.contains("| T-002 |"));
        assert!(md.contains("8. Карточки с `deleted: true`") && md.contains("не считай их зависимостями"));
    }

    #[test]
    fn agents_md_has_refuse_rule() {
        let md = agents_md(&Config::default(), &[]);
        assert!(md.contains("ЖЁСТКОЕ ПРАВИЛО") && md.contains("`depends:`") && md.contains("ОБЯЗАН ОТКАЗАТЬСЯ"));
        assert!(md.contains("не меняй `column:`"));
        assert!(md.contains("7. Ссылки вида `@путь/к/файлу`") && md.contains("относительно корня репозитория"));
        assert!(md.contains("| id | тип | колонка | спринт | depends | blocks | название |"));
        assert!(md.contains("9. Колонка `blocks`") && md.contains("разблокирует") && md.contains("Не редактируй чужие `depends:`"));
    }

    #[test]
    fn agents_md_has_rules_hash_line() {
        let mut cfg = Config::default();
        let line = format!("<!-- pumba-rules-hash: {} · protocol {} -->", crate::rulehash::rules_hash(&cfg), crate::rulehash::PROTOCOL);
        let md = agents_md(&cfg, &[]);
        assert!(md.contains(&line), "{md}");
        // near the top, before the protocol
        assert!(md.find(&line).unwrap() < md.find("## Протокол").unwrap());
        // follows the rules, not the card index
        cfg.columns[1].after.push_str(" ещё");
        assert!(!agents_md(&cfg, &[]).contains(&line));
    }

    #[test]
    fn write_atomic_replaces_and_leaves_no_temp() {
        let dir = std::env::temp_dir().join(format!("pumba-atomic-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("x.md");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "two");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_defaults_for_old_board_json() {
        // an old board.json without the project defaults loads unchanged
        let old = r#"{"columns":[{"id":"a","name":"A"}],"sprint":3}"#;
        let cfg: Config = serde_json::from_str(old).unwrap();
        assert_eq!((cfg.default_model.as_str(), cfg.default_effort.as_str(), cfg.sprint), ("default", "default", 3));
        let mut cfg = cfg;
        cfg.default_model = "opus".into();
        cfg.default_effort = "high".into();
        let back: Config = serde_json::from_str(&serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        assert_eq!((back.default_model.as_str(), back.default_effort.as_str()), ("opus", "high"));
    }

    #[test]
    fn config_tools_default_empty_and_roundtrip() {
        let cfg: Config = serde_json::from_str(r#"{"columns":[{"id":"a","name":"A"}]}"#).unwrap();
        assert!(cfg.tools.is_empty());
        assert!(Config::default().tools.is_empty());
        let mut cfg = cfg;
        cfg.tools = vec!["claude".into(), "codex".into()];
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        assert!(json.contains("\"tools\""));
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tools, ["claude", "codex"]);
        assert_eq!(back, cfg);
    }

    #[test]
    fn agents_md_mentions_defaults_only_when_set() {
        let mut cfg = Config::default();
        assert!(!agents_md(&cfg, &[]).contains("Для новых карточек по умолчанию"));
        cfg.default_model = "opus".into();
        let md = agents_md(&cfg, &[]);
        assert!(md.contains("4. `model:` и `effort:`") && md.contains("Для новых карточек по умолчанию: модель opus, уровень default."), "{md}");
        cfg.default_effort = "max".into();
        assert!(agents_md(&cfg, &[]).contains("модель opus, уровень max."));
    }

    #[test]
    fn agents_md_blocks_column() {
        let mk = |id: &str, deps: &[&str], deleted: bool| {
            let mut t = Task::from_md("---\ntitle: Name\ncolumn: backlog\n---\n", id).unwrap();
            t.depends = deps.iter().map(|d| d.to_string()).collect();
            t.deleted = deleted;
            t
        };
        let tasks = [mk("T-001", &[], false), mk("T-002", &["T-001"], false), mk("T-003", &["T-001", "T-002"], false), mk("T-004", &["T-001"], true)];
        let md = agents_md(&Config::default(), &tasks);
        // trashed dependents are not listed
        assert!(md.contains("| T-001 | task | backlog | - | - | T-002, T-003 | Name |"), "{md}");
        assert!(md.contains("| T-002 | task | backlog | - | T-001 | T-003 | Name |"), "{md}");
        assert!(md.contains("| T-003 | task | backlog | - | T-001, T-002 | - | Name |"), "{md}");
        assert!(!md.contains("| T-004 |"));
    }

    // ------------------------------------------------ pipeline columns (agent / next)

    #[test]
    fn old_column_json_gets_empty_agent_and_next() {
        let old = r#"{"columns":[{"id":"a","name":"A","rule":"r","color":"red"},{"id":"b","name":"B"}]}"#;
        let cfg: Config = serde_json::from_str(old).unwrap();
        assert!(cfg.columns.iter().all(|c| c.agent.is_empty() && c.next.is_empty()));
        assert_eq!((cfg.columns[0].rule.as_str(), cfg.columns[0].color.as_str()), ("r", "red"));
        // the new keys survive a round trip
        let mut cfg = cfg;
        cfg.columns[0].agent = "pumba-worker".into();
        cfg.columns[0].next = "b".into();
        let back: Config = serde_json::from_str(&serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        assert_eq!((back.columns[0].agent.as_str(), back.columns[0].next.as_str()), ("pumba-worker", "b"));
        assert_eq!(back, cfg);
    }

    #[test]
    fn old_column_json_keeps_rule_as_work_with_empty_before_and_after() {
        // an old board: the single `rule` is the «Работа» part, the other two parts are empty
        let old = r#"{"columns":[{"id":"a","name":"A","rule":"do it","agent":"bot","next":"b"},{"id":"b","name":"B"}]}"#;
        let cfg: Config = serde_json::from_str(old).unwrap();
        let a = &cfg.columns[0];
        assert_eq!((a.before.as_str(), a.rule.as_str(), a.after.as_str()), ("", "do it", ""));
        assert!(cfg.columns[1].before.is_empty() && cfg.columns[1].rule.is_empty() && cfg.columns[1].after.is_empty());
        // AGENTS.md of an old board shows only «Работа» and the closing line
        let md = agents_md(&cfg, &[]);
        assert!(md.contains("### A (`a`)\n\n**Работа** (исполнитель: `bot`): do it\n\n**После работы:** После успеха: переместить в «B» (`b`).\n\n"), "{md}");
        assert!(!md.contains("**Перед работой:**"));
        // the three parts survive a round trip under their own keys
        let mut cfg = cfg;
        cfg.columns[0].before = "check".into();
        cfg.columns[0].after = "verify".into();
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        assert!(json.contains("\"before\": \"check\"") && json.contains("\"rule\": \"do it\"") && json.contains("\"after\": \"verify\""), "{json}");
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn default_columns_form_the_pipeline() {
        let cfg = Config::default();
        let ids: Vec<&str> = cfg.columns.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["backlog", "in-progress", "review", "on-dev", "done"]);
        let names: Vec<&str> = cfg.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Бэклог", "В работе", "На ревью", "Ждёт разработчика", "Готово"]);
        let colors: Vec<&str> = cfg.columns.iter().map(|c| c.color.as_str()).collect();
        assert_eq!(colors, ["purple", "orange", "pink", "red", "green"]);
        // every `next` is empty or another existing column; `done` is last and stops
        for c in &cfg.columns {
            assert!(c.next.is_empty() || (c.next != c.id && ids.contains(&c.next.as_str())), "{}", c.id);
            // every column starts with its guard
            assert!(!c.before.trim().is_empty(), "{}", c.id);
        }
        let col = |id: &str| cfg.columns.iter().find(|c| c.id == id).unwrap();
        assert_eq!((col("in-progress").agent.as_str(), col("in-progress").next.as_str()), ("pumba-worker", "review"));
        assert_eq!((col("review").agent.as_str(), col("review").next.as_str()), ("pumba-reviewer", "done"));
        for id in ["backlog", "on-dev", "done"] {
            assert!(col(id).agent.is_empty() && col(id).next.is_empty(), "{id}");
        }
        assert_eq!(cfg.columns.last().unwrap().id, "done");
        // the agents named by the columns are shipped in the template pack
        let agents = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/agents");
        for c in cfg.columns.iter().filter(|c| !c.agent.is_empty()) {
            assert!(agents.join(format!("{}.md", c.agent)).is_file(), "{}", c.agent);
        }
        // the «После работы» texts name the columns they move to
        assert!(col("backlog").after.contains("(`in-progress`)"));
        assert!(col("review").after.contains("(`on-dev`)") && col("review").after.contains("(`in-progress`)"));
        assert!(col("in-progress").after.contains("(`on-dev`)"));
        assert!(col("on-dev").after.contains("(`in-progress`)"));
    }

    #[test]
    fn default_columns_split_into_before_work_after() {
        let cfg = Config::default();
        let col = |id: &str| cfg.columns.iter().find(|c| c.id == id).unwrap();
        // backlog: the guard (warn + ask, stop without «да») is in «Перед работой», no work
        let b = col("backlog");
        assert!(b.before.contains("БЭКЛОГЕ") && b.before.contains("спроси") && b.before.contains("«да»") && b.before.contains("остановись"), "{}", b.before);
        assert!(b.rule.is_empty());
        assert!(b.after.contains("«да»") && b.after.contains("(`in-progress`)") && !b.after.contains("спроси"), "{}", b.after);
        // in-progress: autonomy guard first; planner → tester → worker → checks; checks → next or on-dev
        let p = col("in-progress");
        assert!(p.before.contains("Автономность: Сначала спрашивать") && p.before.contains("Крупное изменение: да"), "{}", p.before);
        let [pl, te, wo, ch] = ["`pumba-planner`", "`pumba-tester`", "`pumba-worker`", "проверки из плана"].map(|w| p.rule.find(w).unwrap());
        assert!(pl < te && te < wo && wo < ch, "{}", p.rule);
        assert!(p.after.contains("`ИТОГ: PASS`") && p.after.contains("режим triage") && p.after.contains("haiku → sonnet → opus"), "{}", p.after);
        assert!(!p.rule.contains("ИТОГ: PASS") && !p.before.contains("triage"));
        // review: acceptance and the reviewer are the work; the commit and the returns come after
        let r = col("review");
        assert!(r.before.contains("Код в этой колонке сам не пиши") && r.before.contains("только проверить"), "{}", r.before);
        assert!(r.rule.contains("режим accept") && r.rule.contains("`pumba-reviewer`") && !r.rule.contains("pumba-committer"), "{}", r.rule);
        assert!(r.after.contains("`pumba-committer`") && r.after.contains("ИТОГ: закоммичено") && r.after.contains("«Мелочи»"), "{}", r.after);
        // on-dev: stop and report first, continue only after a decision
        let d = col("on-dev");
        assert!(d.before.contains("ЖДЁТ РАЗРАБОТЧИКА") && d.before.contains("остановись") && d.before.contains("не откатывай"), "{}", d.before);
        assert!(d.rule.is_empty() && d.after.contains("решение"));
        // done: report it is done, offer a check, stop; the check itself is the (optional) work
        let g = col("done");
        assert!(g.before.contains("ГОТОВА") && g.before.contains("предложи проверить") && g.before.contains("остановись") && g.before.contains("`blocks`"), "{}", g.before);
        assert!(g.rule.contains("`pumba-reviewer`") && g.rule.contains("без переноса") && g.after.is_empty());
    }

    #[test]
    fn readme_json_matches_default_columns() {
        // templates/README.md carries the pasteable JSON of the default columns: it must match exactly
        let readme = include_str!("../../../templates/README.md").replace("\r\n", "\n");
        let start = readme.find("```json\n").expect("json block") + "```json\n".len();
        let end = start + readme[start..].find("```").unwrap();
        let cols: Vec<Column> = serde_json::from_str(&readme[start..end]).expect("README json parses");
        assert_eq!(cols, Config::default().columns);
    }

    #[test]
    fn default_rule_values_and_board_texts() {
        let cfg = Config::default();
        let val = |id: &str| cfg.rules.iter().find(|r| r.id == id).unwrap().value.clone();
        assert_eq!(
            [val("commits"), val("branching"), val("merge"), val("tests"), val("autonomy"), val("board")],
            ["conventional", "per-task", "squash", "required", "ask", "auto"]
        );
        // every rule keeps its ids and its value is one of its options
        for r in &cfg.rules {
            assert!(r.options.iter().any(|o| o.id == r.value), "{}", r.id);
        }
        let board = cfg.rules.iter().find(|r| r.id == "board").unwrap();
        let text = |id: &str| board.options.iter().find(|o| o.id == id).unwrap().text.clone();
        let auto = text("auto");
        assert!(auto.contains("`next`") && auto.contains("`column:`") && auto.contains("## Ход работы"), "{auto}");
        let manual = text("manual");
        assert!(manual.contains("Не редактируй `.pumba/`") && manual.contains("дождись ответа") && manual.contains("Прямая просьба"), "{manual}");
        assert_eq!(board.options.len(), 2);
    }

    #[test]
    fn agents_md_column_lines() {
        let md = agents_md(&Config::default(), &[]);
        let head = md.find("Завершающая колонка: «Готово» (`done`).").expect("last column line");
        assert!(head < md.find("### Бэклог (`backlog`)").unwrap());
        let cfg = Config::default();
        let c = |id: &str| cfg.columns.iter().find(|c| c.id == id).unwrap();
        // in-progress: all three parts, in reading order, with the executor and the `next` line
        let ip = c("in-progress");
        assert!(md.contains(&format!(
            "### В работе (`in-progress`)\n\n**Перед работой:** {}\n\n**Работа** (исполнитель: `pumba-worker`): {}\n\n**После работы:** {} После успеха: переместить в «На ревью» (`review`).\n\n",
            ip.before, ip.rule, ip.after
        )), "{md}");
        assert!(md.contains("**Работа** (исполнитель: `pumba-reviewer`): 1) Если риск"));
        assert!(md.contains("закоммичено`. Коммит упал на хуке") && md.contains("не больше двух раундов. После успеха: переместить в «Готово» (`done`).\n\n"));
        // backlog: no work part, the guard comes first, the stop line closes «После работы»
        let stop = "После успеха: остановиться и отчитаться; карточку переносит человек.";
        let bl = c("backlog");
        assert!(md.contains(&format!(
            "### Бэклог (`backlog`)\n\n**Перед работой:** {}\n\n**После работы:** {} {stop}\n\n### В работе",
            bl.before, bl.after
        )), "{md}");
        assert_eq!(md.matches(stop).count(), 3); // backlog, on-dev, done
        // done: work without an executor, empty «После работы» keeps only the stop line
        assert!(md.contains(&format!(
            "**Работа** (исполнитель: нет, работу колонки делает сам агент-драйвер или человек): {}\n\n**После работы:** {stop}\n\n## Правила проекта",
            c("done").rule
        )), "{md}");
        // protocol: «Перед работой» first, before any executor
        assert!(md.contains("сначала «Перед работой» — выполни её (предупредить, спросить, остановиться) до запуска любого исполнителя; только потом «Работа», затем «После работы»."), "{md}");
        // the guard of each column precedes its work and its closing line
        let (g, w, a) = (md.find("**Перед работой:** Эта задача В РАБОТЕ").unwrap(), md.find("**Работа** (исполнитель: `pumba-worker`)").unwrap(), md.find("**После работы:** Успех").unwrap());
        assert!(g < w && w < a);
        // protocol additions
        assert!(md.contains("10. Карточки двигает и `.pumba/` правит только ведущий (основной) агент"));
        assert!(md.contains("Субагенты-исполнители `.pumba/` не трогают и карточки не переносят."));
        assert!(md.contains("11. По просьбе «выполни задачу» используй скилл `pumba-task` (если установлен для вашего инструмента)"));
        assert!(md.contains("12. `model:` и `effort:` карточки передавай субагенту так, как это умеет ваш инструмент; если инструмент не умеет выбирать модель, сообщи об этом пользователю."));
        // tool-neutral: no Claude-only paths or tool names in the protocol
        assert!(!md.contains(".claude/skills") && !md.contains("инструмента Agent"));
        // project rules show the new board default
        assert!(md.contains("- **Обновление доски: Агент двигает задачи.** После успешного выполнения"));
        assert!(md.contains("- **Модель ветвления: Ветка на задачу.**"));

        // custom columns: an agent without `next`, a `next` without an agent, a dangling `next`
        let mut cfg = Config::default();
        let mk = |id: &str, before: &str, rule: &str, after: &str, agent: &str, next: &str| Column {
            id: id.into(),
            name: id.to_uppercase(),
            before: before.into(),
            rule: rule.into(),
            after: after.into(),
            color: String::new(),
            agent: agent.into(),
            next: next.into(),
        };
        cfg.columns = vec![
            mk("a", "", "ra", "", "bot", ""),
            mk("b", "", "rb", "", "", "a"),
            mk("c", "", "rc", "", "", "zz"),
            mk("d", " bd ", "", " ad ", "bot", "a"),
            mk("e", "", "", "", "", ""),
        ];
        let md = agents_md(&cfg, &[]);
        assert!(md.contains("Завершающая колонка: «E» (`e`)."));
        assert!(md.contains(&format!("### A (`a`)\n\n**Работа** (исполнитель: `bot`): ra\n\n**После работы:** {stop}\n\n")), "{md}");
        assert!(md.contains("### B (`b`)\n\n**Работа** (исполнитель: нет, работу колонки делает сам агент-драйвер или человек): rb\n\n**После работы:** После успеха: переместить в «A» (`a`).\n\n"), "{md}");
        assert!(md.contains("переместить в «zz» (`zz`)."));
        // empty work with an executor: only the executor line; parts are trimmed
        assert!(md.contains("### D (`d`)\n\n**Перед работой:** bd\n\n**Работа** (исполнитель: `bot`).\n\n**После работы:** ad После успеха: переместить в «A» (`a`).\n\n"), "{md}");
        // nothing at all: only the closing line
        assert!(md.contains(&format!("### E (`e`)\n\n**После работы:** {stop}\n\n## Правила проекта")), "{md}");
    }

    #[test]
    fn from_md_accepts_bom_and_crlf() {
        let plain = "---\ntitle: T\ncolumn: review\norder: 3\n---\n\nbody line\nsecond\n";
        let want = Task::from_md(plain, "T-001").unwrap();
        let bom = format!("\u{feff}{plain}");
        let crlf = plain.replace('\n', "\r\n");
        let both = format!("\u{feff}{crlf}");
        for text in [bom, crlf, both] {
            let t = Task::from_md(&text, "T-001").unwrap_or_else(|| panic!("{text:?}"));
            assert_eq!((t.title.as_str(), t.column.as_str(), t.order), ("T", "review", 3));
            assert_eq!(t.body, want.body);
        }
        // not a card at all
        assert!(Task::from_md("\u{feff}just text", "T-002").is_none());
    }

    #[test]
    fn merge_disk_takes_disk_for_untouched_fields() {
        let base = Task::from_md("---\ntitle: A\ncolumn: backlog\nsprint: 1\norder: 1\n---\n\nold body\n", "T-001").unwrap();
        let disk = Task::from_md(
            "---\ntitle: A\ncolumn: review\nsprint: 2\norder: 1\n---\n\nold body\n\n## Ход работы\n- note\n",
            "T-001",
        )
        .unwrap();
        // we only moved the card within the order: column and sprint come from the disk, body too
        let mut ours = base.clone();
        ours.order = 9;
        ours.merge_disk(&base, &disk);
        assert_eq!((ours.column.as_str(), ours.sprint, ours.order), ("review", Some(2), 9));
        assert!(ours.body.ends_with("- note"), "{}", ours.body);
        // a field we changed wins
        let mut ours = base.clone();
        ours.column = "done".into();
        ours.merge_disk(&base, &disk);
        assert_eq!(ours.column, "done");
    }

    fn temp_store(tag: &str) -> (Store, PathBuf) {
        let root = std::env::temp_dir().join(format!("pumba-model-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let store = Store::new(&root);
        store.init().unwrap();
        (store, root)
    }

    #[test]
    fn load_full_reports_unreadable_cards() {
        let (store, root) = temp_store("unreadable");
        let tasks = root.join(".pumba/tasks");
        fs::write(tasks.join("T-001.md"), "---\ntitle: ok\ncolumn: backlog\n---\n\nb\n").unwrap();
        fs::write(tasks.join("T-002.md"), "\u{feff}---\r\ntitle: bom\r\ncolumn: backlog\r\n---\r\n\r\nb\r\n").unwrap();
        fs::write(tasks.join("T-007.md"), "no frontmatter here").unwrap();
        fs::write(tasks.join("T-008.md"), [0x2d, 0xff, 0xfe, 0x00]).unwrap(); // not UTF-8
        fs::write(tasks.join("notes.txt"), "ignored").unwrap();
        fs::create_dir(tasks.join("T-009.md")).unwrap(); // a directory is not a card
        let l = store.load_full().unwrap();
        let ids: Vec<&str> = l.tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["T-001", "T-002"]);
        assert_eq!(l.unreadable, ["T-007.md", "T-008.md"]);
        assert!(l.raws.contains_key("T-001") && !l.raws.contains_key("T-007"));
        // the old signature still works
        assert_eq!(store.load().unwrap().1.len(), 2);
        let _ = fs::remove_dir_all(root);
    }
}
