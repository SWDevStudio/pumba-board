# Pumba Board

[Русский](README.md) · [Website](https://swdevstudio.github.io/pumba-board/) · [GitHub](https://github.com/SWDevStudio/pumba-board)

A terminal kanban board for AI agent tasks. You create cards and set up columns; an agent (Claude Code, Codex, Cursor, Copilot or OpenCode) takes a card, walks it through the columns and moves it by itself. Everything lives in a `.pumba/` folder next to your code.

This guide goes step by step: installation, first run, cards, rules, how the agent works, and an example that ends with a closed task. For the big picture only, see the [project website](https://swdevstudio.github.io/pumba-board/).

**A note on language.** The program's interface and the default column names are in Russian. The agents and skills are written in Russian too, and they answer in Russian. Quoted interface labels below are given in Russian with an English gloss on first use.

## Contents

1. [What it is and why](#1-what-it-is-and-why)
2. [Installation](#2-installation)
3. [First run and `init`](#3-first-run-and-init)
4. [Interface and navigation](#4-interface-and-navigation)
5. [Epics, tasks, dependencies, sprints, hiding, trash](#5-epics-tasks-dependencies-sprints-hiding-trash)
6. [Setting up columns](#6-setting-up-columns)
7. [Project rules and `.pumba/AGENTS.md`](#7-project-rules-and-pumbaagentsmd)
8. [The agent kit: `pumba install`](#8-the-agent-kit-pumba-install)
9. [Giving a task to an agent](#9-giving-a-task-to-an-agent)
10. [A worked example: from an empty folder to a closed task](#10-a-worked-example-from-an-empty-folder-to-a-closed-task)
11. [The `.pumba/` layout](#11-the-pumba-layout)
12. [FAQ](#12-faq)

## 1. What it is and why

Pumba Board is a Rust program with a terminal interface (TUI). The binary is called `pumba`.

Why it exists. An agent needs three things: what to do, in what order, and when to stop. Usually that lives in a human's head or in a long prompt. Pumba moves it into project files:

- **cards** describe what to do (one file per card);
- **columns** define stages, and each column's rule tells the agent what to check before the work, what to do, and what to check afterwards;
- **project rules** fix the commit style, branching, merging, testing and autonomy.

The board sets the process, not the program and not the agent's skill. Change a column's rules and the process changes.

Worth knowing up front:

- data is stored in plain files: `.pumba/board.json`, `.pumba/tasks/<ID>.md`, `.pumba/AGENTS.md`;
- the program picks up edits made to those files from outside (by an agent, an editor, `git checkout`);
- an agent moves a card by changing one `column:` line in its file.

## 2. Installation

You need Rust. `Cargo.toml` uses edition 2024, so Rust 1.85 or newer works.

```bash
git clone git@github.com:SWDevStudio/pumba-board.git
cd pumba-board
cargo install --path .
```

The crate is `pumba-board`, the binary is `pumba`. Check it:

```bash
pumba --version
pumba --help
```

`pumba --help` prints the full help for commands and options.

## 3. First run and `init`

There are two ways: open the program, or create the board with a command.

### From the program

Go to your project folder and run `pumba`:

```bash
cd ~/projects/shop
pumba
```

The program looks for `.pumba/board.json` in this folder and above. If it finds none, it asks: «Папка .pumba/ не найдена. Создать её в …?» (“The .pumba/ folder was not found. Create it in …?”). Answer `y` or press Enter. A list of tools follows, for which to install agents and skills:

- tools already found in the project are pre-checked; if none are found, Claude Code is checked;
- `↑`/`↓` (or `k`/`j`) move the cursor, Space toggles a checkbox, `a` checks or unchecks all;
- Enter creates the board and installs the kit; Esc creates only `.pumba/` with no agents (you can install them later with `I`).

### From the command line

```bash
pumba init                       # create .pumba/ and ask about tools
pumba init --tools claude,cursor # no question
pumba init --no-agents           # only .pumba/, no agents or skills
pumba init ~/projects/shop       # another directory
```

If `.pumba/` already exists, `init` prints «Уже инициализировано» (“Already initialized”) and only installs the missing kit files. It never overwrites existing files.

Without `--tools`, `init` in a terminal asks a question with a list: answer with numbers separated by commas (`1,3`); an empty line accepts the pre-checked tools; `0` means none. When there is no terminal (a script, CI), it takes the list from `tools` in `board.json`, otherwise the tools found in the project, otherwise Claude Code. The choice is saved to `board.json`.

### All commands

| Command | What it does |
|---|---|
| `pumba [PATH]` | opens the board; if `.pumba/` is missing, offers to create it |
| `pumba init [--tools LIST \| --no-agents] [PATH]` | creates `.pumba/` and installs agents and skills |
| `pumba install [--tools LIST] [--force] [PATH]` | installs or updates only the agents and skills |
| `pumba install-claude [--force] [PATH]` | same as `install --tools claude` |
| `pumba --help`, `pumba --version` | help and version |

`PATH` is the project directory, the current one by default. The board is searched for upwards from it. Valid tool ids: `claude`, `codex`, `cursor`, `copilot`, `opencode`, or `all`.

### What gets created

After `init` your project has:

```
.pumba/
  board.json    columns, column rules, project rules, sprint
  tasks/        cards (empty for now)
  AGENTS.md     instructions for agents, rebuilt by the program
```

plus agent files in the folders of the chosen tools (details in [section 8](#8-the-agent-kit-pumba-install)).

## 4. Interface and navigation

The screen has four parts:

1. **Header**: the PUMBA badge, the project name, the sprint number, the `[только спринт]` (sprint only) and `[скрытые видны]` (hidden shown) flags, and the trash counter.
2. **Columns with cards.** A column header reads `● Name  card count`, in the column's colour. The selected card has an orange frame. If the columns do not fit the width, the header shows `кол. 2-4/5` (columns 2-4 of 5) on the right.
3. **Detail pane**: on the left the selected card's title, column, model, level, sprint, epic, dependencies and description; on the right the selected column's rule in its three parts.
4. **Hint bar** with the main keys. Error and success messages appear here too.

### What a card looks like

A card on the board takes four lines inside a frame:

1. a marker (`▪` task, `◆` epic), the id and, if set, `model*level`, for example `▪ T-005  opus*high`;
2. two lines of title;
3. a bottom line with tags.

Tags on the bottom line:

| Tag | Meaning |
|---|---|
| `S1` | the card is in sprint 1 |
| `↳ E-001` | parent: epic E-001 |
| `⊘ T-003` | blocked: T-003 is not in the last column yet |
| `✓ зав.` | the card has dependencies and all of them are done |
| `→ T-007,T-008` | these cards are waiting for it |
| `скрыта` | the card is hidden (visible only with `v`) |
| `▰▰▱▱ 2/4` | on an epic: how many child tasks are in the last column |
| `?колонка` | the file names a column that does not exist; the card is shown in the first column |

### Hotkeys

The in-program help opens with `?`. The Russian (JCUKEN) layout works: keys are recognised by position.

**Cards**

| Key | Action |
|---|---|
| `n` | new task |
| `N` | new epic |
| `Enter` / `e` | edit the card |
| `d` | to the trash |
| `x` | hide / show the card |
| `p` | add to / remove from the current sprint |
| `H` / `L` (or `Shift+←` / `Shift+→`) | to the previous / next column |
| `J` / `K` | order in the column down / up |

**Columns**

| Key | Action |
|---|---|
| `c` | new column (goes before the last one) |
| `r` | edit the column and its rule |
| `X` | delete the column |
| `Ctrl+←` / `Ctrl+→` (or `Alt`, or `<` `>`) | move the column left / right |

**View**

| Key | Action |
|---|---|
| `h` / `l` (or `←` / `→`) | pick a column |
| `j` / `k` (or `↓` / `↑`) | pick a card |
| `v` | show / hide hidden cards |
| `s` | filter: current sprint only |
| `?` | help |
| `q` or `Ctrl+C` | quit |

**Project**

| Key | Action |
|---|---|
| `]` | start the next sprint |
| `P` | project rules for agents |
| `M` | models and reasoning levels |
| `B` | trash (restore a card) |
| `I` | agents and skills (tools) |
| `@` | insert a file reference (in description fields) |

### Forms and suggestions

Editing a card, a column, the rules and the models opens a form.

- `Tab` and `Shift+Tab` switch fields; `↑`/`↓` do too (in a multi-line field they move the cursor first). In a two-column form, `Ctrl+←`/`Ctrl+→` jump between the columns of fields.
- `Ctrl+S` saves; `Esc` or `Ctrl+C` cancels.
- In selector fields change the value with `←`/`→` or Space. With eight or more options you can just type letters: the list filters, and Enter or Tab picks.
- A validation error (an empty title, for example) shows in the status line and the form stays open.

Suggestions, so you do not type values by hand:

- **`@` in descriptions.** In multi-line fields (a card description, the parts of a column rule, custom rules) `@` opens a list of project files and folders, searched as you type. `↑`/`↓` move the selection, Enter or Tab inserts `@path/to/file`, Esc closes the list. The list skips files and folders whose names start with a dot, and directories such as `node_modules`, `target`, `dist`, `build`, `.git`. The agent reads those files before it starts.
- **The “Depends on” and “Blocks” fields.** Type part of an id, a title, description text or a column name, and the program shows matching cards (epics are marked); Enter or Tab inserts the id into the comma-separated list.
- **A column's “Executor” field.** The program scans the project's agent folders (`.claude/agents`, `.codex/agents`, `.cursor/agents`, `.github/agents`, `.opencode/agents`) and suggests the names of the subagents it finds. `pumba-*` agents come first. The list is rebuilt every time the form opens.

## 5. Epics, tasks, dependencies, sprints, hiding, trash

### Create an epic and a task

1. Select the column to put the card in. **A new card goes into the currently selected column.** Usually that is “Бэклог” (Backlog).
2. `N` creates an epic, `n` creates a task.
3. Fill in the form and press `Ctrl+S`.

Form fields:

| Field | What it is |
|---|---|
| Название (Title) | one line, required |
| Описание для агента (Brief for the agent) | a Markdown brief: what to do, where, acceptance criteria; the `@` suggestion works here |
| Модель (Model) | a hint for the agent: `default` or a model from the list (`opus`, `sonnet`, `haiku` by default) |
| Уровень (Level) | reasoning level: `default` or `low`, `medium`, `high`, `xhigh`, `max` |
| Спринт (Sprint) | none, current or next |
| Эпик (Epic) | the parent (tasks only) |
| Зависит от (Depends on) | card ids separated by commas |
| Блокирует (Blocks) | ids of cards that wait for this one |

Ids are issued automatically: `E-001`, `E-002` for epics and `T-001`, `T-002` for tasks. The id is the file name. If an epic is selected when you press `n`, it is filled into the Epic field right away. A new form takes the model and level from the defaults (see `M` below).

To edit a card, press `Enter` or `e`.

### Model and reasoning level

These are hints for the agent, not hard settings. `default` means “no preference”. The agent passes the model and level to the executor in whatever way its tool supports. The `M` key opens a form where you can change the lists of models and levels (one value per line) and pick defaults for new cards.

### Dependencies and “Blocks”

In “Depends on” list the cards that must be finished first. While at least one of them is not **in the last column**, the card is blocked:

- on the board it has a `⊘ T-003` tag, and the detail pane shows a red «ЗАБЛОКИРОВАНА» (“BLOCKED”);
- if you move a blocked card (to any column but the first), the status line warns: «Заблокирована: T-003»;
- an agent refuses to take such a card and names the blockers.

“Blocks” is the same thing from the other side: list the cards that wait for this one, and the program adds your card to their `depends:`. “Blocks” itself is not stored in the card; it is computed.

The program validates references. A card cannot depend on itself, on a card that does not exist or is in the trash, and cannot both depend on another card and block it. A cycle (A depends on B, B depends on A) is rejected: «Цикл зависимостей: T-001 → T-002 → T-001» (“Dependency cycle: …”).

A card in the trash does not count as a dependency and blocks nobody.

### Sprints

The header shows the current sprint. A sprint is just a number on a card.

- `p` puts the selected card into the current sprint or takes it out.
- `s` toggles the “current sprint only” filter.
- `]` starts the next sprint. The program asks to confirm and moves every unfinished card of the old sprint (those not in the last column) into the new one.

### Hiding

`x` hides the selected card (it stays in the files). `v` shows or hides hidden cards. A hidden card is shown in italics with the `скрыта` (hidden) tag.

### Trash instead of deletion

`d` asks for confirmation and sends the card to the trash. The file stays in `.pumba/tasks/` and gets `deleted: true`. An id from the trash is never issued again.

`B` opens the trash: `↑`/`↓` pick a card, `Enter` or `r` restores it, `Esc`, `q` or `B` close it. When an epic goes to the trash, its child tasks stay where they are. Agents ignore trashed cards completely.

### Moving and ordering

`H` and `L` move a card to the neighbouring column, `J` and `K` change the order within a column. By hand you can move a card anywhere: a dependency block only warns a human, while it stops an agent.

## 6. Setting up columns

`c` creates a column, `r` edits the selected one. A new column goes before the last one.

### Column form fields

| Field | What it is |
|---|---|
| Название (Name) | what the column is called on the board |
| Перед работой (проверки, стоп) (Before work: checks, stop) | start conditions |
| Работа (что делает исполнитель) (Work: what the executor does) | what happens while the card is in the column |
| После работы (проверка, куда) (After work: check, where to) | how to verify the result and where to go |
| Исполнитель (пусто — нет) (Executor, empty means none) | the column's subagent, with name suggestions |
| После успеха (After success) | the next column (`next`) or “none” |
| Цвет (Colour) | purple, orange, green, red, pink, tan |

### The three parts of a rule

The agent reads a column's rule part by part, strictly in order.

1. **Before work** (`before`): what to check, what to ask the user, when to stop. The agent does this first, before starting any executor. For example, the Backlog column tells the agent to warn that the task is not planned and to ask “take it into work?”. Without a “yes” the agent stops.
2. **Work** (`rule`): what the column's executor does while the card is there. You can name helper subagents here too: “first `pumba-planner`, then `pumba-tester`”.
3. **After work** (`after`): how to verify the result, what to do on failure, and where to move the card for each outcome.

An empty part means there is nothing to do at that step. On old boards that have only `rule`, it is treated as “Work”.

A rule is written as plain text: the agent reads it and follows it. In the parts you can refer to subagents by name, to other columns by id, and to the project rules.

### Subagent and next column

The **Executor** (`agent`) is the subagent that does the column's “Work”. Empty means the driving agent or a human does the work. **After success** (`next`) is the column a card goes to after the work succeeds. Empty means “stop and report; a human moves the card”. A column cannot point to itself.

### Pinned columns

The first and the last columns are pinned by position:

- they cannot be moved (`Ctrl+←`/`→`) or deleted (`X`);
- new columns go before the last one;
- **the last column is the finishing one**: only it lifts the `depends` block.

Deleting another column moves its cards to the first remaining column, and `next` pointers to it are cleared.

### Default columns

| Column | Colour | Executor | Next | What happens |
|---|---|---|---|---|
| Бэклог (Backlog) | purple | none | none | the agent warns and asks “take it into work?”; after a “yes” it moves the card to In progress |
| В работе (In progress) | orange | `pumba-worker` | Review | the planner makes a plan, the tester writes tests (if tests are required), the executor implements, the agent runs the checks; on failure it retries on a stronger model, then “Ждёт разработчика” |
| На ревью (Review) | pink | `pumba-reviewer` | Done | acceptance and review; after approval `pumba-committer` makes one commit; remarks send the card back to In progress, a second return in one run sends it to “Ждёт разработчика” |
| Ждёт разработчика (Waits for developer) | red | none | none | the agent has stopped and waits for a human decision; once there is one, the card goes back to In progress |
| Готово (Done) | green | none | none | the card is finished; on request the agent re-checks the result without edits |

The whole process lives in `board.json` and is easy to change: remove the commit from Review's “After work” and clear its “After success”, and the agent will stop after the review, leaving the commit and the move to you.

## 7. Project rules and `.pumba/AGENTS.md`

The `P` key opens «Правила проекта» (Project rules): six switches and a «Свои правила (Markdown)» (custom rules) field. Change a value with `←`/`→` or Space. `Ctrl+S` saves.

| Rule | Options (the default is in bold) |
|---|---|
| Commit style | **Conventional Commits**, Gitmoji, task id prefix, free format |
| Branching model | GitHub Flow, Git Flow, Trunk-based, **one branch per task** (`pumba/<id>-<slug>`) |
| Merge strategy | **squash**, merge commit, rebase |
| Testing | **tests required**, TDD, tests optional |
| Autonomy | **ask first** before big changes, autonomous |
| Board updates | **agent moves cards**, manual |

Two rules change the agent's behaviour more than the others:

- **Autonomy: “ask first”**. Before a big change (many files, deletions, dependency changes) the agent shows its plan and waits for confirmation. For long epics with no human around, choose “autonomous”.
- **Board updates: “manual”**. The agent does not touch `.pumba/` on its own: it proposes a move and waits for your answer. “Yes” counts as a direct request, and the agent moves the card itself.

The custom rules field takes any Markdown: naming conventions, prohibitions, links to documentation. All of it lands in `AGENTS.md` under «Свои правила».

### How `AGENTS.md` is built

The program generates `.pumba/AGENTS.md` on every board change. You do not need to edit it by hand; edits will be overwritten. The file contains:

1. **A protocol** of 13 points: how to read a card, how to apply a column rule, the hard dependency rule (the agent must refuse a blocked card), how to treat `@path` references, that trashed cards are ignored, that only the main agent moves cards, and that short decisions are written into the “Ход работы” (progress log) section.
2. **Column rules**: for each column “Перед работой”, “Работа” with its executor, and “После работы” with the column to go to on success.
3. **Project rules** and the custom rules.
4. **A task index**: a table `id | type | column | sprint | depends | blocks | title` (trashed cards are left out).

The index can lag behind the card files, so the agent always takes the column, `parent` and `depends` from the cards themselves. If the column rules in `AGENTS.md` differ from `board.json`, `board.json` wins.

## 8. The agent kit: `pumba install`

The kit contains **five subagents** and **two skills**.

| Name | Role |
|---|---|
| `pumba-task` (skill) | the driver: walks a card through the columns |
| `pumba-new-card` (skill) | creates a card with the right id and frontmatter |
| `pumba-planner` | a plan for a task, or the order of an epic's child cards; read-only |
| `pumba-tester` | tests before the implementation, acceptance, triage of failed checks |
| `pumba-worker` | implements the plan |
| `pumba-reviewer` | review with remark severity (blocks, should fix, nit); read-only |
| `pumba-committer` | one commit following the “Commit style” rule; no push, no history rewriting |

```bash
pumba install --tools claude,codex,cursor,copilot,opencode
pumba install --tools all
pumba install --tools claude --force   # overwrite existing files
```

The installer:

- deletes nothing and does not touch other people's files;
- skips existing files («пропущен», skipped); overwriting needs `--force`, and a file identical to the new one still counts as skipped;
- saves the chosen tools to `board.json` (the `tools` field);
- prints how many files were installed and skipped.

If you pick tools that read each other's folders, it prints a warning about possible copies of the `pumba-task` skill, but installs the files anyway. These combinations are: Claude Code with Cursor, Copilot or OpenCode; Codex with Cursor, Copilot or OpenCode. If a tool picks a copy that is not its own, delete the extra one.

Inside the program the `I` key does the same: a list of tools with checkboxes, `Enter` installs.

### What is created for each tool

**Claude Code** (`claude`, 7 files)

- `.claude/agents/pumba-planner.md`, `pumba-tester.md`, `pumba-worker.md`, `pumba-reviewer.md`, `pumba-committer.md`
- `.claude/skills/pumba-task/SKILL.md`, `.claude/skills/pumba-new-card/SKILL.md`
- invoke: “выполни задачу T-001” (“do task T-001”) or `/pumba-task T-001`

**OpenAI Codex** (`codex`, 7 files)

- `.codex/agents/pumba-*.toml` (five agents; inside, `name = "pumba_worker"` and so on, with underscores)
- `.agents/skills/pumba-task/SKILL.md`, `.agents/skills/pumba-new-card/SKILL.md`
- invoke: `$pumba-task` or a request like “выполни задачу T-001”

**Cursor** (`cursor`, 8 files)

- `.cursor/agents/pumba-*.md` (five agents; planner and reviewer have `readonly: true`)
- `.cursor/skills/pumba-task/SKILL.md`, `.cursor/skills/pumba-new-card/SKILL.md`
- `.cursor/rules/pumba.mdc`: a short pointer to `.pumba/AGENTS.md` and the skills
- invoke: `/pumba-task T-001`

**GitHub Copilot** (`copilot`, 8 files; VS Code and the cloud agent)

- `.github/agents/pumba-*.agent.md` (five agents, hidden from the agent list and started by the driver)
- `.github/skills/pumba-task/SKILL.md`, `.github/skills/pumba-new-card/SKILL.md`
- `.github/instructions/pumba.instructions.md`: a pointer with `applyTo: ".pumba/**"`
- invoke: `/pumba-task T-001`

**OpenCode** (`opencode`, 9 files)

- `.opencode/agents/pumba-*.md` (five agents, `subagent` mode, permissions set through `permission`)
- `.opencode/skills/pumba-task/SKILL.md`, `.opencode/skills/pumba-new-card/SKILL.md`
- `.opencode/commands/pumba-task.md`, `.opencode/commands/pumba-new-card.md`: the commands `/pumba-task <id>` and `/pumba-new-card <description>`

The packages for different tools run the same loop. They differ in how a subagent is started, how a model is chosen, and what permissions the tool itself enforces. Details, documentation links and each tool's limits are in [`templates/targets/README.md`](templates/targets/README.md) and [`templates/TARGETS.md`](templates/TARGETS.md) (both in Russian).

Model names on the board (`haiku`, `sonnet`, `opus`) are Claude model names. In other tools they are read as difficulty levels; how exactly is written in section 3 of the `pumba-task` skill in each package.

## 9. Giving a task to an agent

Open the agent in your project folder and write in Russian:

- «выполни задачу T-001» (do task T-001)
- «возьми карточку T-012», «сделай T-3» (take card T-012, do T-3)
- «возьми эпик E-002» (take epic E-002)
- «продолжи T-005» (continue T-005)
- «создай карточку: …», «заведи задачу», «разбей эпик E-002 на задачи» (create a card, add a task, split epic E-002 into tasks; these go to `pumba-new-card`)

You can invoke the skill explicitly: `/pumba-task T-001` (`$pumba-task` in Codex).

### What the agent does under the `pumba-task` skill

**Accepting the card**

1. It normalises the id: `t-5`, `T5`, `т-005` become `T-005`. If the file does not exist, the agent says so and stops; it does not create the card.
2. If the card is in the trash (`deleted: true`), it refuses.
3. **It checks dependencies.** For each id in `depends:` it reads the file. If at least one dependency is not in the last column, the agent refuses, lists them («T-003 - «В работе» (`in-progress`)») and suggests working on them first. It does not bypass the block, even when asked.
4. It reads the project rules, the custom rules and, if present, `CLAUDE.md` and `AGENTS.md` in the project root.
5. It checks that the `@path` references from the description exist.

**The loop over columns**

1. It re-reads the card (you may have moved it in the program) and finds its column in `board.json`.
2. **Before work**: it checks the conditions and warns or asks. If the rule says stop, the agent stops without moving the card.
3. **Work**: it starts the column's executor (the `agent` field) and any helper subagents in the order the “Work” text names. With no executor, it does the work itself. If a needed subagent is missing in the project, it stops and names it.
4. **After work**: it verifies the result itself, not on trust: runs the checks, reads the executor's `ИТОГ:` line, looks at the last commit. On failure it retries on a stronger model up the ladder `haiku → sonnet → opus`, one attempt per step. If `opus` fails or a human is needed, the card goes to “Ждёт разработчика”.
5. It moves the card (changes the `column:` line) to the right column and returns to step 1.

**Stops.** The loop ends when:

- the card is in the trash or blocked;
- the column's “After success” is empty and “After work” does not say where to go next;
- “Before work” or “After work” says to wait for a human decision;
- the executor failed on `opus`;
- with the “ask first” rule the plan is marked “Крупное изменение: да” (“Big change: yes”) and you have not confirmed it yet;
- manual board updates are on: every move waits for your answer;
- a question comes up that the card, the plan and the rules cannot answer;
- the card lands in the same column for the third time in one run.

**How executors are started.** Only the main session (the driver) starts subagents; executors never start anyone. In the task for a subagent the driver puts the project root, the card, the column rule, the plan and the starting point. Acceptance, failure triage and review are done by a fresh subagent instance, not by the one that wrote the code. After the tester, the driver remembers the hashes of the test files and checks that the executor did not change them.

**Git.** Before the first executor that changes files, the driver looks at `git status --short` and leaves other people's uncommitted changes alone. It picks the branch by the “Branching model” rule (for example `pumba/T-005-login-form`). Only `pumba-committer` commits, and only where the column rule says so. Push, merge, pull request, `--force`, `--amend`, `rebase` and `reset` are done only on your direct request.

**An epic** goes through the columns the same way. In a column with an executor the driver walks the child cards instead of running the executor: `pumba-planner` in `epic` mode suggests the order and waves, and for backlog cards the driver asks once for the whole epic whether to take them. The epic leaves the column when all its children are in the last one.

**What the agent leaves on the board**

- After a stop, a report (up to 20 lines): where the card is and why it stopped; what was done per column; changed files and check results; review remarks; the commit and branch; what it needs from you; which cards got unblocked.
- In the card body, under `## Ход работы`, it appends short lines such as `- 2026-10-31 14:05 - «В работе» → «На ревью»: проверки зелёные, исполнитель pumba-worker (sonnet).` If it decided something itself or did something differently from the card, it writes 1-3 lines on what and why. With manual board updates `.pumba/` is not edited, and the agent tells you the note instead.

**What the agent does not do.** It does not delete card files. It does not create cards or ids itself (only through the `pumba-new-card` skill, at your request). It does not change `title`, `type`, `parent`, `depends`, `order`, `sprint`, `model`, `effort` or `hidden` without a request. It does not edit `board.json` and `AGENTS.md`. It does not move a card to a column that is not in “After success” or in the rule parts.

### The `pumba-new-card` skill

It creates a card as the file `.pumba/tasks/<ID>.md` at your request. Before creating, it checks for duplicate titles (the trash included), that the parent exists and is an epic, that the dependencies exist and form no cycles, and that the column exists in `board.json`. The id is the maximum among all files, the trash included, plus one. If a dependency is not done yet, it tells you that the new card is blocked. It does not start work on the new card until you ask.

## 10. A worked example: from an empty folder to a closed task

Say we are building a small shop and working with Claude Code.

**1. A folder and a board**

```bash
mkdir shop && cd shop
git init
pumba init --tools claude
```

The output ends with a line like `Готово: установлено 7, пропущено 0 (корень: …)` (“Done: installed 7, skipped 0”). The project now has `.pumba/` and `.claude/`.

**2. Open the board**

```bash
pumba
```

Five empty columns, with “Бэклог” selected.

**3. An epic and tasks**

- `N`: title «Вход в систему» (Sign-in), `Ctrl+S`. This becomes `E-001`.
- With the epic selected, press `n`. Title «Форма входа» (Login form). In the description write what to do and how to check it, with a file link through `@` (for example `@src/login.html`). The Epic field is already filled; `Ctrl+S`. This becomes `T-001`.
- Press `n` again: «Проверка e-mail на сервере» (Server-side e-mail check). In “Depends on” start typing `T-0` and pick `T-001` from the suggestion. `Ctrl+S`. This is `T-002`: it has the tag `⊘ T-001`, and `T-001` has the tag `→ T-002`.
- Select `T-001` and press `p` to put it into the current sprint.

The card file looks like this:

```markdown
---
id: T-001
type: task
title: Форма входа
column: backlog
parent: E-001
model: default
effort: default
sprint: 1
hidden: false
order: 1
---

Сделать страницу входа с полями e-mail и пароль.

Где: @src/login.html

Критерии приёмки:
- пустая форма не отправляется;
- ошибка показывается под полем.
```

**4. Ask the agent**

Start Claude Code in the `shop` folder and write: «выполни задачу T-002». The agent reads the dependencies, sees that `T-001` is in Backlog, refuses and asks you to work on it first. This rule beats any request.

Now write: «выполни задачу T-001».

1. The card is in Backlog. “Before work” says to warn and ask. The agent says the task is in the backlog and asks whether to take it. You answer “да” (yes).
2. The agent changes `column: backlog` to `column: in-progress`. You see the card move in the program: it re-reads the files by itself.
3. In “В работе” `pumba-planner` runs and makes a plan. Under the “ask first” autonomy rule, if the plan is marked «Крупное изменение: да», the agent shows it and waits for confirmation. Then `pumba-tester` writes tests (the “tests required” rule), `pumba-worker` implements, and the agent runs the checks itself.
4. The checks are green and the card moves to “На ревью”. `pumba-reviewer` looks at the diff. If there are “blocks” or “should fix” remarks, the card goes back to “В работе” and the remarks are passed to the executor verbatim.
5. The review approves: `pumba-committer` makes one commit under the “Conventional Commits” rule, and the card moves to “Готово”.
6. The agent writes its report: the commit, the branch (`pumba/T-001-login-form`), the check results, and “`T-002` gets unblocked: all its dependencies are in the last column”.

The body of `T-001` now has short lines under `## Ход работы`.

**5. Next**

`T-002` is no longer blocked. “выполни задачу T-002” will go the same way. When all of the epic's children are in “Готово”, `E-001` shows a full progress bar `▰▰▰▰ 2/2` on the board.

If at any step the agent fails or waits for a decision, the card ends up in “Ждёт разработчика”. The reason is written in the progress log. Resolve it, tell the agent «продолжи T-001» (continue T-001), and it returns the card to “В работе”.

## 11. The `.pumba/` layout

```
.pumba/
  board.json
  AGENTS.md
  tasks/
    E-001.md
    T-001.md
    T-002.md
```

### `board.json`

| Key | What it holds |
|---|---|
| `version` | format version (1) |
| `columns` | columns in order: `id`, `name`, `color`, `agent`, `next`, `before`, `rule`, `after` |
| `models`, `efforts` | lists of models and reasoning levels for the forms |
| `default_model`, `default_effort` | defaults for new cards |
| `sprint` | the current sprint number |
| `rules` | the six project rules: `id`, `title`, the chosen `value` and the `options` |
| `custom_rules` | the custom rules (Markdown) |
| `tools` | ids of the tools the kit was installed for |

A column with all fields:

```json
{
  "id": "in-progress",
  "name": "В работе",
  "color": "orange",
  "agent": "pumba-worker",
  "next": "review",
  "before": "…",
  "rule": "…",
  "after": "…"
}
```

Colours: `purple`, `orange`, `green`, `red`, `pink`, `tan`. The last column in the list is the finishing one. An old `board.json` without the newer keys is read as is.

If `board.json` cannot be read, the program writes nothing and reports an error: the file is never overwritten.

### `tasks/<ID>.md`

The file starts with a `---` block with keys in this order:

| Key | Value |
|---|---|
| `id` | same as the file name |
| `type` | `task` or `epic` |
| `title` | the title on one line |
| `column` | the column `id` from `board.json` |
| `parent` | the epic id (written only if there is one) |
| `model`, `effort` | hints for the agent, `default` for no preference |
| `sprint` | the sprint number (written only if there is one) |
| `hidden` | `true` or `false` |
| `order` | position in the column |
| `depends` | ids separated by commas (written only if there are any) |
| `deleted`, `deleted_at` | trash: written only when the card is in the trash |

After the closing `---` come an empty line and a Markdown body. The id comes from the file name. `@path` references are counted from the project root.

Details that matter:

- pumba writes the frontmatter itself and drops unknown keys, so agents do not add any;
- encoding is UTF-8 with `\n` line endings; the program tolerates a BOM and CRLF when reading, but agents are told to write without a BOM;
- unreadable cards do not vanish silently: their file names show in red in the status line;
- the program watches the disk and merges outside changes with yours: if an agent appended to the progress log or moved a card while you were not changing it, the outside edit is kept.

### `AGENTS.md`

Created and rewritten by the program. Details are in [section 7](#7-project-rules-and-pumbaagentsmd). It is the first file agents read.

## 12. FAQ

**Should I commit `.pumba/`?** Your call. They are plain text files, and git sees them. Add them to your project's `.gitignore` only if the board is just for you.

**The agent does not see the skill or a subagent.** Check that the kit is installed for your tool: `pumba install --tools <id>`. The end of the output should list «установлен» (installed) or «пропущен» (skipped) lines. If the agent session was open before the install, open a new one.

**The agent refuses to take a card.** Most likely it is blocked: at least one dependency is not in the last column. The agent lists the blockers. Either finish them, or remove the dependency in the edit form (`Enter`, the “Depends on” field).

**The agent stopped at Backlog.** That is what the column's “Before work” says: it waits for a “yes”. Either answer, or move the card to “В работе” yourself (`L`).

**I want to check the result myself before “Done”.** Clear “After success” on Review and remove the commit from its “After work”: the agent stops after the review, and you do the commit and the move.

**I want the agent to leave `.pumba/` alone.** Choose “Board updates: manual”. The agent then proposes a move and waits for your “yes”.

**Can I add my own column, such as “Tests”?** Yes: `c`, fill in the name, the three rule parts, the executor and “After success”. The column goes before “Done”. For cards to arrive there, set it as “After success” of the previous column.

**What happens to cards when I delete a column?** They move to the first remaining column, and `next` pointers to the deleted column are cleared. The first and last columns cannot be deleted.

**Why is a card in the first column marked `?колонка`?** Its file has a `column:` with an id that does not exist (a typo or a deleted column). Open the card with `Enter` and move it with `H`/`L`, or fix the line in the file.

**How do I restore a deleted card?** `B`, pick the card, `Enter`.

**Does the Russian keyboard layout work?** Yes, keys are recognised in JCUKEN too.

**Where is the key help?** `?` on the board. In forms: `Ctrl+S` saves, `Esc` cancels.

**Why are the models `opus`, `sonnet`, `haiku` when I use another tool?** They are hints for the agent. You can change the lists with `M`. In non-Claude tools the skill reads them as difficulty levels (see section 3 of the `pumba-task` skill in your package).

**What is in `templates/`?** The sources of the agent kit: agents, skills and packages for different tools. They are embedded in the binary, so the program does not need the `templates/` folder after installation. Descriptions: [`templates/README.md`](templates/README.md), [`templates/targets/README.md`](templates/targets/README.md); a study of the tools' formats: [`templates/TARGETS.md`](templates/TARGETS.md); a sketch of a future API for agents: [`templates/API-DESIGN.md`](templates/API-DESIGN.md). These documents are in Russian.

---

Website: <https://swdevstudio.github.io/pumba-board/>. Repository: <https://github.com/SWDevStudio/pumba-board>.
