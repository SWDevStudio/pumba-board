# Пакеты pumba для других ИИ-инструментов

Готовые файлы пакета pumba для Codex, Cursor, GitHub Copilot и OpenCode. Эталон поведения — пакет Claude Code в `templates/agents/` и `templates/skills/` (см. `templates/README.md`). Форматы и пути сверены с документацией, ссылки и пометки UNVERIFIED — в `templates/TARGETS.md`.

Папка `targets/<id>/` повторяет раскладку внутри проекта: файл `targets/cursor/.cursor/agents/pumba-worker.md` ставится в `<корень проекта>/.cursor/agents/pumba-worker.md`. Шаблонизатора нет: установщик вшивает файлы через `include_str!` и копирует как есть. Этот README в проект не ставится.

## Состав

| Цель (`id`) | Куда ставится | Файлы |
|---|---|---|
| `codex` — OpenAI Codex CLI/IDE | `.codex/agents/`, `.agents/skills/` | 5 агентов `.codex/agents/pumba-*.toml`, скиллы `.agents/skills/pumba-task/SKILL.md` и `.agents/skills/pumba-new-card/SKILL.md`. Файла-указателя нет (см. «Указатель в `AGENTS.md`») |
| `cursor` — Cursor | `.cursor/agents/`, `.cursor/skills/`, `.cursor/rules/` | 5 агентов `.cursor/agents/pumba-*.md`, 2 скилла в `.cursor/skills/`, указатель `.cursor/rules/pumba.mdc` |
| `copilot` — GitHub Copilot (VS Code, облачный агент) | `.github/agents/`, `.github/skills/`, `.github/instructions/` | 5 агентов `.github/agents/pumba-*.agent.md`, 2 скилла в `.github/skills/`, указатель `.github/instructions/pumba.instructions.md` |
| `opencode` — OpenCode | `.opencode/agents/`, `.opencode/skills/`, `.opencode/commands/` | 5 агентов `.opencode/agents/pumba-*.md`, 2 скилла в `.opencode/skills/`, команды `/pumba-task` и `/pumba-new-card` в `.opencode/commands/` |

Роли везде одни и те же: `pumba-planner` (план, только чтение), `pumba-tester` (тесты, приёмка, разбор падений), `pumba-worker` (реализация), `pumba-reviewer` (ревью, только чтение), `pumba-committer` (один коммит). Скиллы: `pumba-task` (драйвер) и `pumba-new-card` (новая карточка).

Полный список файлов: 32 файла (Codex — 7, Cursor — 8, Copilot — 8, OpenCode — 9).

## Что общее со всеми пакетами

Поведение то же, что в пакете Claude Code, тексты совпадают везде, кроме мест, где инструменты действительно различаются:

- драйвер — основная сессия по скиллу `pumba-task`: читает карточку, находит колонку в `board.json` и идёт по трём частям её правила: сначала «Перед работой» (`before`: проверить, предупредить, спросить, остановиться) до запуска любого исполнителя, затем «Работа» (`rule`, сам или субагентом из поля `agent`), затем «После работы» (`after`: проверка результата и что делать при неудаче); переносит карточку в `next` или в колонку, названную в «После работы», и повторяет. На старых досках есть только `rule` — это «Работа»;
- жёсткие остановки: карточка в корзине, незавершённые `depends`, колонка без `next`, «Перед работой» или «После работы» велит ждать человека, неудача на последней ступени повышения, «Автономность: Сначала спрашивать» и крупный план, ручное обновление доски, вопрос к человеку, третье попадание в одну колонку;
- вспомогательных субагентов запускает только драйвер, исполнители никого не запускают;
- правила правки карточки одинаковые: только строка `column:` существующим `id` и новые строки в `## Ход работы`, точечной правкой; остальной frontmatter и `---` не трогать; UTF-8 без BOM; `board.json` и `.pumba/AGENTS.md` не править; карточки не удалять; новые — только скиллом `pumba-new-card` по просьбе пользователя;
- формат ответов ролей (`ИТОГ: …`), план, режимы тестировщика и серьёзность замечаний ревью — без изменений.

Что меняется в каждом пакете:

- раздел 3 скилла `pumba-task` («Как запускать исполнителей в <инструмент>»): чем запускать субагента, как выбрать модель и усилие, как повышать модель, можно ли продолжить прежнего субагента;
- строка «Права в <инструмент>» в начале каждой роли: что инструмент запрещает сам, а что держится только на инструкции;
- список файлов правил проекта, которые читают роли (`AGENTS.md`, `CLAUDE.md`, `.cursor/rules/`, `.github/instructions/` и т. п.);
- способ вызова: `$pumba-task` в Codex, `/pumba-task` в Cursor, Copilot и OpenCode.

Новое во всех четырёх пакетах по сравнению с Claude Code: драйвер сам проверяет права ролей. До и после планировщика и ревьюера он сравнивает `git status --short` и хеш диффа, после тестировщика и исполнителя — `HEAD` и индекс. Причина: в Codex, Cursor и Copilot запреты части ролей держатся только на тексте.

Имена моделей на доске (`haiku`, `sonnet`, `opus` в `model:` карточки, в плане и в «После работы» колонки «В работе») — имена моделей Claude. В других пакетах это уровни сложности. Как каждый инструмент их применяет, написано в его `pumba-task`, раздел 3.

## Отличия по инструментам

### Codex

- Агенты — TOML: `name`, `description`, `developer_instructions` (тело роли). `model` не задан: наследуется от сессии. У коммиттера `model_reasoning_effort = "low"`.
- **Имена через подчёркивание** (см. ниже).
- Субагента драйвер запускает просьбой к Codex («Запусти агента `pumba_worker` с заданием: …»): Codex запускает субагентов, когда этого требует скилл.
- Повышение — усилием рассуждений при запуске: `haiku` → `low`, `sonnet` → `medium`, `opus` → `high`, лестница до `xhigh`, если модель его принимает. Сильнее модель выбирает пользователь (`/model`).
- Скиллы — в `.agents/skills/`: другого места для скиллов проекта у Codex нет.

### Cursor

- Агенты — Markdown с `name`, `description`, `model: inherit`, `readonly`. Планировщик и ревьюер — `readonly: true`, это ограничение Cursor проверяет сам.
- Поля `tools` в Cursor нет: запреты тестировщика, исполнителя и коммиттера (чужие файлы, `git commit`, `git push`) — только в тексте.
- Субагент запускается инструментом Task. Модель можно назвать при запуске (id модели Cursor), усилие — опцией `[effort=…]`, если модель её принимает. Id моделей в файлах не зашиты.
- У коммиттера нет дешёвой модели: `model: inherit`, потому что id моделей Cursor меняются.

### GitHub Copilot

- Агенты — `.agent.md` с `name`, `description`, `tools` (алиасы `read`, `search`, `edit`, `execute`) и `user-invocable: false`: роли не видны в списке агентов, их запускает драйвер.
- Права: у планировщика, ревьюера и коммиттера нет `edit`. Командная строка (`execute`) есть у всех: какие команды git можно запускать, решает только текст.
- Субагент запускается инструментом `runSubagent` (Run Subagent, `agent/runSubagent`), он должен быть включён в выборе инструментов. Явная модель при запуске важнее `model` агента, но формат имени не задокументирован. Усилие передать нечем.
- Указатель `.github/instructions/pumba.instructions.md`: `applyTo: ".pumba/**"` и `description` для подгрузки по смыслу.

### OpenCode

- Агенты — Markdown с `description`, `mode: subagent` и `permission`; имя агента — имя файла.
- Права выражены точнее всего: планировщику, ревьюеру и коммиттеру `edit: deny`; планировщику и ревьюеру без подтверждения разрешены только читающие команды git и `ls`; тестировщику и исполнителю запрещены `git add`, `commit`, `push`, `checkout`, `switch`, `stash`, `reset`, `rebase`, `merge`; коммиттеру — `push`, `--amend`, `--no-verify`, `add -A`, `add .`, `commit -a`, переписывание истории и `--force`. Всем ролям `task: deny`.
- Скиллы OpenCode не показывает как slash-команды (в документации этого нет), поэтому добавлены команды `/pumba-task <id>` и `/pumba-new-card <описание>`: они только загружают скилл.
- Модель при вызове `task` не выбирается: повышения нет.

## Ограничения (что хуже, чем в Claude Code)

| Инструмент | Ограничение | Как обходится |
|---|---|---|
| все | имена моделей на доске — имена Claude | читаются как уровни сложности; перевод — в разделе 3 `pumba-task` |
| все | протокол в `.pumba/AGENTS.md` (пункты 11–12) теперь без привязки к инструменту (скилл «если установлен для вашего инструмента», модель и усилие «как умеет инструмент»); доски со старым текстом пунктов говорят про инструмент Agent и `.claude/skills` | скилл велит выполнять эти пункты по своему разделу 3 |
| все | субагент недоступен вообще (выключен инструмент, режим) | драйвер выполняет роль сам по её файлу и пишет в отчёте «ревью в той же сессии» |
| Codex | агента нельзя сделать «только чтение», списка инструментов нет | запреты — в тексте роли; драйвер проверяет `git status`, дифф, `HEAD` и индекс |
| Codex | модель субагента не повышается | лестница усилия `low` → `medium` → `high` → `xhigh`; дальше — «переключите модель и скажите «продолжи»» |
| Codex | дефис в `name` не подтверждён | имена `pumba_*`, перевод `-` → `_` в скилле |
| Codex | вложенный запуск не описан, субагенты запускаются по просьбе | все запускает драйвер, явной просьбой |
| Codex, OpenCode | нет отдельного файла правил для указателя | срабатывает описание скилла (и команды в OpenCode); размеченный блок в `AGENTS.md` — только с согласия |
| Cursor | нет `tools`: запреты исполнителя, тестировщика и коммиттера — только текст | проверка драйвером `HEAD` и индекса |
| Cursor, Copilot | имена моделей в файлах не зашиты, коммиттер не на дешёвой модели | модель называет драйвер при запуске, если знает id; иначе модель сессии |
| Cursor, Copilot | повышение зависит от того, знает ли драйвер id более сильной модели | не знает — один повтор, затем просьба переключить модель |
| Copilot | `execute` нельзя ограничить по командам | запреты git — в тексте; проверка драйвером |
| Copilot | формат модели в `runSubagent` не задокументирован, усилие передать нечем | модель — только видимая в выборе моделей; усилие не передаётся |
| Copilot | каждый `runSubagent` — новый экземпляр, параллельность не гарантирована | доработка — новым запуском с замечаниями |
| Copilot | указатель с `applyTo: ".pumba/**"` подключается при работе с файлами доски или по описанию | основной вход — описание скилла и `/pumba-task` |
| OpenCode | модель при вызове `task` не выбирается, усилия нет | один повтор на той же модели, затем просьба переключить модель или задать `model:` в `.opencode/agents/pumba-worker.md` |
| OpenCode | команды планировщика и ревьюера вне чтения git ждут подтверждения | так задумано: это роли «только чтение» |

## Имена ролей в Codex

Codex различает агентов по полю `name`, а не по имени файла. Документация не говорит, допустим ли в `name` дефис, во всех её примерах — подчёркивание (`pr_explorer`). Поэтому:

| На доске (`board.json` → `agent`, тексты правил) | Файл в проекте | `name` в Codex |
|---|---|---|
| `pumba-planner` | `.codex/agents/pumba-planner.toml` | `pumba_planner` |
| `pumba-tester` | `.codex/agents/pumba-tester.toml` | `pumba_tester` |
| `pumba-worker` | `.codex/agents/pumba-worker.toml` | `pumba_worker` |
| `pumba-reviewer` | `.codex/agents/pumba-reviewer.toml` | `pumba_reviewer` |
| `pumba-committer` | `.codex/agents/pumba-committer.toml` | `pumba_committer` |

Доска остаётся с дефисами: она общая для всех инструментов. Перевод `-` → `_` делает скилл `.agents/skills/pumba-task/SKILL.md`, то же написано в комментарии в начале каждого `.toml`. Если подтвердится, что дефис допустим, достаточно поменять `name` в пяти файлах и абзац «Имена ролей в Codex» в скилле.

## Кто какие папки читает

Отсюда выбор папок: каждый пакет ставится в собственные папки инструмента, а не в `.claude/` или общую `.agents/`, чтобы не дублировать пакет Claude Code.

| Папка в проекте | Читают |
|---|---|
| `.claude/agents/` | Claude Code, Cursor, Copilot (VS Code) |
| `.claude/skills/` | Claude Code, Cursor, Copilot, OpenCode, Cline |
| `.claude/rules/` | Claude Code, Copilot (VS Code) |
| `.agents/skills/` | Codex, Cursor, Copilot, OpenCode (а также Gemini CLI, Antigravity, Devin); Claude Code — нет (UNVERIFIED) |
| `.codex/agents/` | Codex, Cursor |
| `.cursor/agents/`, `.cursor/skills/`, `.cursor/rules/` | Cursor |
| `.github/agents/`, `.github/skills/`, `.github/instructions/` | Copilot |
| `.opencode/agents/`, `.opencode/skills/`, `.opencode/commands/` | OpenCode |
| `AGENTS.md` в корне | Codex, Cursor, Copilot, OpenCode; Claude Code — только если нет `CLAUDE.md` |

Почему так:

- **Codex** читает скиллы проекта только из `.agents/skills/`, другого выбора нет. Claude Code эту папку не читает, поэтому дубля с пакетом Claude нет.
- **Cursor, Copilot, OpenCode** умеют читать `.claude/skills/`, но скилл Claude описывает инструмент Agent, `subagent_type` и лестницу `haiku → sonnet → opus`. Им нужен свой текст раздела 3, поэтому пакет ставится в `.cursor/`, `.github/`, `.opencode/`, а не в `.claude/` и не в `.agents/`.

Дубли при нескольких целях сразу. Все копии ведут одинаковый цикл, различается только раздел 3 (чем запускать субагента). Если инструмент выберет чужую копию, он может описать запуск чужими словами.

| Выбраны вместе | Что увидит инструмент | Что известно |
|---|---|---|
| Claude Code + Cursor | агенты `pumba-*` в `.claude/agents/` и `.cursor/agents/`; скиллы `pumba-*` в `.claude/skills/` и `.cursor/skills/` | для агентов побеждает `.cursor/`; для скиллов порядок не описан |
| Claude Code + Copilot | агенты в `.claude/agents/` и `.github/agents/`; скиллы в `.claude/skills/` и `.github/skills/` | порядок при совпадении имён не описан |
| Claude Code + OpenCode | скиллы в `.claude/skills/` и `.opencode/skills/` | документация просит уникальные имена; `.opencode/skills/` в списке поиска первая, но порядок при дубле не описан |
| Codex + Cursor / Copilot / OpenCode | скиллы `pumba-*` ещё и в `.agents/skills/` (текст для Codex) | порядок не описан; Cursor ещё видит агентов `pumba_*` из `.codex/agents/` под другими именами |

Установщик при таких сочетаниях печатает предупреждение (в CLI после итога, в TUI в строке состояния: первое и «и ещё N»): «<инструмент> читает и чужие копии пакета: … Скилл `pumba-task` может оказаться в нескольких экземплярах; если <инструмент> выберет не свою копию, удалите лишнюю». Установку оно не блокирует. Правила — `foreign_dirs()` в `src/install.rs`. Сочетания Claude Code + Codex и Cursor + Copilot дублей не дают.

## Правила безопасной установки

- Ставить только файлы из раздела «Точный список файлов», по одному пути на файл; каталоги создавать при необходимости.
- Существующий файл не перезаписывать без `--force`; совпадающий по содержимому считать пропущенным.
- Все устанавливаемые пути содержат `pumba` в имени файла или папки скилла; чужие файлы в тех же папках не трогать и ничего не удалять.
- Файлы пользователя не менять: `AGENTS.md`, `CLAUDE.md`, `opencode.json`, `.codex/config.toml`, `.github/copilot-instructions.md`, `.cursor/rules/*` кроме `pumba.mdc`. Исключение — размеченный блок ниже, только с явного согласия.
- Для целей, кроме Claude Code, не писать в `.claude/`. В `.agents/` пишет только цель `codex` и только `.agents/skills/pumba-*`. Папку `.agents/agents/` не заполнять: её по-разному читают Antigravity и Devin CLI.
- Писать байты как есть: UTF-8 без BOM, переводы строк `\n`.
- Удаление пакета — ровно эти файлы (и пустые папки скиллов `pumba-*`).

## Указатель в `AGENTS.md` (необязательный размеченный блок)

Файлом не поставляется. Нужен для Codex и OpenCode, у которых нет отдельного файла правил: без него скилл срабатывает только по своему описанию или по явному `$pumba-task` / `/pumba-task`. Ставить только с явного согласия пользователя, в корневой `AGENTS.md`, целиком между маркерами; удалять тоже целиком.

```markdown
<!-- pumba:begin -->
## Доска pumba

Карточки — `.pumba/tasks/<ID>.md`, процесс и правила колонок — `.pumba/AGENTS.md`. На «выполни задачу T-001», «возьми эпик E-001» используй скилл `pumba-task`, на «создай карточку» — скилл `pumba-new-card`. Карточку двигает только основная сессия, меняя строку `column:`.
<!-- pumba:end -->
```

Осторожно:

- `AGENTS.md` есть — дописать блок в конец, если маркеров ещё нет; маркеры есть — заменить содержимое между ними.
- `AGENTS.md` нет, но есть `CLAUDE.md` — **не создавать** `AGENTS.md` ради OpenCode: OpenCode берёт первый найденный файл и перестанет читать `CLAUDE.md`. Для Codex новый `AGENTS.md` безопасен, но OpenCode в том же проекте пострадает — спросить пользователя.
- Ни того, ни другого нет — можно создать `AGENTS.md` из одного блока.
- В том же блоке не повторять правила колонок: они живут в `.pumba/AGENTS.md`, и pumba переписывает их сам.
- `AGENTS.override.md` не использовать: в своей папке он заменяет `AGENTS.md` целиком.

## Точный список файлов

Пары «путь в проекте — источник» для `src/install.rs` (пути источников — от `src/`). Для Codex `name` агента не совпадает с именем файла (`pumba_worker` против `pumba-worker.toml`): проверка вида «`name` = имя файла» к нему не применима.

```rust
// codex: 7
(".agents/skills/pumba-new-card/SKILL.md", include_str!("../templates/targets/codex/.agents/skills/pumba-new-card/SKILL.md")),
(".agents/skills/pumba-task/SKILL.md", include_str!("../templates/targets/codex/.agents/skills/pumba-task/SKILL.md")),
(".codex/agents/pumba-committer.toml", include_str!("../templates/targets/codex/.codex/agents/pumba-committer.toml")),
(".codex/agents/pumba-planner.toml", include_str!("../templates/targets/codex/.codex/agents/pumba-planner.toml")),
(".codex/agents/pumba-reviewer.toml", include_str!("../templates/targets/codex/.codex/agents/pumba-reviewer.toml")),
(".codex/agents/pumba-tester.toml", include_str!("../templates/targets/codex/.codex/agents/pumba-tester.toml")),
(".codex/agents/pumba-worker.toml", include_str!("../templates/targets/codex/.codex/agents/pumba-worker.toml")),
// cursor: 8
(".cursor/agents/pumba-committer.md", include_str!("../templates/targets/cursor/.cursor/agents/pumba-committer.md")),
(".cursor/agents/pumba-planner.md", include_str!("../templates/targets/cursor/.cursor/agents/pumba-planner.md")),
(".cursor/agents/pumba-reviewer.md", include_str!("../templates/targets/cursor/.cursor/agents/pumba-reviewer.md")),
(".cursor/agents/pumba-tester.md", include_str!("../templates/targets/cursor/.cursor/agents/pumba-tester.md")),
(".cursor/agents/pumba-worker.md", include_str!("../templates/targets/cursor/.cursor/agents/pumba-worker.md")),
(".cursor/rules/pumba.mdc", include_str!("../templates/targets/cursor/.cursor/rules/pumba.mdc")),
(".cursor/skills/pumba-new-card/SKILL.md", include_str!("../templates/targets/cursor/.cursor/skills/pumba-new-card/SKILL.md")),
(".cursor/skills/pumba-task/SKILL.md", include_str!("../templates/targets/cursor/.cursor/skills/pumba-task/SKILL.md")),
// copilot: 8
(".github/agents/pumba-committer.agent.md", include_str!("../templates/targets/copilot/.github/agents/pumba-committer.agent.md")),
(".github/agents/pumba-planner.agent.md", include_str!("../templates/targets/copilot/.github/agents/pumba-planner.agent.md")),
(".github/agents/pumba-reviewer.agent.md", include_str!("../templates/targets/copilot/.github/agents/pumba-reviewer.agent.md")),
(".github/agents/pumba-tester.agent.md", include_str!("../templates/targets/copilot/.github/agents/pumba-tester.agent.md")),
(".github/agents/pumba-worker.agent.md", include_str!("../templates/targets/copilot/.github/agents/pumba-worker.agent.md")),
(".github/instructions/pumba.instructions.md", include_str!("../templates/targets/copilot/.github/instructions/pumba.instructions.md")),
(".github/skills/pumba-new-card/SKILL.md", include_str!("../templates/targets/copilot/.github/skills/pumba-new-card/SKILL.md")),
(".github/skills/pumba-task/SKILL.md", include_str!("../templates/targets/copilot/.github/skills/pumba-task/SKILL.md")),
// opencode: 9
(".opencode/agents/pumba-committer.md", include_str!("../templates/targets/opencode/.opencode/agents/pumba-committer.md")),
(".opencode/agents/pumba-planner.md", include_str!("../templates/targets/opencode/.opencode/agents/pumba-planner.md")),
(".opencode/agents/pumba-reviewer.md", include_str!("../templates/targets/opencode/.opencode/agents/pumba-reviewer.md")),
(".opencode/agents/pumba-tester.md", include_str!("../templates/targets/opencode/.opencode/agents/pumba-tester.md")),
(".opencode/agents/pumba-worker.md", include_str!("../templates/targets/opencode/.opencode/agents/pumba-worker.md")),
(".opencode/commands/pumba-new-card.md", include_str!("../templates/targets/opencode/.opencode/commands/pumba-new-card.md")),
(".opencode/commands/pumba-task.md", include_str!("../templates/targets/opencode/.opencode/commands/pumba-task.md")),
(".opencode/skills/pumba-new-card/SKILL.md", include_str!("../templates/targets/opencode/.opencode/skills/pumba-new-card/SKILL.md")),
(".opencode/skills/pumba-task/SKILL.md", include_str!("../templates/targets/opencode/.opencode/skills/pumba-task/SKILL.md")),
```

## Как править пакеты

Тексты ролей и скиллов во всех четырёх пакетах и в пакете Claude Code должны совпадать, кроме мест из списка «Что меняется в каждом пакете». Меняете правило в одном пакете — меняйте во всех пяти. Перед коммитом проверьте: frontmatter парсится строгим YAML (описания в кавычках: в простом YAML-значении сочетание «: » ломает разбор), TOML парсится, нет BOM и `\r`, `name` скилла совпадает с папкой.
