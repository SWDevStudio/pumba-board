# Цели установки pumba-пакета для разных ИИ-агентов

Справка для `pumba init`: куда и в каком формате ставить пакет pumba (5 субагентов и 2 скилла) для каждого инструмента. Данные сверены с официальной документацией 2026-10-31. Пометка **UNVERIFIED** значит, что подтверждения в документации не нашлось.

## 1. Сводная таблица

| Инструмент | Правила (проект) | Субагенты (проект) | Скиллы / команды (проект) | Корневой `AGENTS.md` | Уверенность |
|---|---|---|---|---|---|
| Claude Code | `CLAUDE.md`, `.claude/CLAUDE.md`, `.claude/rules/*.md` | `.claude/agents/*.md` | `.claude/skills/<n>/SKILL.md` (`.claude/commands/*.md` тоже работают) | да, но только если нет `CLAUDE.md`/`CLAUDE.local.md` | высокая |
| OpenAI Codex (CLI/IDE/app) | `AGENTS.md` (+ `AGENTS.override.md`) | `.codex/agents/*.toml` | `.agents/skills/<n>/SKILL.md`; custom prompts устарели | да, основной формат | высокая |
| Cursor | `.cursor/rules/**/*.mdc` | `.cursor/agents/*.md` (+ читает `.claude/agents`, `.codex/agents`) | `.cursor/skills/`, `.agents/skills/` (+ `.claude/skills`, `.codex/skills`) | да, корень и вложенные | высокая |
| GitHub Copilot (VS Code, cloud agent) | `.github/copilot-instructions.md`, `.github/instructions/*.instructions.md` | `.github/agents/*.agent.md` (+ VS Code читает `.claude/agents`) | `.github/skills/`, `.claude/skills/`, `.agents/skills/`; `.github/prompts/*.prompt.md` | да | высокая |
| OpenCode | `AGENTS.md` (fallback `CLAUDE.md`), `instructions` в `opencode.json` | `.opencode/agents/*.md` | `.opencode/skills/`, `.claude/skills/`, `.agents/skills/` | да | высокая |
| Gemini CLI (legacy) | `GEMINI.md` | `.gemini/agents/*.md` | `.gemini/skills/`, `.agents/skills/`; `.gemini/commands/*.toml` | только через `context.fileName` | средняя: продукт заменён |
| Antigravity CLI (`agy`) | `AGENTS.md`/`GEMINI.md`, `.agents/rules/*.md` | `.agents/agents/<n>.md` | `.agents/skills/<n>/SKILL.md` | да | средняя |
| Devin Desktop (бывший Windsurf) | `.devin/rules/*.md` (+ legacy `.windsurf/rules/`, `.windsurfrules`) | Desktop: путь не описан; Devin CLI: `.devin/agents/`, `.agents/agents/` | `.devin/skills/`, `.agents/skills/`, `.windsurf/skills/`; workflows `.devin/workflows/*.md` | да (`AGENTS.md`, `AGENT.md`) | средняя |
| Cline | `.clinerules/*.md`, `.cline/rules/*.md` | нет (UNVERIFIED) | `.cline/skills/`, `.clinerules/skills/`, `.claude/skills/` | да | средняя |

Roo Code не включён: расширение закрыто 2026-05-15, пользователей направили в Cline ([The New Stack](https://thenewstack.io/?p=22820574)).

## 2. Инструменты

### 2.1 Claude Code

- Правила: `CLAUDE.md` или `.claude/CLAUDE.md`. Файлы `.claude/rules/*.md` без `paths:` грузятся при старте с приоритетом `.claude/CLAUDE.md`, с `paths:` — по глобу ([memory](https://code.claude.com/docs/en/memory)).
- `AGENTS.md` читается, только если в рабочей папке и выше нет `CLAUDE.md`, `.claude/CLAUDE.md`, `CLAUDE.local.md`. Настройка **Project instructions** = `claude-md-and-agents-md` читает оба. Нужна v2.1.277+ (там же).
- Субагенты: `.claude/agents/*.md`. Обязательны `name`, `description`. Ещё `tools`, `disallowedTools`, `model` (`sonnet|opus|haiku|fable|<полный id>|inherit`), `effort`, `permissionMode`, `isolation: worktree`, `color` и др. Параметр `model` при вызове принимает только алиасы. **Субагенты теперь могут запускать субагентов** (до 3 уровней, `CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH`). `Task` переименован в `Agent` ([sub-agents](https://code.claude.com/docs/en/sub-agents)).
- Скиллы: `.claude/skills/<n>/SKILL.md`, стандарт [Agent Skills](https://agentskills.io). Поля `name`, `description`, `allowed-tools`, `disable-model-invocation`, `user-invocable`, `model`, `context: fork`, `agent` и др. `/pumba-task T-005` подставляет `$ARGUMENTS` ([skills](https://code.claude.com/docs/en/skills)). `.agents/skills/` в документации не упомянут (UNVERIFIED, считаем: не читает).
- Пример: текущие `templates/agents/pumba-worker.md` и `templates/skills/pumba-task/SKILL.md` без изменений.

### 2.2 OpenAI Codex

- Правила: `AGENTS.md` в каждой папке от корня до cwd. В папке берётся первый из `AGENTS.override.md`, `AGENTS.md`, `project_doc_fallback_filenames`; лимит `project_doc_max_bytes` ([agents-md](https://developers.openai.com/codex/guides/agents-md)).
- Субагенты: `.codex/agents/<n>.toml`, один агент на файл. Обязательны `name` (источник истины, не имя файла), `description`, `developer_instructions`. Необязательны `model`, `model_reasoning_effort`, `model_reasoning_summary`, `model_verbosity`, `features.shell_tool|apps|plugins|memories|request_permissions_tool`, `skills.config`. `sandbox_mode`, `approval_policy`, `mcp_servers` в файле агента **не переопределяют** родителя: «только чтение» держится лишь на инструкции. Списка инструментов нет. Codex запускает субагентов по прямой просьбе или когда этого требуют `AGENTS.md`/скилл ([subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents.md)). Имена моделей в документации: `gpt-6-luna`, `gpt-6.1-sol`. Допустим ли дефис в `name`: UNVERIFIED, в примере `pr_explorer`.
- Скиллы: `.agents/skills/<n>/SKILL.md` (cwd, родители, корень репо). Обязательны `name`, `description`. Вызов: `$pumba-task`, `/skills` или неявно по описанию. Необязательный `agents/openai.yaml` внутри скилла задаёт `policy.allow_implicit_invocation` (по умолчанию `true`) ([build-skills](https://learn.chatgpt.com/docs/build-skills)). Custom prompts (`~/.codex/prompts`) устарели и живут вне репозитория ([custom-prompts](https://developers.openai.com/codex/custom-prompts)).

```toml
# .codex/agents/pumba-worker.toml
name = "pumba-worker"            # UNVERIFIED: дефис; иначе pumba_worker и то же имя в board.json
description = "Исполнитель карточки доски pumba: реализует план pumba-planner. Не коммитит, не трогает .pumba/."
model_reasoning_effort = "medium" # model не задаём: наследуется от родителя
developer_instructions = """
<тело templates/agents/pumba-worker.md без frontmatter>
"""
```

`.agents/skills/pumba-task/SKILL.md` — тот же процесс, что для Claude (`name` + `description`), но с разделом 3 для Codex. Готовые файлы — `templates/targets/codex/`; проверка имени через дефис не дала подтверждения, поэтому в пакете `name = "pumba_worker"` и т. д. (см. `templates/targets/README.md`).

### 2.3 Cursor

- Правила: `.cursor/rules/**/*.mdc`. Простой `.md` там игнорируется. Поля `description`, `globs`, `alwaysApply`. `AGENTS.md` поддержан в корне и во вложенных папках, без frontmatter ([rules](https://cursor.com/docs/context/rules)).
- Субагенты: `.cursor/agents/*.md`, плюс `.claude/agents/`, `.codex/agents/`; при совпадении имён побеждает `.cursor/`. Поля `name`, `description`, `model` (`inherit` по умолчанию или id вида `composer-2`, `gpt-5.6-sol`), `readonly`, `is_background`. **Поля `tools` нет.** Вложенность: субагент 1-го уровня может запускать субагентов, 2-го — уже нет. Вызов: авто по `description`, `/name` или словами ([subagents](https://cursor.com/docs/context/subagents)).
- Скиллы: `.cursor/skills/`, `.agents/skills/`, плюс `.claude/skills/`, `.codex/skills/`. `name` обязан совпадать с папкой. Поля `paths`, `disable-model-invocation`, `metadata`. Вызов: `/` или авто. Slash-команды переводятся в скиллы через `/migrate-to-skills` ([skills](https://cursor.com/docs/context/skills)).

```markdown
<!-- .cursor/agents/pumba-worker.md -->
---
name: pumba-worker
description: Исполнитель карточки доски pumba ... (как в шаблоне)
model: inherit
readonly: false
---
<тело шаблона>
```

Для planner и reviewer — `readonly: true`. Как Cursor обработает `tools:` и `model: haiku` из `.claude/agents`: UNVERIFIED.

### 2.4 GitHub Copilot (VS Code и cloud agent)

- Правила: `.github/copilot-instructions.md`; `.github/instructions/**/*.instructions.md` с `applyTo` (глоб от корня). `AGENTS.md` в корне включён по умолчанию (`chat.useAgentsMdFile`), вложенные — экспериментально. Читаются также `CLAUDE.md` и `.claude/rules` ([custom-instructions](https://code.visualstudio.com/docs/copilot/customization/custom-instructions)). Cloud agent: ближайший `AGENTS.md` или один `CLAUDE.md`/`GEMINI.md` в корне ([GitHub Docs](https://docs.github.com/en/copilot/how-tos/configure-custom-instructions/add-repository-instructions)).
- Субагенты: `.github/agents/*.agent.md` или `.md`. VS Code также читает `.claude/agents/*.md` в формате Claude ([custom-agents](https://code.visualstudio.com/docs/copilot/customization/custom-agents)). Поля: `name`, `description` (обязательно), `target` (`vscode|github-copilot`), `tools`, `model`, `disable-model-invocation`, `user-invocable`; `infer` устарел. Тело — до 30 000 символов. Алиасы инструментов регистронезависимы: `execute`, `read`, `edit`, `search`, `agent`, `web`, `todo` (таблица в разделе 3). Неизвестные имена игнорируются ([reference](https://docs.github.com/en/copilot/reference/custom-agents-configuration)).
- Запуск субагентов в VS Code: инструмент `runSubagent` (`agent/runSubagent`). Координатору нужен `agent` в `tools`, разрешённые субагенты — в `agents: [...]`. Вложенность выключена (`chat.subagents.allowInvocationsFromSubagents`). `model` — имя модели или список с приоритетом ([subagents](https://code.visualstudio.com/docs/copilot/agents/subagents)). Формат имени модели в агенте: UNVERIFIED; в prompt files это отображаемое имя, например `Claude Sonnet 4` ([prompt-files](https://code.visualstudio.com/docs/copilot/customization/prompt-files)).
- Скиллы: `.github/skills/`, `.claude/skills/`, `.agents/skills/`. Поля `name` (= папка, иначе скилл молча не грузится), `description` (≤1024), `argument-hint`, `user-invocable`, `disable-model-invocation`, `context: fork` ([agent-skills](https://code.visualstudio.com/docs/copilot/customization/agent-skills)). Prompt files: `.github/prompts/*.prompt.md`, поля `description`, `name`, `argument-hint`, `agent`, `model`, `tools`.

```markdown
<!-- .github/agents/pumba-worker.agent.md -->
---
name: pumba-worker
description: Исполнитель карточки доски pumba ... (как в шаблоне)
tools: ['read', 'search', 'edit', 'execute']
---
<тело шаблона>
```

### 2.5 OpenCode

- Правила: `AGENTS.md` от cwd вверх. Если его нет — `CLAUDE.md`. Побеждает первый найденный. Дополнительные файлы — массив `instructions` в `opencode.json` ([rules](https://opencode.ai/docs/rules/)).
- Субагенты: `.opencode/agents/<n>.md`, имя = имя файла. Поля: `description` (обязательно), `mode: primary|subagent|all`, `model: provider/model-id`, `temperature`, `steps`, `hidden`, `permission`. Поле `tools` устарело. Ключи `permission`: `read`, `edit` (покрывает write/edit/apply_patch), `glob`, `grep`, `list`, `bash`, `task`, `webfetch`, `skill` и др. Значения `allow|ask|deny`, для `bash` — шаблоны. Вызов: авто или `@name` ([agents](https://opencode.ai/docs/agents/)).
- Скиллы: `.opencode/skills/`, `.claude/skills/`, `.agents/skills/`. `name` по `^[a-z0-9]+(-[a-z0-9]+)*$` и = папка. `description` 1–1024 символа. Загрузка — инструментом `skill`, права — `permission.skill` ([skills](https://opencode.ai/docs/skills/)).

```markdown
<!-- .opencode/agents/pumba-worker.md -->
---
description: Исполнитель карточки доски pumba ... (как в шаблоне)
mode: subagent
permission:
  edit: allow
  bash: allow
  task: deny
---
<тело шаблона>
```

Reviewer и planner: `edit: deny`. Для reviewer `bash` открыт: ему нужен `git diff`.

### 2.6 Gemini CLI (legacy) и Antigravity CLI

Gemini CLI заменён Antigravity CLI (`agy`): пользователей бесплатного тарифа и Google One перевели 2026-06-18. Документация Gemini CLI предупреждает об этом на каждой странице ([subagents](https://geminicli.com/docs/core/subagents/)). Gemini CLI остаётся у платных и enterprise-пользователей (UNVERIFIED, вывод из формулировки).

**Gemini CLI.** Правила: `GEMINI.md` по иерархии, импорт `@file.md`. `AGENTS.md` читается только через `"context": {"fileName": ["AGENTS.md", "GEMINI.md"]}` в `settings.json` ([gemini-md](https://geminicli.com/docs/cli/gemini-md/)). Субагенты: `.gemini/agents/*.md`. Поля: `name` (обязательно), `description` (обязательно), `kind`, `tools`, `mcpServers`, `model` (`inherit` по умолчанию), `temperature`, `max_turns`, `timeout_mins`. Субагенты не вызывают других. Принудительный вызов: `@name`. Имена инструментов: `read_file`, `read_many_files`, `list_directory`, `glob`, `grep_search`, `run_shell_command`, `write_file`, `replace` ([tools](https://geminicli.com/docs/reference/tools/)). Скиллы: `.gemini/skills/` или `.agents/skills/` (приоритет у `.agents`); активация через `activate_skill` с подтверждением пользователя ([skills](https://geminicli.com/docs/cli/skills/)). Команды: `.gemini/commands/<ns>/<n>.toml`, поля `prompt`, `description`, подстановка `{{args}}`; `pumba/task.toml` даёт `/pumba:task` ([custom-commands](https://geminicli.com/docs/cli/custom-commands/)).

```markdown
<!-- .gemini/agents/pumba-worker.md -->
---
name: pumba-worker
description: Исполнитель карточки доски pumba ... (как в шаблоне)
tools: [read_file, read_many_files, list_directory, glob, grep_search, run_shell_command, write_file, replace]
model: inherit
---
<тело шаблона>
```

**Antigravity.** Правила: `AGENTS.md` или `GEMINI.md` (всегда активны), `.agents/rules/*.md` (плоско, legacy `.agent/rules/`). У каждого rule-файла обязателен `trigger: always_on|model_decision|glob|manual`, иначе файл **молча отбрасывается**. Для `model_decision` нужен `description`. Лимит: 24 000 байт на файл и общий бюджет 20 000 токенов ([rules](https://antigravity.google/docs/rules)). Скиллы: `.agents/skills/<n>/SKILL.md`, `description` обязателен, в CLI каждый скилл становится slash-командой ([skills](https://antigravity.google/docs/skills)). Субагенты: `.agents/agents/<n>.md` или `.agents/agents/<n>/agent.md`. Поля: `name`, `description` (обязательны), `tools` (по умолчанию `[]`), `mainAgent`, `subagent`, `model: inherit|flash|pro`, `commandExecutionPolicy: off|auto|eager|sandbox`, `mcpServers`, `skills`. Вызов — `invoke_subagent` или `@name`. Известная ошибка: опечатка в `tools` вешает субагента ([subagents](https://antigravity.google/docs/subagents?tab=cli)). Подтверждены только `view_file`, `replace_file_content`, `grep_search`, `run_command`. Создание файла (`write_to_file`?) и семантика пустого `tools`: UNVERIFIED. Сторонние гайды пишут, что субагенты работают асинхронно в фоне (UNVERIFIED).

### 2.7 Devin Desktop (бывший Windsurf)

Windsurf переименован в Devin Desktop 2026-06-02, агент Cascade заменён на Devin Local (вторичные источники: [usecarly](https://www.usecarly.com/blog/windsurf-is-now-devin-desktop), [jacar.es](https://jacar.es/en/windsurf-agentic-editor/)). `docs.windsurf.com` редиректит на `docs.devin.ai`.

- Правила: `.devin/rules/*.md`, `.devin/global_rules.md`. Legacy `.windsurf/rules/*.md`, `.windsurfrules` по умолчанию тоже импортируются. Поля: `trigger: always_on|glob|model_decision|manual`, `globs`. `AGENTS.md`, `AGENTS.local.md`, `AGENT.md` всегда активны, вложенные тоже ([memories](https://docs.devin.ai/desktop/cascade/memories)).
- Скиллы: `.devin/skills/<n>/SKILL.md`, также `.agents/skills/`, `.windsurf/skills/` (там же). Workflows: `.devin/workflows/*.md`, только ручной `/name`, до 12 000 символов. Devin Local их не поддерживает, документация советует переносить в скиллы ([workflows](https://docs.devin.ai/desktop/cascade/workflows)).
- Субагенты в Desktop: только переключатель «Subagents (Preview)», путь не описан. Devin CLI (экспериментально): `.devin/agents/<n>.md` или `<n>/AGENT.md`, также `.agents/agents/`. Поля: `name`, `description`, `model` (по умолчанию модель субагентов, **не модель родителя**), `allowed-tools` (алиас `tools`; имена `read`, `grep`, `glob`, `exec`), `max-nesting` ([cli/subagents](https://docs.devin.ai/cli/subagents)). Читает ли Desktop эти файлы: UNVERIFIED.

### 2.8 Cline

Правила: `.clinerules/` или `.cline/rules/`, Markdown. Условные правила — frontmatter `paths: [глобы]`, без frontmatter — всегда активны. `AGENTS.md` читается ([cline-rules](https://docs.cline.bot/features/cline-rules)). Скиллы: `.cline/skills/`, `.clinerules/skills/`, `.claude/skills/`; поля `name` (= папка), `description` (≤1024); активация инструментом `use_skill` ([skills](https://docs.cline.bot/customization/skills.md)). Субагентов в документации нет (UNVERIFIED): роли выполняет одна сессия.

## 3. MAPPING

### Инструменты

| Claude | Codex | Cursor | Copilot | OpenCode `permission` | Gemini CLI | Antigravity | Devin CLI |
|---|---|---|---|---|---|---|---|
| `Read` | нет списка | нет поля `tools` | `read` | `read` | `read_file`, `read_many_files` | `view_file` | `read` |
| `Grep` | — | — | `search` | `grep` | `grep_search` | `grep_search` | `grep` |
| `Glob` | — | — | `search` | `glob`, `list` | `glob`, `list_directory` | UNVERIFIED (`find_by_name`?) | `glob` |
| `Bash` | `features.shell_tool` | — | `execute` | `bash` | `run_shell_command` | `run_command` | `exec` |
| `Write` | — | `readonly: false` | `edit` | `edit` | `write_file` | UNVERIFIED (`write_to_file`?) | UNVERIFIED |
| `Edit` | — | `readonly: false` | `edit` | `edit` | `replace` | `replace_file_content` | UNVERIFIED |
| `Agent` (запуск субагента) | спавн по просьбе / `AGENTS.md` | Task | `agent` (`runSubagent`) | `task` | нет у субагентов | `invoke_subagent` | `run_subagent` |

Только чтение (planner, reviewer): Cursor — `readonly: true`; OpenCode — `edit: deny`; Copilot — без `edit`; Gemini CLI и Antigravity — без write/replace. Codex: запрета нет, только текст инструкции. Copilot принимает и имена Claude (`Read`, `Edit`, `Write`, `Bash`, `Grep`, `Glob`) как совместимые алиасы.

### Модели

| Claude | Codex | Cursor | Copilot | OpenCode | Gemini CLI | Antigravity | Devin CLI |
|---|---|---|---|---|---|---|---|
| `inherit` | не задавать `model` | `inherit` | не задавать | не задавать | `inherit` | `inherit` | не задавать (берётся модель субагентов, не родителя) |
| `haiku` (committer) | не задавать `model`, `model_reasoning_effort = "low"` | `inherit` (id меняются) | не задавать | не задавать (`provider/model` у каждого свой) | `inherit` | `flash` | не задавать |
| лестница `haiku → sonnet → opus` | лестница `model_reasoning_effort` low → medium → high (значение при спавне) | нет | `model` при `runSubagent` (UNVERIFIED формат) | нет | нет | `flash → pro` (только во frontmatter) | нет |

Вывод: конкретные имена моделей вне Claude Code не зашивать. Лестницу повышения в нейтральном скилле описать как «повтори на более сильной модели или с большим усилием рассуждений, если твой инструмент это умеет; иначе — один повтор и `on-dev`».

### Что не переносится

- **Тело `pumba-task` завязано на Claude**: «инструмент Agent», `subagent_type`, параметры `model`/`effort`, `SendMessage`, «инструментом Edit», `Grep`. Предлагаю нейтральный `SKILL.md` («запусти субагента `pumba-worker`, если инструмент умеет; иначе выполни роль сам по файлу роли») и отдельный `pumba-task/runners/<tool>.md`. Скилл подгружает его по месту.
- **Нет субагентов** (Cline, Codex без `.codex/agents`, любой fallback): роли складываются в один скилл. Тела субагентов кладутся рядом, в `pumba-task/roles/pumba-*.md`. Драйвер читает файл роли и выполняет её сам, сохраняя формат `ИТОГ: …`. «Свежий взгляд» для ревью тогда недоступен: в отчёте честно пишется «ревью в той же сессии». Правило колонки `in-progress` уже это предусматривает.
- **Хеши тестов, `git`, отчёты** переносятся без изменений: нужны только shell и чтение.
- **Вложенный запуск**: строка «Субагент в Claude Code не может запустить другого субагента» в README шаблонов была неверна (Claude Code — до 3 уровней) и исправлена. Порядок «вспомогательных запускает драйвер» оставлен: Gemini CLI и Copilot (по умолчанию) вложенность запрещают.

## 4. Рекомендация: первые 5 целей

Ключевое наблюдение. Каталог `.agents/skills/` читают Codex, Cursor, Copilot, OpenCode, Gemini CLI, Antigravity и Devin. Каталог `.claude/skills/` читают Claude Code, Cursor, Copilot, OpenCode и Cline. Одного скилла в двух местах хватает почти всем. Но если поставить оба, Cursor, Copilot и OpenCode увидят два `pumba-task` с одним `name`. Поведение при дубле: UNVERIFIED. Поэтому установщик ставит `.agents/skills/` только тогда, когда выбранной цели не хватает `.claude/skills/`.

| # | Цель | Почему | Что ставить | Минимальный указатель |
|---|---|---|---|---|
| 1 | Claude Code | уже есть, полная поддержка цикла | `.claude/agents/pumba-*.md`, `.claude/skills/pumba-{task,new-card}/` | `.claude/rules/pumba.md` (новый файл, без `paths:`): «Карточки pumba — `.pumba/tasks/`, процесс — `.pumba/AGENTS.md`, для работы над T-xxx/E-xxx используй скилл `pumba-task`». `CLAUDE.md` и `AGENTS.md` не трогать. |
| 2 | OpenAI Codex | массовый CLI; `AGENTS.md` нативно; субагенты запускаются, когда этого требует скилл | `.codex/agents/pumba-*.toml` (тело — в `developer_instructions`), `.agents/skills/pumba-{task,new-card}/` (+ `agents/openai.yaml` не нужен) | Отдельного rules-файла у Codex нет. Скилл срабатывает по описанию. Управляемый блок `<!-- pumba:begin -->…<!-- pumba:end -->` в корневом `AGENTS.md` — только с согласия. Если файла нет, создать `AGENTS.md` с одним блоком. |
| 3 | Cursor | популярная IDE; есть субагенты, скиллы, `readonly` | Если выбран Claude Code — ничего: Cursor читает `.claude/agents` и `.claude/skills`. Иначе `.cursor/agents/pumba-*.md` (`model: inherit`, `readonly`), `.cursor/skills/pumba-*/` | `.cursor/rules/pumba.mdc`: `description: Работа с карточками pumba (T-xxx, E-xxx, .pumba/)`, `alwaysApply: false` |
| 4 | GitHub Copilot | самая большая база пользователей; VS Code и cloud agent | `.github/agents/pumba-*.agent.md` (`tools` — алиасы), скиллы в `.github/skills/` (если нет `.claude/skills/pumba-*`) | `.github/instructions/pumba.instructions.md` с `applyTo: ".pumba/**"`. `copilot-instructions.md` не трогать. |
| 5 | OpenCode | открытый CLI, любой провайдер; права на `edit`/`bash` выражают read-only точно | `.opencode/agents/pumba-*.md` (`mode: subagent`, `permission`); скиллы — из `.claude/skills` или `.agents/skills` | Отдельного rules-каталога нет, `opencode.json` — файл пользователя. Указатель — сам скилл. Строку в `instructions` добавлять только с согласия. |

Следующая очередь: **Antigravity CLI** (`.agents/agents/`, `.agents/skills/`, указатель `.agents/rules/pumba.md` с `trigger: model_decision`). Она ждёт подтверждённых имён инструментов записи: опечатка вешает субагента. **Devin Desktop**: `.devin/skills/` или `.agents/skills/`, указатель `.devin/rules/pumba.md` с `trigger: model_decision`, субагенты пока preview. **Gemini CLI** — только для тех, кто остался на нём. **Cline** — режим без субагентов, указатель `.clinerules/pumba.md` с `paths: [".pumba/**"]`.

Общие правила установщика (как в README): существующие файлы не перезаписывать без `--force`. Все новые файлы — с префиксом `pumba-` или в папке `pumba/`. Файлы пользователя (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `opencode.json`, `.gemini/settings.json`, `copilot-instructions.md`) менять только с явного согласия и только размеченным блоком, который удаляется целиком. Каталог `.agents/agents/` не заполнять: его по-разному читают Antigravity (`view_file`, `flash|pro`) и Devin CLI (`read`, `exec`), одного файла на обоих не сделать.

`.pumba/AGENTS.md` не загружается сам ни одним инструментом при старте. Codex, Cursor, Copilot и Devin подхватят его как вложенный `AGENTS.md`, когда агент начнёт работать в `.pumba/`. Claude Code сделает это только без `CLAUDE.md`. Поэтому нужен явный указатель. Плюс: индекс задач в этом файле тогда попадает в контекст и тратит токены.

## 5. Признаки инструмента в проекте (предвыбор в `pumba init`)

| Инструмент | Надёжные признаки | Слабые признаки |
|---|---|---|
| Claude Code | `.claude/` (`agents/`, `skills/`, `settings.json`), `CLAUDE.md`, `CLAUDE.local.md`, `.mcp.json` | `AGENTS.md` (общий) |
| Codex | `.codex/` (`config.toml`, `agents/`), `AGENTS.override.md` | `AGENTS.md`, `.agents/skills/` |
| Cursor | `.cursor/` (`rules/`, `agents/`, `skills/`, `commands/`), `.cursorrules`, `.cursorignore` | — |
| Copilot | `.github/copilot-instructions.md`, `.github/instructions/`, `.github/agents/`, `.github/prompts/`, `.github/skills/` | `.vscode/` |
| OpenCode | `opencode.json`, `opencode.jsonc`, `.opencode/` | — |
| Gemini CLI | `.gemini/` (`settings.json`, `commands/`, `agents/`), `GEMINI.md` | — |
| Antigravity | `.agents/rules/`, `.agents/agents/`, `.agent/` (legacy) | `GEMINI.md`, `.agents/skills/` |
| Devin Desktop | `.devin/`, `.windsurf/`, `.windsurfrules`, `AGENTS.local.md` | — |
| Cline | `.clinerules` (файл или папка), `.cline/` | — |

`AGENTS.md` и `.agents/skills/` общие для многих инструментов. Сами по себе они ничего не предвыбирают: показать их как «есть общий `AGENTS.md`» и спросить.
