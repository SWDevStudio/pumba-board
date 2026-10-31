/* Pumba Board site: language switch, pixel icons, copy buttons. No dependencies. */
(function () {
  'use strict';

  var I18N = {
    "ru": {
      "meta.title": "Pumba Board - канбан-доска для задач ИИ-агентов",
      "meta.desc": "Pumba Board: канбан-доска в терминале для задач ИИ-агентов. Карточки, колонки и правила лежат в папке .pumba рядом с кодом. Работает с Claude Code, Codex, Cursor, Copilot и OpenCode.",
      "skip": "К содержимому",
      "nav.aria": "Разделы",
      "lang.aria": "Язык",
      "nav.about": "Что это",
      "nav.demo": "Доска",
      "nav.start": "Старт",
      "nav.board": "Колонки",
      "nav.cards": "Карточки",
      "nav.rules": "Правила",
      "nav.agent": "Агент",
      "nav.tools": "Инструменты",
      "nav.keys": "Клавиши",
      "gh": "GitHub",
      "hero.tag": "Канбан-доска в терминале для задач ИИ-агентов",
      "hero.lead": "Вы ведёте задачи на доске, а агент берёт карточку, проходит по колонкам и двигает её сам. Всё хранится в папке <code>.pumba/</code> рядом с кодом.",
      "hero.start": "Быстрый старт",
      "hero.logo": "Пиксельный кабан в каске и шарфе, на спине у него маленькая канбан-доска",
      "about.h": "Что это",
      "about.p": "Pumba Board - программа для терминала на Rust, бинарник называется <code>pumba</code>. Вы заводите карточки и настраиваете колонки, а ИИ-агент выполняет работу по этим карточкам.",
      "about.f1.h": "Всё в одной папке",
      "about.f1.p": "Колонки и правила лежат в <code>.pumba/board.json</code>, карточки в <code>.pumba/tasks/</code>, инструкции для агентов в <code>.pumba/AGENTS.md</code>. Это обычные файлы, их видно в git.",
      "about.f2.h": "Порядок задаёте вы",
      "about.f2.p": "У каждой колонки три части правила: «Перед работой», «Работа», «После работы». Что делать с карточкой, решает доска, а не программа.",
      "about.f3.h": "Пять инструментов",
      "about.f3.p": "Claude Code, Codex, Cursor, Copilot и OpenCode. Одна команда ставит им агентов и скиллы.",
      "about.f4.h": "След остаётся",
      "about.f4.p": "Агент записывает краткие решения в раздел «Ход работы» карточки. Удалённые карточки уходят в корзину, файл остаётся на диске.",
      "demo.h": "Как выглядит доска",
      "demo.note": "Макет в стиле программы. Настоящая доска работает в терминале. Интерфейс программы и названия колонок по умолчанию русские.",
      "demo.aria": "Макет канбан-доски: пять колонок с карточками",
      "col.backlog": "Бэклог",
      "col.progress": "В работе",
      "col.review": "На ревью",
      "col.ondev": "Ждёт разработчика",
      "col.done": "Готово",
      "card.e1": "Вход в систему",
      "card.e2": "Оплата заказа",
      "card.t1": "Каркас проекта",
      "card.t2": "Подключить базу данных",
      "card.t4": "Проверка e-mail на сервере",
      "card.t5": "Форма входа",
      "card.t6": "Выбор платёжного провайдера",
      "card.t7": "Экспорт отчёта в CSV",
      "demo.hidden": "скрыта",
      "demo.legend": "<b>◆</b> эпик, <b>▪</b> задача, <b>S1</b> спринт, <b>↳ E-001</b> эпик-родитель, <b>⊘ T-005</b> заблокирована, <b>→ T-007</b> ждут эту карточку.",
      "start.h": "Быстрый старт",
      "start.s1": "Установите из исходников",
      "start.s1n": "Нужен Rust. Крейт называется <code>pumba-board</code>, бинарник <code>pumba</code>.",
      "start.s2": "Откройте доску в папке проекта",
      "start.s2n": "Если <code>.pumba/</code> нет, программа предложит её создать и выбрать инструменты для агентов.",
      "start.s3": "Или создайте доску из командной строки",
      "start.s3n": "Создаёт <code>.pumba/</code> и ставит агентов и скиллы. Флаг <code>--no-agents</code> создаёт только папку.",
      "start.s4": "Поставьте агентов для нужных инструментов",
      "start.s4n": "Существующие файлы не перезаписываются, для этого нужен <code>--force</code>. Вместо списка можно написать <code>all</code>.",
      "start.s5": "Скажите агенту, что делать",
      "start.s5n": "Задачу берёт скилл <code>pumba-task</code>. Можно сказать и «возьми эпик E-001».",
      "copy": "Копировать",
      "copied": "Скопировано",
      "copy.aria": "Копировать команду",
      "board.h": "Как устроена доска",
      "board.p": "Колонки по умолчанию образуют цепочку. Агент ведёт карточку от колонки к колонке, пока не дойдёт до колонки, где нужен человек.",
      "board.chain": "Цепочка колонок по умолчанию",
      "board.ondev": "Неудача после всех попыток уводит карточку в «Ждёт разработчика».",
      "board.parts": "Три части правила колонки",
      "board.before.h": "Перед работой",
      "board.before.p": "Условия старта: что проверить, о чём спросить, когда остановиться. Агент выполняет это раньше всего.",
      "board.rule.h": "Работа",
      "board.rule.p": "Что делает исполнитель колонки, пока карточка в ней: например, план, тесты и реализация.",
      "board.after.h": "После работы",
      "board.after.p": "Как проверить результат, что делать при неудаче и куда перенести карточку.",
      "board.fields": "Кроме правила, у колонки есть <b>агент</b> (субагент, который делает работу), <b>следующая колонка</b> (после успеха), цвет и название. Первая и последняя колонки закреплены: их нельзя двигать и удалять. Новые колонки встают перед последней.",
      "tbl.col": "Колонка",
      "tbl.what": "Что происходит",
      "board.r1": "Агент спрашивает: «брать в работу?». После «да» карточка идёт в «В работе».",
      "board.r2": "Исполнитель <code>pumba-worker</code>: план, тесты, реализация, проверки. Дальше «На ревью».",
      "board.r3": "Исполнитель <code>pumba-reviewer</code>: приёмка и ревью. После одобрения один коммит и «Готово».",
      "board.r4": "Агент остановился и ждёт решения человека. Решение есть - карточка возвращается в «В работе».",
      "board.r5": "Только последняя колонка снимает блокировку с зависимых карточек.",
      "cards.h": "Карточки",
      "cards.p": "Карточка - один файл <code>.pumba/tasks/&lt;ID&gt;.md</code>: заголовок в frontmatter и описание для агента в Markdown. Номер совпадает с именем файла.",
      "cards.c1.h": "Эпик и задача",
      "cards.c1.p": "Эпик (<code>E-001</code>) собирает задачи (<code>T-001</code>). Связь хранится в поле <code>parent</code> задачи, у эпика на доске виден прогресс.",
      "cards.c2.h": "Модель и уровень размышления",
      "cards.c2.p": "Подсказка агенту, какую модель и какой уровень рассуждений взять. Списки настраиваются клавишей <kbd>M</kbd>.",
      "cards.c3.h": "Спринт",
      "cards.c3.p": "<kbd>p</kbd> добавляет карточку в текущий спринт, <kbd>s</kbd> показывает только его, <kbd>]</kbd> начинает следующий: незавершённые карточки перейдут в него.",
      "cards.c4.h": "Зависимости и «Блокирует»",
      "cards.c4.p": "Поле «Зависит от» и обратное «Блокирует». Пока зависимости не в последней колонке, карточка заблокирована, и агент откажется её брать.",
      "cards.c5.h": "Скрытие",
      "cards.c5.p": "<kbd>x</kbd> прячет карточку с доски, <kbd>v</kbd> показывает скрытые.",
      "cards.c6.h": "Корзина вместо удаления",
      "cards.c6.p": "<kbd>d</kbd> отправляет карточку в корзину: файл остаётся на диске. Восстановить можно в корзине (<kbd>B</kbd>). Её номер больше не используется.",
      "cards.code": "Пример файла карточки",
      "cards.at": "Ссылки <code>@путь</code> в описании подставляются подсказкой. Агент прочитает эти файлы перед работой.",
      "rules.h": "Правила проекта",
      "rules.p": "Клавиша <kbd>P</kbd> открывает правила: шесть переключателей и поле для своих правил в Markdown. Из них и из правил колонок программа собирает <code>.pumba/AGENTS.md</code> и переписывает файл при каждом изменении. Агент читает его первым.",
      "rules.note": "Жирным выделен вариант по умолчанию.",
      "rules.r1": "Стиль коммитов",
      "rules.r1o": "<b>Conventional Commits</b>, Gitmoji, префикс с ID задачи, свободный формат",
      "rules.r2": "Модель ветвления",
      "rules.r2o": "GitHub Flow, Git Flow, Trunk-based, <b>ветка на задачу</b>",
      "rules.r3": "Стратегия слияния",
      "rules.r3o": "<b>Squash</b>, merge-коммит, rebase",
      "rules.r4": "Тестирование",
      "rules.r4o": "<b>Тесты обязательны</b>, TDD, тесты по желанию",
      "rules.r5": "Автономность",
      "rules.r5o": "<b>Сначала спрашивать</b> перед крупными изменениями, работать автономно",
      "rules.r6": "Обновление доски",
      "rules.r6o": "<b>Агент двигает задачи</b>, вручную (агент предлагает, вы отвечаете)",
      "agent.h": "Как работает агент",
      "agent.p": "Агенту хватает фразы «выполни задачу T-001». Дальше работает скилл <code>pumba-task</code>: он ведёт карточку по колонкам.",
      "agent.s1.h": "Принимает карточку",
      "agent.s1.p": "Читает файл и проверяет зависимости. Если хоть одна не в последней колонке, агент отказывается и называет блокирующие карточки.",
      "agent.s2.h": "Читает правила",
      "agent.s2.p": "Берёт «Правила проекта» из <code>AGENTS.md</code> и правило колонки карточки из <code>board.json</code>.",
      "agent.s3.h": "Перед работой",
      "agent.s3.p": "Проверяет условия, предупреждает, спрашивает. Правило велит остановиться - агент останавливается.",
      "agent.s4.h": "Работа",
      "agent.s4.p": "Запускает субагента колонки, например <code>pumba-worker</code>, или делает работу сам, если исполнителя нет.",
      "agent.s5.h": "После работы",
      "agent.s5.p": "Проверяет результат: запускает проверки, читает итог исполнителя. При неудаче повторяет на модели выше (haiku, sonnet, opus), потом зовёт человека.",
      "agent.s6.h": "Двигает карточку",
      "agent.s6.p": "Меняет строку <code>column:</code> в файле и идёт дальше, пока не упрётся в колонку без следующей.",
      "agent.s7.h": "Оставляет след",
      "agent.s7.p": "Пишет 1-3 строки в «Ход работы» карточки и отчитывается: что сделано, какой коммит, какие карточки разблокированы.",
      "agent.roles": "Пять субагентов",
      "agent.ro1": "план задачи или порядок карточек эпика, только чтение",
      "agent.ro2": "тесты до реализации, приёмка, разбор упавших проверок",
      "agent.ro3": "реализация по плану",
      "agent.ro4": "ревью с серьёзностью замечаний, только чтение",
      "agent.ro5": "один коммит по правилу «Стиль коммитов»",
      "agent.note": "Субагентов запускает только основная сессия, исполнители никого не вызывают. Второй скилл, <code>pumba-new-card</code>, создаёт карточку с правильным номером и frontmatter по вашей просьбе.",
      "agent.never": "Агент не удаляет карточки, не правит <code>board.json</code> и <code>AGENTS.md</code>, не делает push и слияние без прямой просьбы.",
      "tools.h": "Поддерживаемые инструменты",
      "tools.p": "<code>pumba install</code> кладёт пять агентов и два скилла в папки выбранного инструмента. Существующие файлы пропускаются.",
      "tools.col1": "Инструмент",
      "tools.col2": "Что создаётся",
      "tools.col3": "Вызов",
      "tools.col4": "Файлов",
      "tools.note": "Если выбраны инструменты, которые читают папки друг друга (например, Claude Code и Cursor), установщик печатает предупреждение о возможных дублях скилла <code>pumba-task</code>. Выбор записывается в <code>board.json</code>.",
      "keys.h": "Горячие клавиши",
      "keys.p": "Справка внутри программы открывается клавишей <kbd>?</kbd>. Русская раскладка (ЙЦУКЕН) работает.",
      "keys.g1": "Карточки",
      "keys.g2": "Колонки",
      "keys.g3": "Вид",
      "keys.g4": "Проект",
      "keys.g5": "В формах",
      "key.n": "новая задача",
      "key.N": "новый эпик",
      "key.enter": "правка карточки",
      "key.d": "в корзину",
      "key.x": "скрыть / показать",
      "key.p": "в спринт / из спринта",
      "key.HL": "в предыдущую / следующую колонку",
      "key.JK": "порядок вниз / вверх",
      "key.c": "новая колонка (перед «Готово»)",
      "key.r": "правка колонки и правила",
      "key.X": "удалить колонку",
      "key.ctrl": "колонку влево / вправо",
      "key.hl": "выбор колонки",
      "key.jk": "выбор карточки",
      "key.v": "показать / скрыть скрытые",
      "key.s": "фильтр: только спринт",
      "key.q": "выход",
      "key.help": "эта справка",
      "key.br": "начать следующий спринт",
      "key.P": "правила для агентов",
      "key.M": "модели и уровни",
      "key.at": "вставить ссылку на файл",
      "key.B": "корзина (восстановить)",
      "key.I": "агенты и скиллы (инструменты)",
      "key.save": "сохранить",
      "key.esc": "отмена",
      "key.tab": "следующее / предыдущее поле",
      "foot.src": "Исходный код на GitHub",
      "foot.up": "Наверх"
    },
    "en": {
      "meta.title": "Pumba Board - a kanban board for AI agent tasks",
      "meta.desc": "Pumba Board: a terminal kanban board for AI agent tasks. Cards, columns and rules live in a .pumba folder next to your code. Works with Claude Code, Codex, Cursor, Copilot and OpenCode.",
      "skip": "Skip to content",
      "nav.aria": "Sections",
      "lang.aria": "Language",
      "nav.about": "About",
      "nav.demo": "Board",
      "nav.start": "Start",
      "nav.board": "Columns",
      "nav.cards": "Cards",
      "nav.rules": "Rules",
      "nav.agent": "Agent",
      "nav.tools": "Tools",
      "nav.keys": "Keys",
      "gh": "GitHub",
      "hero.tag": "A terminal kanban board for AI agent tasks",
      "hero.lead": "You keep tasks on the board. The agent takes a card, walks it through the columns and moves it by itself. Everything is stored in a <code>.pumba/</code> folder next to your code.",
      "hero.start": "Quick start",
      "hero.logo": "Pixel-art wild boar in a hard hat and scarf with a tiny kanban board on its back",
      "about.h": "What it is",
      "about.p": "Pumba Board is a terminal program written in Rust; the binary is called <code>pumba</code>. You create cards and set up columns, and an AI agent does the work on those cards.",
      "about.f1.h": "Everything in one folder",
      "about.f1.p": "Columns and rules are in <code>.pumba/board.json</code>, cards in <code>.pumba/tasks/</code>, agent instructions in <code>.pumba/AGENTS.md</code>. They are plain files, so git sees them.",
      "about.f2.h": "You set the process",
      "about.f2.p": "Every column has a rule in three parts: “Before work”, “Work”, “After work”. The board decides what happens to a card, not the program.",
      "about.f3.h": "Five tools",
      "about.f3.p": "Claude Code, Codex, Cursor, Copilot and OpenCode. One command installs agents and skills for them.",
      "about.f4.h": "A trail is kept",
      "about.f4.p": "The agent writes short notes about its decisions into the card’s “Progress log”. Deleted cards go to the trash and the file stays on disk.",
      "demo.h": "What the board looks like",
      "demo.note": "A mock-up in the program’s style. The real board runs in a terminal. The program’s interface and the default column names are in Russian.",
      "demo.aria": "Mock-up of a kanban board: five columns with cards",
      "col.backlog": "Backlog",
      "col.progress": "In progress",
      "col.review": "Review",
      "col.ondev": "Waits for developer",
      "col.done": "Done",
      "card.e1": "Sign-in",
      "card.e2": "Order payment",
      "card.t1": "Project skeleton",
      "card.t2": "Connect the database",
      "card.t4": "Server-side e-mail check",
      "card.t5": "Login form",
      "card.t6": "Pick a payment provider",
      "card.t7": "Export report to CSV",
      "demo.hidden": "hidden",
      "demo.legend": "<b>◆</b> epic, <b>▪</b> task, <b>S1</b> sprint, <b>↳ E-001</b> parent epic, <b>⊘ T-005</b> blocked by, <b>→ T-007</b> cards waiting for this one.",
      "start.h": "Quick start",
      "start.s1": "Install from source",
      "start.s1n": "Rust is required. The crate is <code>pumba-board</code>, the binary is <code>pumba</code>.",
      "start.s2": "Open the board in your project folder",
      "start.s2n": "If there is no <code>.pumba/</code>, the program offers to create it and to pick tools for the agents.",
      "start.s3": "Or create the board from the command line",
      "start.s3n": "Creates <code>.pumba/</code> and installs agents and skills. The <code>--no-agents</code> flag creates the folder only.",
      "start.s4": "Install the agents for the tools you use",
      "start.s4n": "Existing files are not overwritten; use <code>--force</code> for that. You can write <code>all</code> instead of a list.",
      "start.s5": "Tell the agent what to do",
      "start.s5n": "The <code>pumba-task</code> skill takes the card. You can also say “take epic E-001”.",
      "copy": "Copy",
      "copied": "Copied",
      "copy.aria": "Copy the command",
      "board.h": "How the board works",
      "board.p": "The default columns form a chain. The agent walks a card from column to column until it reaches one that needs a human.",
      "board.chain": "Default column chain",
      "board.ondev": "A failure after all attempts sends the card to “Waits for developer”.",
      "board.parts": "The three parts of a column rule",
      "board.before.h": "Before work",
      "board.before.p": "Start conditions: what to check, what to ask, when to stop. The agent does this first.",
      "board.rule.h": "Work",
      "board.rule.p": "What the column’s executor does while the card is there: a plan, tests and the implementation, for example.",
      "board.after.h": "After work",
      "board.after.p": "How to verify the result, what to do on failure and where to move the card.",
      "board.fields": "Besides the rule, a column has an <b>agent</b> (the subagent that does the work), a <b>next column</b> (after success), a colour and a name. The first and the last columns are pinned: you cannot move or delete them. New columns go before the last one.",
      "tbl.col": "Column",
      "tbl.what": "What happens",
      "board.r1": "The agent asks whether to start. After a “yes” the card goes to “In progress”.",
      "board.r2": "Executor <code>pumba-worker</code>: plan, tests, implementation, checks. Next is “Review”.",
      "board.r3": "Executor <code>pumba-reviewer</code>: acceptance and review. After approval, one commit and “Done”.",
      "board.r4": "The agent has stopped and waits for a human decision. Once there is one, the card returns to “In progress”.",
      "board.r5": "Only the last column unblocks dependent cards.",
      "cards.h": "Cards",
      "cards.p": "A card is one file, <code>.pumba/tasks/&lt;ID&gt;.md</code>: a frontmatter header and a Markdown brief for the agent. The id is the file name.",
      "cards.c1.h": "Epic and task",
      "cards.c1.p": "An epic (<code>E-001</code>) groups tasks (<code>T-001</code>). The link lives in the task’s <code>parent</code> field, and the epic shows progress on the board.",
      "cards.c2.h": "Model and reasoning level",
      "cards.c2.p": "A hint for the agent about which model and reasoning level to use. Press <kbd>M</kbd> to edit the lists.",
      "cards.c3.h": "Sprint",
      "cards.c3.p": "<kbd>p</kbd> adds a card to the current sprint, <kbd>s</kbd> shows only that sprint, <kbd>]</kbd> starts the next one and carries unfinished cards over.",
      "cards.c4.h": "Dependencies and “Blocks”",
      "cards.c4.p": "The “Depends on” field and its reverse, “Blocks”. While dependencies are not in the last column the card is blocked, and the agent refuses to take it.",
      "cards.c5.h": "Hiding",
      "cards.c5.p": "<kbd>x</kbd> hides a card from the board, <kbd>v</kbd> shows hidden ones.",
      "cards.c6.h": "Trash instead of deletion",
      "cards.c6.p": "<kbd>d</kbd> moves a card to the trash and keeps the file on disk. Restore it from the trash (<kbd>B</kbd>). Its id is never reused.",
      "cards.code": "Example card file",
      "cards.at": "<code>@path</code> references in the brief are filled in from a suggestion list. The agent reads those files before it starts.",
      "rules.h": "Project rules",
      "rules.p": "The <kbd>P</kbd> key opens the rules: six switches and a field for your own rules in Markdown. From these and from the column rules the program builds <code>.pumba/AGENTS.md</code> and rewrites it on every change. The agent reads it first.",
      "rules.note": "The default option is in bold.",
      "rules.r1": "Commit style",
      "rules.r1o": "<b>Conventional Commits</b>, Gitmoji, task id prefix, free format",
      "rules.r2": "Branching model",
      "rules.r2o": "GitHub Flow, Git Flow, Trunk-based, <b>one branch per task</b>",
      "rules.r3": "Merge strategy",
      "rules.r3o": "<b>Squash</b>, merge commit, rebase",
      "rules.r4": "Testing",
      "rules.r4o": "<b>Tests required</b>, TDD, tests optional",
      "rules.r5": "Autonomy",
      "rules.r5o": "<b>Ask first</b> before big changes, work autonomously",
      "rules.r6": "Board updates",
      "rules.r6o": "<b>Agent moves cards</b>, manual (the agent proposes, you answer)",
      "agent.h": "How the agent works",
      "agent.p": "A phrase like “do task T-001” is enough. Then the <code>pumba-task</code> skill takes over and walks the card through the columns.",
      "agent.s1.h": "Takes the card",
      "agent.s1.p": "Reads the file and checks dependencies. If any of them is not in the last column, the agent refuses and names the blocking cards.",
      "agent.s2.h": "Reads the rules",
      "agent.s2.p": "Takes the project rules from <code>AGENTS.md</code> and the rule of the card’s column from <code>board.json</code>.",
      "agent.s3.h": "Before work",
      "agent.s3.p": "Checks the conditions, warns, asks. If the rule says stop, the agent stops.",
      "agent.s4.h": "Work",
      "agent.s4.p": "Runs the column’s subagent, <code>pumba-worker</code> for example, or does the work itself if there is no executor.",
      "agent.s5.h": "After work",
      "agent.s5.p": "Checks the result: runs the checks and reads the executor’s verdict. On failure it retries on a stronger model (haiku, sonnet, opus), then calls a human.",
      "agent.s6.h": "Moves the card",
      "agent.s6.p": "Changes the <code>column:</code> line in the file and goes on until it hits a column with no next one.",
      "agent.s7.h": "Leaves a trail",
      "agent.s7.p": "Writes 1-3 lines into the card’s “Progress log” and reports: what was done, which commit, which cards got unblocked.",
      "agent.roles": "Five subagents",
      "agent.ro1": "plan for a task or the order of an epic’s cards, read-only",
      "agent.ro2": "tests before the implementation, acceptance, failure triage",
      "agent.ro3": "implementation of the plan",
      "agent.ro4": "review with severity levels, read-only",
      "agent.ro5": "one commit following the “Commit style” rule",
      "agent.note": "Only the main session starts subagents; executors never start anyone. The second skill, <code>pumba-new-card</code>, creates a card with the right id and frontmatter when you ask.",
      "agent.never": "The agent never deletes cards, never edits <code>board.json</code> or <code>AGENTS.md</code>, and does no push or merge unless you ask.",
      "tools.h": "Supported tools",
      "tools.p": "<code>pumba install</code> puts five agents and two skills into the folders of the chosen tool. Existing files are skipped.",
      "tools.col1": "Tool",
      "tools.col2": "What is created",
      "tools.col3": "Invoke",
      "tools.col4": "Files",
      "tools.note": "If you pick tools that read each other’s folders (Claude Code and Cursor, for example), the installer prints a warning about possible duplicates of the <code>pumba-task</code> skill. The choice is saved in <code>board.json</code>.",
      "keys.h": "Hotkeys",
      "keys.p": "The in-program help opens with <kbd>?</kbd>. The Russian (JCUKEN) layout works too.",
      "keys.g1": "Cards",
      "keys.g2": "Columns",
      "keys.g3": "View",
      "keys.g4": "Project",
      "keys.g5": "In forms",
      "key.n": "new task",
      "key.N": "new epic",
      "key.enter": "edit the card",
      "key.d": "to the trash",
      "key.x": "hide / show",
      "key.p": "add to / remove from the sprint",
      "key.HL": "to the previous / next column",
      "key.JK": "order down / up",
      "key.c": "new column (before “Done”)",
      "key.r": "edit the column and its rule",
      "key.X": "delete the column",
      "key.ctrl": "move the column left / right",
      "key.hl": "pick a column",
      "key.jk": "pick a card",
      "key.v": "show / hide hidden cards",
      "key.s": "filter: current sprint only",
      "key.q": "quit",
      "key.help": "this help",
      "key.br": "start the next sprint",
      "key.P": "rules for agents",
      "key.M": "models and levels",
      "key.at": "insert a file reference",
      "key.B": "trash (restore)",
      "key.I": "agents and skills (tools)",
      "key.save": "save",
      "key.esc": "cancel",
      "key.tab": "next / previous field",
      "foot.src": "Source code on GitHub",
      "foot.up": "Back to top"
    }
  };

  var LANGS = ['ru', 'en'];
  var KEY = 'pumba-lang';
  var lang = 'ru';

  // ---------------------------------------------------------------- icons
  // Hand-drawn pixel icons: '#' is a filled pixel. Drawn as inline SVG rects, no blur.
  var ICONS = {
    star: [
      '....#....',
      '....#....',
      '...###...',
      '#########',
      '.#######.',
      '..#####..',
      '..##.##..',
      '.##...##.',
      '.#.....#.'
    ],
    column: [
      '##.##.##',
      '##.##.##',
      '##.##.##',
      '##.##.##',
      '##....##',
      '##......'
    ],
    card: [
      '########',
      '#......#',
      '#.####.#',
      '#......#',
      '#.###..#',
      '#......#',
      '########'
    ],
    arrow: [
      '.....#..',
      '......#.',
      '########',
      '......#.',
      '.....#..'
    ],
    gear: [
      '...#.#...',
      '.#.###.#.',
      '..#####..',
      '###...###',
      '.##...##.',
      '###...###',
      '..#####..',
      '.#.###.#.',
      '...#.#...'
    ]
  };

  function iconSvg(rows) {
    var h = rows.length, w = 0, rects = '', y, x, start;
    for (y = 0; y < h; y++) { if (rows[y].length > w) w = rows[y].length; }
    for (y = 0; y < h; y++) {
      start = -1;
      for (x = 0; x <= w; x++) {
        var on = x < rows[y].length && rows[y].charAt(x) === '#';
        if (on && start < 0) start = x;
        if (!on && start >= 0) {
          rects += '<rect x="' + start + '" y="' + y + '" width="' + (x - start) + '" height="1"/>';
          start = -1;
        }
      }
    }
    return '<svg viewBox="0 0 ' + w + ' ' + h + '" width="' + w + '" height="' + h +
      '" fill="currentColor" shape-rendering="crispEdges" focusable="false" aria-hidden="true">' + rects + '</svg>';
  }

  function drawIcons() {
    var els = document.querySelectorAll('[data-icon]');
    for (var i = 0; i < els.length; i++) {
      var rows = ICONS[els[i].getAttribute('data-icon')];
      if (rows) els[i].innerHTML = iconSvg(rows);
    }
  }

  // ----------------------------------------------------------------- i18n
  function pickLang() {
    var q = '';
    try { q = (new URLSearchParams(location.search).get('lang') || '').toLowerCase(); } catch (e) {}
    if (LANGS.indexOf(q) >= 0) return q;
    try {
      var s = localStorage.getItem(KEY);
      if (LANGS.indexOf(s) >= 0) return s;
    } catch (e) {}
    var nav = ((navigator.languages && navigator.languages[0]) || navigator.language || 'en').toLowerCase();
    return nav.indexOf('ru') === 0 ? 'ru' : 'en';
  }

  function applyLang(l, remember) {
    var d = I18N[l];
    if (!d) return;
    lang = l;
    document.documentElement.lang = l;
    var i, el, keys;
    var t = document.querySelectorAll('[data-i18n]');
    for (i = 0; i < t.length; i++) {
      el = t[i];
      if (d[el.getAttribute('data-i18n')] != null) el.textContent = d[el.getAttribute('data-i18n')];
    }
    t = document.querySelectorAll('[data-i18n-html]');
    for (i = 0; i < t.length; i++) {
      el = t[i];
      if (d[el.getAttribute('data-i18n-html')] != null) el.innerHTML = d[el.getAttribute('data-i18n-html')];
    }
    t = document.querySelectorAll('[data-i18n-attr]');
    for (i = 0; i < t.length; i++) {
      el = t[i];
      keys = el.getAttribute('data-i18n-attr').split(',');
      for (var j = 0; j < keys.length; j++) {
        var p = keys[j].split(':');
        if (p.length === 2 && d[p[1]] != null) el.setAttribute(p[0], d[p[1]]);
      }
    }
    var btns = document.querySelectorAll('[data-lang]');
    for (i = 0; i < btns.length; i++) {
      btns[i].setAttribute('aria-pressed', btns[i].getAttribute('data-lang') === l ? 'true' : 'false');
    }
    if (remember) {
      try { localStorage.setItem(KEY, l); } catch (e) {}
    }
  }

  // ------------------------------------------------------------ copy button
  function fallbackCopy(text) {
    var ta = document.createElement('textarea');
    ta.value = text;
    ta.setAttribute('readonly', '');
    ta.style.position = 'fixed';
    ta.style.opacity = '0';
    document.body.appendChild(ta);
    ta.select();
    var ok = false;
    try { ok = document.execCommand('copy'); } catch (e) {}
    document.body.removeChild(ta);
    return ok;
  }

  function copyText(text) {
    if (navigator.clipboard && window.isSecureContext) {
      return navigator.clipboard.writeText(text).then(function () { return true; }, function () { return fallbackCopy(text); });
    }
    return Promise.resolve(fallbackCopy(text));
  }

  function setupCopy() {
    var btns = document.querySelectorAll('.copy');
    Array.prototype.forEach.call(btns, function (b) {
      b.addEventListener('click', function () {
        copyText(b.getAttribute('data-copy')).then(function (ok) {
          var label = b.querySelector('[data-i18n]');
          if (!label || !ok) return;
          label.setAttribute('data-i18n', 'copied');
          label.textContent = I18N[lang].copied;
          b.classList.add('done');
          setTimeout(function () {
            label.setAttribute('data-i18n', 'copy');
            label.textContent = I18N[lang].copy;
            b.classList.remove('done');
          }, 1600);
        });
      });
    });
  }

  // ------------------------------------------------------------ nav marker
  function setupNav() {
    if (!('IntersectionObserver' in window)) return;
    var links = {};
    Array.prototype.forEach.call(document.querySelectorAll('.nav a'), function (a) {
      links[a.getAttribute('href').slice(1)] = a;
    });
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (en) {
        var a = links[en.target.id];
        if (!a) return;
        if (en.isIntersecting) {
          Object.keys(links).forEach(function (k) { links[k].removeAttribute('aria-current'); });
          a.setAttribute('aria-current', 'true');
        }
      });
    }, { rootMargin: '-30% 0px -60% 0px' });
    Object.keys(links).forEach(function (id) {
      var s = document.getElementById(id);
      if (s) io.observe(s);
    });
  }

  // ------------------------------------------------------------------ init
  function init() {
    drawIcons();
    var langBtns = document.querySelectorAll('[data-lang]');
    Array.prototype.forEach.call(langBtns, function (b) {
      b.addEventListener('click', function () { applyLang(b.getAttribute('data-lang'), true); });
    });
    applyLang(pickLang(), false);
    setupCopy();
    setupNav();
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
